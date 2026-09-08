pub mod rotation;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::sync::Arc;
use std::{collections::BTreeMap, path::PathBuf};
use wallfolio_backend_api::WallpaperBackend;
use wallfolio_catalog::{Catalog, Wallpaper};
use wallfolio_protocol::{Request, Response, VERSION};
use wallfolio_provider_api::WallpaperProvider;
use wallfolio_storage::{
    cache::{ThumbnailCache, CACHE_LIMIT},
    Storage,
};

pub struct Application {
    catalog: Catalog,
    cache: Arc<ThumbnailCache>,
    storage: Storage,
    providers: BTreeMap<String, Box<dyn WallpaperProvider>>,
    backends: BTreeMap<String, Box<dyn WallpaperBackend>>,
    http: reqwest::blocking::Client,
}
impl Application {
    pub fn open(root: PathBuf) -> Result<Self> {
        let cache = root.join("cache/thumbnails");
        Self::open_with_cache(root, cache)
    }
    pub fn open_with_cache(root: PathBuf, cache_root: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&root)?;
        Ok(Self {
            cache: ThumbnailCache::new(cache_root, CACHE_LIMIT)?,
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
    pub fn start_background(&self) {
        self.cache.start();
    }
    fn decorate(&self, mut item: Value) -> Value {
        let cached = if let (Some(path), Some(hash)) =
            (item["local_path"].as_str(), item["content_hash"].as_str())
        {
            self.cache.local(std::path::Path::new(path), hash)
        } else if let Some(url) = item["thumbnail"].as_str() {
            self.cache.remote(url)
        } else {
            None
        };
        item["cached_thumbnail"] = json!(cached.map(|p| p.to_string_lossy().into_owned()));
        item
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
        let p = if p.is_null() { json!({}) } else { p };
        if !p.is_object() {
            bail!("params must be an object");
        }
        let id = || required(&p, "id");
        match method {
            "device.info" => Ok(
                json!({"version": env!("CARGO_PKG_VERSION"), "os": std::env::consts::OS, "pid": std::process::id(), "desktop": std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default()}),
            ),
            "device.settings" => {
                Ok(json!({"preferred_backend": self.catalog.preferred_backend()?}))
            }
            "device.settings.update" => {
                let backend = required(&p, "preferred_backend")?;
                if !self.backends.contains_key(backend) {
                    bail!("unknown backend");
                }
                self.catalog.set_preferred_backend(backend)?;
                Ok(json!({"preferred_backend": backend}))
            }
            "device.backends" => Ok(serde_json::to_value(
                self.backends.values().map(|b| b.info()).collect::<Vec<_>>(),
            )?),
            "cache.status" => {
                let (bytes, entries, limit) = self.cache.stats()?;
                Ok(json!({"bytes":bytes,"entries":entries,"max_bytes":limit}))
            }
            "wallpaper.random" => self.random_wallpaper(p),
            "rotation.status" => Ok(json!(self.rotation()?)),
            "rotation.configure" => self.configure_rotation(p),
            "rotation.stop" => self.stop_rotation(),
            "catalog.duplicates" => self
                .catalog
                .duplicate_groups(number(&p, "limit", 20), number(&p, "offset", 0)),
            "provider.list" => Ok(json!(self.providers.keys().collect::<Vec<_>>())),
            "provider.search" => {
                let provider = self
                    .providers
                    .get(required(&p, "provider")?)
                    .context("unknown provider")?;
                let candidates =
                    provider.search(p["query"].as_str().unwrap_or(""), number(&p, "page", 1))?;
                let ids: Vec<String> = candidates.iter().map(|c| c.external_id.clone()).collect();
                let saved = self.catalog.matching_sources(provider.id(), &ids)?;
                let result: Result<Vec<Value>> = candidates
                    .into_iter()
                    .map(|candidate| {
                        Ok(match saved.get(&candidate.external_id) {
                            Some(wallpaper) => serde_json::to_value(wallpaper)?,
                            None => serde_json::to_value(candidate)?,
                        })
                    })
                    .collect();
                Ok(Value::Array(
                    result?.into_iter().map(|v| self.decorate(v)).collect(),
                ))
            }
            "provider.get" => Ok(serde_json::to_value(
                self.providers
                    .get(required(&p, "provider")?)
                    .context("unknown provider")?
                    .get(required(&p, "external_id")?)?,
            )?),
            "catalog.search" => {
                let rows = self.catalog.search(
                    p["query"].as_str().unwrap_or(""),
                    p["favorite"].as_bool().unwrap_or(false),
                    number(&p, "limit", 100),
                    number(&p, "offset", 0),
                )?;
                Ok(Value::Array(
                    rows.into_iter().map(|w| self.decorate(json!(w))).collect(),
                ))
            }
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
                Ok(self.decorate(serde_json::to_value(wallpaper)?))
            }
            "catalog.remove" => {
                self.catalog.remove(id()?)?;
                Ok(json!({"removed":true}))
            }
            "favorite.add" | "favorite.remove" => {
                let mut wallpaper = self.catalog.get(id()?)?;
                wallpaper.favorite = method == "favorite.add";
                self.catalog.update(&wallpaper)?;
                Ok(self.decorate(serde_json::to_value(wallpaper)?))
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
                Ok(self.decorate(serde_json::to_value(wallpaper)?))
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
                Ok(self.decorate(serde_json::to_value(wallpaper)?))
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
                Ok(self.decorate(serde_json::to_value(wallpaper)?))
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
                let preferred = self.catalog.preferred_backend()?;
                let explicit = p["backend"].as_str().filter(|s| !s.is_empty());
                let backend = match explicit.or(preferred.as_deref()) {
                    Some(name) => self.backends.get(name).context("unknown backend")?,
                    None => self
                        .backends
                        .values()
                        .find(|b| b.info().available)
                        .context("no available wallpaper backend")?,
                };
                backend.apply(&path, p["monitor"].as_str())?;
                for other in self.backends.values() {
                    if other.info().id != backend.info().id {
                        other.deactivate();
                    }
                }
                if let Some(hash) = wallpaper.content_hash.as_deref() {
                    self.catalog.set_setting("last_applied_hash", hash)?;
                }
                if let Some(name) = explicit {
                    self.catalog.set_preferred_backend(name)?;
                }
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
