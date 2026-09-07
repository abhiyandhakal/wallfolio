use anyhow::{bail, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallpaper {
    pub id: String,
    pub title: String,
    pub provider: String,
    pub external_id: String,
    pub source: String,
    pub thumbnail: Option<String>,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub local_path: Option<String>,
    pub content_hash: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

pub struct Catalog(Connection);
impl Catalog {
    pub fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path)?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: u32 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > 1 {
            bail!("catalog schema {version} requires a newer Wallfolio version");
        }
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        if version == 0 {
            db.execute_batch(
                "BEGIN IMMEDIATE;
              CREATE TABLE IF NOT EXISTS wallpapers (
                id TEXT PRIMARY KEY, provider TEXT NOT NULL, external_id TEXT NOT NULL,
                document TEXT NOT NULL, UNIQUE(provider, external_id));
              PRAGMA user_version=1;
              COMMIT;",
            )?;
        }
        Ok(Self(db))
    }
    pub fn add(&self, mut wallpaper: Wallpaper) -> Result<Wallpaper> {
        let mut stmt = self
            .0
            .prepare("SELECT document FROM wallpapers WHERE provider=?1 AND external_id=?2")?;
        let mut rows = stmt.query(params![wallpaper.provider, wallpaper.external_id])?;
        if let Some(row) = rows.next()? {
            return Ok(serde_json::from_str(&row.get::<_, String>(0)?)?);
        }
        wallpaper.id = Uuid::new_v4().to_string();
        self.0.execute(
            "INSERT INTO wallpapers VALUES (?1,?2,?3,?4)",
            params![
                wallpaper.id,
                wallpaper.provider,
                wallpaper.external_id,
                serde_json::to_string(&wallpaper)?
            ],
        )?;
        Ok(wallpaper)
    }
    pub fn get(&self, id: &str) -> Result<Wallpaper> {
        let document: String =
            self.0
                .query_row("SELECT document FROM wallpapers WHERE id=?1", [id], |r| {
                    r.get(0)
                })?;
        Ok(serde_json::from_str(&document)?)
    }
    pub fn search(
        &self,
        query: &str,
        favorite: bool,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Wallpaper>> {
        let mut stmt = self.0.prepare("SELECT document FROM wallpapers WHERE
          (instr(lower(json_extract(document,'$.title')),lower(?1)) > 0 OR
           EXISTS (SELECT 1 FROM json_each(document,'$.tags') WHERE instr(lower(value),lower(?1)) > 0))
          AND (?2=0 OR json_extract(document,'$.favorite')=1) ORDER BY rowid DESC LIMIT ?3 OFFSET ?4")?;
        let rows = stmt.query_map(params![query, favorite, limit.clamp(1, 200), offset], |r| {
            r.get::<_, String>(0)
        })?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn update(&self, wallpaper: &Wallpaper) -> Result<()> {
        if self.0.execute(
            "UPDATE wallpapers SET document=?2 WHERE id=?1",
            params![wallpaper.id, serde_json::to_string(wallpaper)?],
        )? == 0
        {
            bail!("wallpaper not found");
        }
        Ok(())
    }
    pub fn remove(&self, id: &str) -> Result<()> {
        if self.0.execute("DELETE FROM wallpapers WHERE id=?1", [id])? == 0 {
            bail!("wallpaper not found");
        }
        Ok(())
    }
    pub fn hash_references(&self, hash: &str) -> Result<u64> {
        Ok(self.0.query_row("SELECT count(*) FROM wallpapers WHERE json_extract(document,'$.content_hash')=?1 AND json_extract(document,'$.local_path') IS NOT NULL", [hash], |r| r.get(0))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn candidate() -> Wallpaper {
        Wallpaper {
            id: String::new(),
            title: "Mountain".into(),
            provider: "local".into(),
            external_id: "one".into(),
            source: "/tmp/one.png".into(),
            thumbnail: None,
            tags: vec!["dark".into()],
            favorite: false,
            local_path: None,
            content_hash: None,
            width: None,
            height: None,
        }
    }
    #[test]
    fn newer_schema_is_not_modified() -> Result<()> {
        let path = std::env::temp_dir().join(format!("wallfolio-schema-{}.db", Uuid::new_v4()));
        let db = Connection::open(&path)?;
        db.execute_batch("PRAGMA user_version=2")?;
        assert!(Catalog::open(&path).is_err());
        assert_eq!(
            db.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))?,
            2
        );
        drop(db);
        std::fs::remove_file(path)?;
        Ok(())
    }
    #[test]
    fn identity_search_and_lifecycle() -> Result<()> {
        let c = Catalog::open(Path::new(":memory:"))?;
        let mut a = c.add(candidate())?;
        assert_eq!(a.id, c.add(candidate())?.id);
        assert_eq!(c.search("DARK", false, 10, 0)?.len(), 1);
        assert!(c.search("%", false, 10, 0)?.is_empty());
        a.favorite = true;
        c.update(&a)?;
        assert_eq!(c.search("", true, 10, 0)?.len(), 1);
        c.remove(&a.id)?;
        assert!(c.get(&a.id).is_err());
        Ok(())
    }
}
