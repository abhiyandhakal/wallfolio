use anyhow::Result;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub provider: String,
    pub external_id: String,
    pub title: String,
    pub source: String,
    pub thumbnail: Option<String>,
    pub tags: Vec<String>,
}
/// Discovery does not mutate the user's catalog or download full images.
pub trait WallpaperProvider: Send + Sync {
    fn id(&self) -> &str;
    fn search(&self, query: &str, page: u32) -> Result<Vec<Candidate>>;
    fn get(&self, external_id: &str) -> Result<Candidate>;
}
