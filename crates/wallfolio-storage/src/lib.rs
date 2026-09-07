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
        Ok(Self { root })
    }
    pub fn import(&self, mut input: impl Read) -> Result<StoredImage> {
        let temporary = self.root.join("originals/.incoming");
        let result = (|| {
            let mut out = fs::File::create(&temporary)?;
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
            let (width, height) = reader.into_dimensions()?;
            if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 100_000_000 {
                bail!("invalid or excessive image dimensions");
            }
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
