use anyhow::{bail, Context, Result};
use std::{
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use wallfolio_backend_api::{BackendInfo, WallpaperBackend};
use wallfolio_provider_api::{Candidate, WallpaperProvider};

pub struct LocalProvider;
impl WallpaperProvider for LocalProvider {
    fn id(&self) -> &str {
        "local"
    }
    fn get(&self, id: &str) -> Result<Candidate> {
        let path = Path::new(id).canonicalize()?;
        if !path.is_file() {
            bail!("source must be a regular file");
        }
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !["png", "jpg", "jpeg", "webp"].contains(&ext.as_str()) {
            bail!("supported formats: PNG, JPEG, WebP");
        }
        let source = path.to_str().context("path is not UTF-8")?.to_owned();
        Ok(Candidate {
            provider: "local".into(),
            external_id: source.clone(),
            title: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            source,
            thumbnail: None,
            tags: vec![],
        })
    }
    fn search(&self, query: &str, page: u32) -> Result<Vec<Candidate>> {
        // A single directory, with bounded discovery and no symlink recursion.
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(query)?.take(10_000) {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                if let Ok(candidate) = self.get(&entry.path().to_string_lossy()) {
                    entries.push(candidate);
                }
            }
        }
        entries.sort_by(|a, b| a.external_id.cmp(&b.external_id));
        Ok(entries
            .into_iter()
            .skip(page.saturating_sub(1) as usize * 24)
            .take(24)
            .collect())
    }
}

pub struct WallhavenProvider {
    client: reqwest::blocking::Client,
}
impl WallhavenProvider {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .user_agent(concat!("wallfolio/", env!("CARGO_PKG_VERSION")))
                .build()?,
        })
    }
    fn candidate(value: &serde_json::Value) -> Result<Candidate> {
        let id = value["id"].as_str().context("missing Wallhaven id")?;
        Ok(Candidate {
            provider: "wallhaven".into(),
            external_id: id.into(),
            title: format!("Wallhaven {id}"),
            source: value["path"]
                .as_str()
                .context("missing download URL")?
                .into(),
            thumbnail: value["thumbs"]["large"].as_str().map(str::to_owned),
            tags: value["tags"]
                .as_array()
                .map(|tags| {
                    tags.iter()
                        .filter_map(|t| t["name"].as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
        })
    }
    fn request(&self, url: &str, query: &[(&str, String)]) -> Result<serde_json::Value> {
        use std::io::Read;
        let response = self
            .client
            .get(url)
            .query(query)
            .send()?
            .error_for_status()?;
        let mut bytes = Vec::new();
        response.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 2 * 1024 * 1024 {
            bail!("provider response too large");
        }
        Ok(serde_json::from_slice(&bytes)?)
    }
}
impl WallpaperProvider for WallhavenProvider {
    fn id(&self) -> &str {
        "wallhaven"
    }
    fn search(&self, query: &str, page: u32) -> Result<Vec<Candidate>> {
        let value = self.request(
            "https://wallhaven.cc/api/v1/search",
            &[("q", query.into()), ("page", page.max(1).to_string())],
        )?;
        value["data"]
            .as_array()
            .context("missing provider results")?
            .iter()
            .map(Self::candidate)
            .collect()
    }
    fn get(&self, id: &str) -> Result<Candidate> {
        if id.len() != 6 || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
            bail!("invalid Wallhaven ID");
        }
        Self::candidate(&self.request(&format!("https://wallhaven.cc/api/v1/w/{id}"), &[])?["data"])
    }
}

pub struct CommandBackend {
    pub id: &'static str,
}
fn installed(executable: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p).any(|d| {
                std::fs::metadata(d.join(executable))
                    .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}
fn run(program: &str, args: &[&str]) -> Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("cannot start {program}"))?;
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                bail!("{program} failed ({status}); check its daemon and session");
            }
            return Ok(());
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("{program} timed out");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
impl WallpaperBackend for CommandBackend {
    fn info(&self) -> BackendInfo {
        let available = std::env::var_os("WAYLAND_DISPLAY").is_some()
            && installed(if self.id == "hyprpaper" {
                "hyprctl"
            } else {
                "swww"
            });
        BackendInfo {
            id: self.id.into(),
            available,
            per_monitor: true,
            transitions: self.id == "swww",
        }
    }
    fn apply(&self, path: &Path, monitor: Option<&str>) -> Result<()> {
        let path = path.to_str().context("path is not UTF-8")?;
        let monitor = monitor.unwrap_or("");
        if monitor.contains([',', '\n', '\r']) || monitor.starts_with('-') {
            bail!("invalid monitor name");
        }
        match self.id {
            "swww" => {
                let mut args = vec!["img", path];
                if !monitor.is_empty() {
                    args.extend(["--outputs", monitor]);
                }
                run("swww", &args)
            }
            "hyprpaper" => {
                if path.contains([',', '\n', '\r']) {
                    bail!("Hyprpaper cannot accept commas or newlines in image paths");
                }
                run(
                    "hyprctl",
                    &["hyprpaper", "wallpaper", &format!("{monitor},{path}")],
                )
            }
            _ => bail!("unknown backend"),
        }
    }
}
