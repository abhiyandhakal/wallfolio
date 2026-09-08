use crate::{normalize_tags, Application, Result};
use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Rotation {
    pub enabled: bool,
    pub interval_seconds: u64,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub monitor: Option<String>,
    pub next_run: Option<u64>,
    pub last_run: Option<u64>,
    pub last_error: Option<String>,
}
impl Default for Rotation {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_seconds: 1800,
            favorite: false,
            tags: vec![],
            monitor: None,
            next_run: None,
            last_run: None,
            last_error: None,
        }
    }
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
impl Application {
    pub fn rotation(&self) -> Result<Rotation> {
        self.catalog
            .setting("rotation")?
            .map(|s| Ok(serde_json::from_str(&s)?))
            .unwrap_or_else(|| Ok(Rotation::default()))
    }
    fn save_rotation(&self, rotation: &Rotation) -> Result<()> {
        self.catalog
            .set_setting("rotation", &serde_json::to_string(rotation)?)
    }
    pub(crate) fn configure_rotation(&self, params: Value) -> Result<Value> {
        let mut config: Rotation = serde_json::from_value(params)?;
        if !(10..=604800).contains(&config.interval_seconds) {
            bail!("rotation interval must be 10 seconds to 7 days");
        }
        config.tags = normalize_tags(config.tags)?;
        config.next_run = config.enabled.then(|| now() + config.interval_seconds);
        config.last_run = None;
        config.last_error = None;
        self.save_rotation(&config)?;
        Ok(json!(config))
    }
    pub(crate) fn stop_rotation(&self) -> Result<Value> {
        let mut config = self.rotation()?;
        config.enabled = false;
        config.next_run = None;
        self.save_rotation(&config)?;
        Ok(json!(config))
    }
    /// Called by the daemon while idle and between requests. An overdue schedule
    /// runs once after restart, never once per missed interval.
    pub fn tick(&self, timestamp: u64) -> Result<()> {
        let mut config = self.rotation()?;
        if !config.enabled || config.next_run.is_some_and(|due| due > timestamp) {
            return Ok(());
        }
        config.next_run = Some(timestamp + config.interval_seconds);
        config.last_run = Some(timestamp);
        config.last_error = self
            .random_wallpaper(
                json!({"favorite":config.favorite,"tags":config.tags,"monitor":config.monitor}),
            )
            .err()
            .map(|e| format!("{e:#}"));
        self.save_rotation(&config)
    }
    pub(crate) fn random_wallpaper(&self, params: Value) -> Result<Value> {
        let tags: Vec<String> =
            serde_json::from_value(params.get("tags").cloned().unwrap_or_else(|| json!([])))?;
        let tags = normalize_tags(tags)?;
        let previous = self.catalog.setting("last_applied_hash")?;
        let wallpaper = self
            .catalog
            .random_downloaded(
                params["favorite"].as_bool().unwrap_or(false),
                &tags,
                previous.as_deref(),
            )?
            .context("no downloaded wallpapers match; download a library image first")?;
        let mut apply_params = params;
        apply_params["id"] = json!(wallpaper.id);
        let mut result = self.dispatch("wallpaper.apply", apply_params)?;
        result["wallpaper"] = json!(wallpaper);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::Path,
        sync::{Arc, Mutex},
    };
    use wallfolio_backend_api::{BackendInfo, WallpaperBackend};
    use wallfolio_catalog::Wallpaper;
    struct Backend(Arc<Mutex<Vec<String>>>);
    impl WallpaperBackend for Backend {
        fn info(&self) -> BackendInfo {
            BackendInfo {
                id: "test".into(),
                available: true,
                per_monitor: true,
                transitions: false,
            }
        }
        fn apply(&self, path: &Path, _: Option<&str>) -> Result<()> {
            self.0
                .lock()
                .unwrap()
                .push(path.to_string_lossy().into_owned());
            Ok(())
        }
    }
    #[test]
    fn random_rotation_restart_filters_and_duplicate_groups() -> Result<()> {
        let root = std::env::temp_dir().join(format!("wallfolio-rotation-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let mut app = Application::open(root.clone())?;
        let calls = Arc::new(Mutex::new(Vec::new()));
        app.register_backend(Box::new(Backend(calls.clone())));
        app.catalog.set_preferred_backend("test")?;
        assert!(app.random_wallpaper(json!({})).is_err());
        for (name, hash, favorite, local) in [
            ("a", "a", true, true),
            ("b", "b", true, true),
            ("c", "a", false, true),
            ("remote", "remote", true, false),
            ("missing", "missing", true, true),
        ] {
            let path = root.join(format!("{name}.png"));
            if local && name != "missing" {
                std::fs::write(&path, b"fixture")?;
            }
            app.catalog.add(Wallpaper {
                id: String::new(),
                title: name.into(),
                provider: "local".into(),
                external_id: name.into(),
                source: path.to_string_lossy().into_owned(),
                thumbnail: None,
                tags: vec!["dark".into()],
                favorite,
                local_path: local.then(|| path.to_string_lossy().into_owned()),
                content_hash: local.then(|| hash.into()),
                width: None,
                height: None,
            })?;
        }
        assert!(app.random_wallpaper(json!({"tags":["absent"]})).is_err());
        let first = app.random_wallpaper(json!({"favorite":true,"tags":[" dark ", "", "dark"]}))?;
        let second = app.random_wallpaper(json!({"favorite":true,"tags":["dark"]}))?;
        assert_ne!(
            first["wallpaper"]["content_hash"],
            second["wallpaper"]["content_hash"]
        );
        assert!(second["wallpaper"]["favorite"].as_bool().unwrap());
        let groups = app.catalog.duplicate_groups(20, 0)?;
        assert_eq!(groups[0]["count"], 2);
        assert_eq!(groups[0]["items"].as_array().unwrap().len(), 2);
        let settings =
            app.configure_rotation(json!({"enabled":true,"interval_seconds":10,"favorite":true}))?;
        let due = settings["next_run"].as_u64().unwrap();
        calls.lock().unwrap().clear();
        app.tick(due - 1)?;
        assert!(calls.lock().unwrap().is_empty());
        app.tick(due)?;
        app.tick(due)?;
        assert_eq!(calls.lock().unwrap().len(), 1);
        drop(app);
        let mut app = Application::open(root.clone())?;
        app.register_backend(Box::new(Backend(calls.clone())));
        app.tick(due + 100)?;
        assert_eq!(calls.lock().unwrap().len(), 2); // once, not ten missed intervals
        app.stop_rotation()?;
        app.tick(due + 1000)?;
        assert_eq!(calls.lock().unwrap().len(), 2);
        assert!(app
            .configure_rotation(json!({"enabled":true,"interval_seconds":1}))
            .is_err());
        assert!(app
            .random_wallpaper(json!({"tags":vec!["tag";101]}))
            .is_err());
        let failed = app.configure_rotation(
            json!({"enabled":true,"interval_seconds":10,"tags":[" absent ","absent",""]}),
        )?;
        assert_eq!(failed["tags"], json!(["absent"]));
        let due = failed["next_run"].as_u64().unwrap();
        app.tick(due)?;
        let status = app.rotation()?;
        assert!(status
            .last_error
            .unwrap()
            .contains("no downloaded wallpapers match"));
        assert_eq!(status.next_run, Some(due + 10));
        assert_eq!(calls.lock().unwrap().len(), 2);
        drop(app);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
