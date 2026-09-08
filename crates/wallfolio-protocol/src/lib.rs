use serde::{Deserialize, Serialize};
use serde_json::Value;
pub const VERSION: u32 = 1;
pub const MAX_FRAME: u64 = 1024 * 1024;
#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub version: u32,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
impl Response {
    pub fn success(result: Value) -> Self {
        Self {
            ok: true,
            result: Some(result),
            error: None,
        }
    }
    pub fn failure(error: impl ToString) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(error.to_string()),
        }
    }
}

pub fn socket_path() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("WALLFOLIO_SOCKET") {
        return path.into();
    }
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| ".".into())
                .join(".cache")
        });
    base.join("wallfolio/wallfoliod.sock")
}
