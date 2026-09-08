use anyhow::Result;
use serde::Serialize;
use std::path::Path;
#[derive(Debug, Serialize)]
pub struct BackendInfo {
    pub id: String,
    pub available: bool,
    pub per_monitor: bool,
    pub transitions: bool,
}
pub trait WallpaperBackend: Send + Sync {
    fn info(&self) -> BackendInfo;
    /// Release only processes owned by this backend when switching away.
    fn deactivate(&self) {}
    fn apply(&self, path: &Path, monitor: Option<&str>) -> Result<()>;
}
