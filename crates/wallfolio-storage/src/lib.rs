pub mod cache;
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const MAX_IMAGE_BYTES: u64 = 100 * 1024 * 1024;
pub struct StoredImage {
    pub path: PathBuf,
    pub hash: String,
    pub width: u32,
    pub height: u32,
}
pub struct Storage {
    root: PathBuf,
}
impl Storage {
    pub fn new(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(root.join("originals"))?;
        Ok(Self {
            root: root.canonicalize()?,
        })
    }
    pub fn import(&self, mut input: impl Read) -> Result<StoredImage> {
        let temporary = self
            .root
            .join(format!("originals/.incoming-{}", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut out = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            let mut hash = Sha256::new();
            let mut total = 0u64;
            let mut buffer = [0u8; 65536];
            loop {
                let n = input.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                total += n as u64;
                if total > MAX_IMAGE_BYTES {
                    bail!("image exceeds 100 MiB limit");
                }
                hash.update(&buffer[..n]);
                out.write_all(&buffer[..n])?;
            }
            out.sync_all()?;
            let reader = image::ImageReader::open(&temporary)?.with_guessed_format()?;
            let format = reader.format().context("unsupported image format")?;
            if !matches!(
                format,
                image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP
            ) {
                bail!("supported formats: PNG, JPEG, WebP");
            }
            let (width, height) = reader.into_dimensions()?;
            if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 100_000_000 {
                bail!("invalid or excessive image dimensions");
            }
            // Validate the complete image before promoting a download. Header-only
            // validation would silently cache truncated files as successful downloads.
            let mut reader = image::ImageReader::open(&temporary)?.with_guessed_format()?;
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(256 * 1024 * 1024);
            limits.max_image_width = Some(16_384);
            limits.max_image_height = Some(16_384);
            reader.limits(limits);
            drop(
                reader
                    .decode()
                    .context("invalid image data or decoder memory limit exceeded")?,
            );
            let hash = format!("{:x}", hash.finalize());
            let dir = self.root.join("originals").join(&hash[..2]);
            fs::create_dir_all(&dir)?;
            let path = dir.join(format!("{}.{}", hash, format.extensions_str()[0]));
            if !path.exists() {
                fs::rename(&temporary, &path)?;
            }
            Ok(StoredImage {
                path,
                hash,
                width,
                height,
            })
        })();
        let _ = fs::remove_file(&temporary);
        result
    }
    pub fn delete(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let canonical = path.canonicalize()?;
        if !canonical.starts_with(self.root.join("originals").canonicalize()?) {
            bail!("refusing to delete a file outside managed originals");
        }
        fs::remove_file(canonical)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[test]
    fn rejects_truncation_and_preserves_external_files() -> Result<()> {
        let root = std::env::temp_dir().join(format!("wallfolio-storage-{}", uuid::Uuid::new_v4()));
        let storage = Storage::new(root.clone())?;
        let mut bytes = Cursor::new(Vec::new());
        image::RgbImage::new(2, 2).write_to(&mut bytes, image::ImageFormat::Png)?;
        let bytes = bytes.into_inner();
        let stored = storage.import(Cursor::new(&bytes))?;
        assert_eq!(stored.path, storage.import(Cursor::new(&bytes))?.path);
        let truncated = &bytes[..45];
        assert_eq!(
            image::ImageReader::new(Cursor::new(truncated))
                .with_guessed_format()?
                .into_dimensions()?,
            (2, 2)
        );
        assert!(storage.import(Cursor::new(truncated)).is_err());
        assert!(!fs::read_dir(root.join("originals"))?.any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".incoming")));
        let external = root.join("user.png");
        fs::write(&external, &bytes)?;
        assert!(storage.delete(&external).is_err());
        assert!(external.exists());
        storage.delete(&stored.path)?;
        assert!(!stored.path.exists());
        fs::remove_dir_all(root)?;
        Ok(())
    }
}
