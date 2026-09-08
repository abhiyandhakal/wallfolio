use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashSet, VecDeque},
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

pub const CACHE_LIMIT: u64 = 256 * 1024 * 1024;
enum Job {
    Remote(String),
    Local(PathBuf),
}
#[derive(Default)]
struct Queue {
    jobs: VecDeque<(String, Job)>,
    keys: HashSet<String>,
}
/// One bounded worker owns image decoding; network failures never fail discovery.
pub struct ThumbnailCache {
    root: PathBuf,
    limit: u64,
    queue: Mutex<Queue>,
    files: Mutex<()>,
    http: reqwest::blocking::Client,
}
impl ThumbnailCache {
    pub fn new(root: PathBuf, limit: u64) -> Result<Arc<Self>> {
        fs::create_dir_all(&root)?;
        let cache = Arc::new(Self {
            root: root.canonicalize()?,
            limit,
            queue: Mutex::new(Queue::default()),
            files: Mutex::new(()),
            http: reqwest::blocking::Client::builder()
                .https_only(true)
                .timeout(Duration::from_secs(10))
                .build()?,
        });
        cache.evict()?;
        Ok(cache)
    }
    pub fn start(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        std::thread::spawn(move || loop {
            let Some(cache) = weak.upgrade() else {
                break;
            };
            let _ = cache.process_one();
            drop(cache);
            std::thread::sleep(Duration::from_millis(100));
        });
    }
    pub fn lookup(&self, key: &str) -> Option<PathBuf> {
        if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let _guard = self.files.lock().unwrap();
        let path = self.root.join(format!("{key}.png"));
        if path.is_file() {
            if let Ok(file) = fs::File::open(&path) {
                let _ = file.set_times(fs::FileTimes::new().set_modified(SystemTime::now()));
            }
            return Some(path);
        }
        None
    }
    fn request(&self, key: String, job: Job) -> Option<PathBuf> {
        if let Some(path) = self.lookup(&key) {
            return Some(path);
        }
        let mut queue = self.queue.lock().unwrap();
        if queue.jobs.len() < 100 && queue.keys.insert(key.clone()) {
            queue.jobs.push_back((key, job));
        }
        None
    }
    pub fn remote_key(url: &str) -> String {
        format!("{:x}", Sha256::digest(url.as_bytes()))
    }
    pub fn local_key(hash: &str) -> String {
        format!("{:x}", Sha256::digest(format!("local:{hash}").as_bytes()))
    }
    pub fn remote(&self, url: &str) -> Option<PathBuf> {
        let key = Self::remote_key(url);
        self.request(key, Job::Remote(url.into()))
    }
    pub fn local(&self, path: &Path, hash: &str) -> Option<PathBuf> {
        let key = Self::local_key(hash);
        self.request(key, Job::Local(path.into()))
    }
    pub fn process_one(&self) -> Result<()> {
        let Some((key, job)) = self.queue.lock().unwrap().jobs.pop_front() else {
            return Ok(());
        };
        let result = (|| {
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(256 * 1024 * 1024);
            limits.max_image_width = Some(16384);
            limits.max_image_height = Some(16384);
            let image = match job {
                Job::Remote(url) => {
                    let response = self.http.get(url).send()?.error_for_status()?;
                    let mut bytes = Vec::new();
                    response.take(5 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
                    if bytes.len() > 5 * 1024 * 1024 {
                        bail!("thumbnail exceeds 5 MiB");
                    }
                    limits.max_alloc = Some(32 * 1024 * 1024);
                    let mut reader =
                        image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
                    reader.limits(limits);
                    reader.decode()?
                }
                Job::Local(path) => {
                    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
                    reader.limits(limits);
                    reader.decode()?
                }
            };
            let thumbnail = image.thumbnail(image.width().min(512), image.height().min(320));
            let _guard = self.files.lock().unwrap();
            let temporary = self.root.join(format!("{key}.part"));
            thumbnail.save_with_format(&temporary, image::ImageFormat::Png)?;
            fs::rename(temporary, self.root.join(format!("{key}.png")))?;
            self.evict_locked()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(self.root.join(format!("{key}.part")));
        }
        self.queue.lock().unwrap().keys.remove(&key);
        result
    }
    fn entries(&self) -> Result<Vec<(SystemTime, u64, PathBuf)>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.file_type()?.is_file() && entry.path().extension().is_some_and(|s| s == "png")
            {
                let m = entry.metadata()?;
                entries.push((
                    m.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    m.len(),
                    entry.path(),
                ));
            }
        }
        Ok(entries)
    }
    fn evict_locked(&self) -> Result<()> {
        let mut entries = self.entries()?;
        let mut bytes: u64 = entries.iter().map(|e| e.1).sum();
        entries.sort_by_key(|e| e.0);
        for (_, size, path) in entries {
            if bytes <= self.limit {
                break;
            }
            fs::remove_file(path)?;
            bytes -= size;
        }
        Ok(())
    }
    pub fn evict(&self) -> Result<()> {
        let _guard = self.files.lock().unwrap();
        self.evict_locked()
    }
    pub fn stats(&self) -> Result<(u64, usize, u64)> {
        let _guard = self
            .files
            .lock()
            .map_err(|_| anyhow::anyhow!("cache lock poisoned"))?;
        let entries = self.entries().context("read thumbnail cache")?;
        Ok((entries.iter().map(|e| e.1).sum(), entries.len(), self.limit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_preview_is_cached_and_reused_offline() -> Result<()> {
        use std::io::Write;
        let root =
            std::env::temp_dir().join(format!("wallfolio-remote-cache-{}", uuid::Uuid::new_v4()));
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let url = format!("http://{}/preview.png", listener.local_addr()?);
        let server = std::thread::spawn(move || -> std::io::Result<()> {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let mut request = [0; 4096];
            assert!(stream.read(&mut request)? > 0);
            let mut png = Cursor::new(Vec::new());
            image::DynamicImage::ImageRgb8(image::RgbImage::new(8, 4))
                .write_to(&mut png, image::ImageFormat::Png)
                .unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                png.get_ref().len()
            )?;
            stream.write_all(png.get_ref())
        });
        let mut cache = ThumbnailCache::new(root.clone(), CACHE_LIMIT)?;
        // Only this test permits HTTP, for an isolated loopback fixture server.
        Arc::get_mut(&mut cache).unwrap().http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;
        assert!(cache.remote(&url).is_none());
        cache.process_one()?;
        server.join().unwrap()?;
        let path = cache.remote(&url).unwrap();
        assert_eq!(image::image_dimensions(&path)?, (8, 4));
        drop(cache);
        let cache = ThumbnailCache::new(root.clone(), CACHE_LIMIT)?;
        // The HTTP server is gone; the normal HTTPS-only client still hits disk.
        assert_eq!(cache.remote(&url), Some(path));
        assert!(cache.lookup("../../outside.png").is_none());
        assert_eq!(cache.stats()?.1, 1);
        drop(cache);
        fs::remove_dir_all(root)?;
        Ok(())
    }
    #[test]
    fn generated_thumbnails_survive_restart_and_evict_old_entries() -> Result<()> {
        let root = std::env::temp_dir().join(format!("wallfolio-cache-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root)?;
        let source = root.join("source.png");
        image::RgbImage::new(1024, 768).save(&source)?;
        let cache = ThumbnailCache::new(root.join("cache"), CACHE_LIMIT)?;
        assert!(cache.local(&source, "one").is_none());
        cache.process_one()?;
        let path = cache.local(&source, "one").unwrap();
        assert_eq!(image::image_dimensions(&path)?, (427, 320));
        let size = fs::metadata(&path)?.len();
        drop(cache);
        let cache = ThumbnailCache::new(root.join("cache"), size)?;
        assert!(cache.local(&source, "one").is_some());
        cache.local(&source, "two");
        cache.process_one()?;
        assert!(!path.exists());
        assert!(cache.stats()?.0 <= size);
        drop(cache);
        fs::remove_dir_all(root)?;
        Ok(())
    }
}
