use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use super::models::ClearMindConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverySnapshot {
    pub tab_uuid: String,
    pub file_path: Option<String>,
    pub content: String,
    pub disk_hash_at_snapshot: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoadConfigResult {
    Loaded {
        config: ClearMindConfig,
        hash: u64,
    },
    CreatedDefault {
        config: ClearMindConfig,
        hash: u64,
    },
    RecoveredFromCorrupt {
        config: ClearMindConfig,
        hash: u64,
        backup_path: PathBuf,
        error: String,
    },
}

pub fn app_data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("clearmind");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("clearmind");
        }
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("clearmind");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config").join("clearmind");
    }
    PathBuf::from(".clearmind")
}

pub fn config_file_path() -> PathBuf {
    app_data_dir().join("clearmind.toml")
}

pub fn recovery_dir() -> PathBuf {
    app_data_dir().join("recovery")
}

pub fn hash_content(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

pub fn load_config_from_path(path: &Path) -> LoadConfigResult {
    if !path.exists() {
        let cfg = ClearMindConfig::default().clamped();
        let hash = save_config_to_path(&cfg, path).unwrap_or(0);
        return LoadConfigResult::CreatedDefault { config: cfg, hash };
    }

    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            let cfg = ClearMindConfig::default().clamped();
            return LoadConfigResult::RecoveredFromCorrupt {
                config: cfg,
                hash: 0,
                backup_path: path.with_extension("toml.bak"),
                error: e.to_string(),
            };
        }
    };

    match toml::from_str::<ClearMindConfig>(&raw) {
        Ok(cfg) => LoadConfigResult::Loaded {
            config: cfg.clamped(),
            hash: hash_content(&raw),
        },
        Err(err) => {
            let backup_path = path.with_extension("toml.bak");
            let _ = fs::copy(path, &backup_path);
            let cfg = ClearMindConfig::default().clamped();
            let hash = save_config_to_path(&cfg, path).unwrap_or(0);
            LoadConfigResult::RecoveredFromCorrupt {
                config: cfg,
                hash,
                backup_path,
                error: err.to_string(),
            }
        }
    }
}

pub fn save_config_to_path(config: &ClearMindConfig, path: &Path) -> std::io::Result<u64> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = toml::to_string_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, &serialized)?;
    Ok(hash_content(&serialized))
}

pub fn write_recovery_snapshot(
    dir: &Path,
    snapshot: &RecoverySnapshot,
) -> std::io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.json", snapshot.tab_uuid));
    let json = serde_json::to_string_pretty(snapshot)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(&path, json)?;
    Ok(path)
}

pub fn read_all_recovery_snapshots(dir: &Path) -> Vec<RecoverySnapshot> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json")
            && let Ok(raw) = fs::read_to_string(&path)
            && let Ok(snap) = serde_json::from_str::<RecoverySnapshot>(&raw)
        {
            out.push(snap);
        }
    }
    out
}

pub fn delete_recovery_snapshot(dir: &Path, tab_uuid: &str) -> std::io::Result<()> {
    let path = dir.join(format!("{tab_uuid}.json"));
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("clearmind_test_{name}_{nanos}"))
    }

    #[test]
    fn toml_round_trip_and_clamping() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("clearmind.toml");

        let mut cfg = ClearMindConfig::default();
        cfg.fonts.font_size = 99.0;
        cfg.margin.content_max_width = 100.0;
        cfg.view.zoom = 5.0;

        let clamped = cfg.clamped();
        assert_eq!(clamped.fonts.font_size, 28.0);
        assert_eq!(clamped.margin.content_max_width, 560.0);
        assert_eq!(clamped.view.zoom, 2.0);

        save_config_to_path(&clamped, &path).unwrap();
        match load_config_from_path(&path) {
            LoadConfigResult::Loaded { config, .. } => assert_eq!(config, clamped),
            other => panic!("Expected Loaded, got {other:?}"),
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_toml_creates_bak_and_writes_defaults() {
        let dir = temp_dir("corrupt");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("clearmind.toml");
        fs::write(&path, "not valid toml = [[[").unwrap();

        match load_config_from_path(&path) {
            LoadConfigResult::RecoveredFromCorrupt {
                config,
                backup_path,
                ..
            } => {
                assert!(backup_path.exists());
                assert_eq!(config, ClearMindConfig::default());
            }
            other => panic!("Expected RecoveredFromCorrupt, got {other:?}"),
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
