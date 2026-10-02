use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// User-editable settings at `~/.config/cyberwatch/config.json`. Discovery
/// is automatic (see `discover.rs`) — this just lets you patch its result:
/// add a unit the ExecStart-path heuristic missed (a plain shell-script
/// `ExecStart=`, say), or drop one you don't want watched.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Config {
    pub extra_units: Vec<String>,
    pub ignore: Vec<String>,
}

pub fn config_path() -> PathBuf {
    config_path_in(&dirs::home_dir().unwrap_or_default())
}

/// The config file under an explicit home directory (see
/// `discover::discover_units_in`).
pub fn config_path_in(home: &std::path::Path) -> PathBuf {
    home.join(".config/cyberwatch/config.json")
}

/// Reads an existing config without creating one — for read-only callers.
pub fn load_from(path: &std::path::Path) -> Result<Config> {
    if !path.exists() {
        return Ok(Config::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub fn load_or_init() -> Result<Config> {
    let path = config_path();
    if path.exists() {
        let data = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&data)?)
    } else {
        let cfg = Config::default();
        save(&cfg)?;
        Ok(cfg)
    }
}

pub fn save(cfg: &Config) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(cfg)?)?;
    Ok(())
}
