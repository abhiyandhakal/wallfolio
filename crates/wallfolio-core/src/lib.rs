use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf};
use wallfolio_backend_api::WallpaperBackend;
use wallfolio_catalog::{Catalog, Wallpaper};
use wallfolio_protocol::{Request, Response, VERSION};
use wallfolio_provider_api::WallpaperProvider;
use wallfolio_storage::Storage;

pub struct Application {
    catalog: Catalog,
    storage: Storage,
    providers: BTreeMap<String, Box<dyn WallpaperProvider>>,
    backends: BTreeMap<String, Box<dyn WallpaperBackend>>,
    http: reqwest::blocking::Client,
}
impl Application {
    pub fn open(root: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&root)?;
        Ok(Self {
            catalog: Catalog::open(&root.join("wallfolio.db"))?,
            storage: Storage::new(root)?,
            providers: BTreeMap::new(),
            backends: BTreeMap::new(),
            http: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .https_only(true)
                .build()?,
        })
    }
    pub fn register_provider(&mut self, provider: Box<dyn WallpaperProvider>) {
        self.providers.insert(provider.id().into(), provider);
    }
    pub fn register_backend(&mut self, backend: Box<dyn WallpaperBackend>) {
        self.backends.insert(backend.info().id.clone(), backend);
    }
    pub fn handle(&self, request: Request) -> Response {
        if request.version != VERSION {
            return Response::failure("unsupported protocol version");
        }
        match self.dispatch(&request.method, request.params) {
            Ok(value) => Response::success(value),
            Err(error) => Response::failure(format!("{error:#}")),
        }
    }
    fn dispatch(&self, method: &str, p: Value) -> Result<Value> {
        let id = || required(&p, "id");
        match method {
            "device.info" => Ok(
                json!({"version": env!("CARGO_PKG_VERSION"), "os": std::env::consts::OS, "desktop": std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default()}),
            ),
            "device.backends" => Ok(serde_json::to_value(
                self.backends.values().map(|b| b.info()).collect::<Vec<_>>(),
            )?),
            "provider.list" => Ok(json!(self.providers.keys().collect::<Vec<_>>())),
            "provider.search" => {
                let provider = self
                    .providers
                    .get(required(&p, "provider")?)
                    .context("unknown provider")?;
                Ok(serde_json::to_value(provider.search(
                    p["query"].as_str().unwrap_or(""),
                    number(&p, "page", 1),
                )?)?)
            }
            "provider.get" => Ok(serde_json::to_value(
                self.providers
                    .get(required(&p, "provider")?)
                    .context("unknown provider")?
                    .get(required(&p, "external_id")?)?,
            )?),
            "catalog.search" => Ok(serde_json::to_value(self.catalog.search(
                p["query"].as_str().unwrap_or(""),
                p["favorite"].as_bool().unwrap_or(false),
                number(&p, "limit", 100),
                number(&p, "offset", 0),
            )?)?),
            "catalog.get" => Ok(serde_json::to_value(self.catalog.get(id()?)?)?),
            "catalog.add" => {
                let candidate = self
                    .providers
                    .get(required(&p, "provider")?)
                    .context("unknown provider")?
                    .get(required(&p, "external_id")?)?;
                let wallpaper = self.catalog.add(Wallpaper {
                    id: String::new(),
                    title: candidate.title,
                    provider: candidate.provider,
                    external_id: candidate.external_id,
                    source: candidate.source,
                    thumbnail: candidate.thumbnail,
                    tags: candidate.tags,
                    favorite: false,
                    local_path: None,
                    content_hash: None,
                    width: None,
                    height: None,
                })?;
                Ok(serde_json::to_value(wallpaper)?)
            }
            "catalog.remove" => {
                self.catalog.remove(id()?)?;
                Ok(json!({"removed":true}))
            }
            "favorite.add" | "favorite.remove" => {
                let mut wallpaper = self.catalog.get(id()?)?;
                wallpaper.favorite = method == "favorite.add";
                self.catalog.update(&wallpaper)?;
                Ok(serde_json::to_value(wallpaper)?)
            }
            "catalog.tags" => {
                let mut wallpaper = self.catalog.get(id()?)?;
                let tags: Vec<String> = serde_json::from_value(p["tags"].clone())?;
                if tags.len() > 100 || tags.iter().any(|t| t.len() > 100) {
                    bail!("at most 100 tags of 100 bytes each");
                }
                wallpaper.tags = tags
                    .into_iter()
                    .map(|t| t.trim().to_owned())
                    .filter(|t| !t.is_empty())
                    .collect();
                wallpaper.tags.sort();
                wallpaper.tags.dedup();
                self.catalog.update(&wallpaper)?;
                Ok(serde_json::to_value(wallpaper)?)
            }
            "wallpaper.download" => {
                let mut wallpaper = self.catalog.get(id()?)?;
                let stored = if wallpaper.provider == "local" {
                    self.storage
                        .import(std::fs::File::open(&wallpaper.source)?)?
                } else {
                    let response = self
                        .http
                        .get(&wallpaper.source)
                        .send()?
                        .error_for_status()?;
                    self.storage.import(response)?
                };
                wallpaper.local_path = Some(stored.path.to_string_lossy().into_owned());
                wallpaper.content_hash = Some(stored.hash);
                wallpaper.width = Some(stored.width);
                wallpaper.height = Some(stored.height);
                self.catalog.update(&wallpaper)?;
                Ok(serde_json::to_value(wallpaper)?)
            }
            "wallpaper.delete_local" => {
                let mut wallpaper = self.catalog.get(id()?)?;
                if let Some(path) = &wallpaper.local_path {
                    if let Some(hash) = &wallpaper.content_hash {
                        if self.catalog.hash_references(hash)? <= 1 {
                            self.storage.delete(std::path::Path::new(path))?;
                        }
                    }
                }
                wallpaper.local_path = None;
                self.catalog.update(&wallpaper)?;
                Ok(serde_json::to_value(wallpaper)?)
            }
            "wallpaper.apply" => {
                let wallpaper = self.catalog.get(id()?)?;
                let path = PathBuf::from(
                    wallpaper
                        .local_path
                        .context("download the wallpaper before applying it")?,
                );
                if !path.is_file() {
                    bail!("local copy is missing; download it again");
                }
                let backend = match p["backend"].as_str().filter(|s| !s.is_empty()) {
                    Some(name) => self.backends.get(name).context("unknown backend")?,
                    None => self
                        .backends
                        .values()
                        .find(|b| b.info().available)
                        .context("no available wallpaper backend")?,
                };
                backend.apply(&path, p["monitor"].as_str())?;
                Ok(json!({"applied":true,"backend":backend.info().id}))
            }
            _ => bail!("unknown method: {method}"),
        }
    }
}
fn required<'a>(params: &'a Value, key: &str) -> Result<&'a str> {
    params[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .with_context(|| format!("missing {key}"))
}
fn number(params: &Value, key: &str, default: u32) -> u32 {
    params[key]
        .as_u64()
        .map(|v| v.min(u32::MAX as u64) as u32)
        .unwrap_or(default)
}
