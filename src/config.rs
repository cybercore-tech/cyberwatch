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
    dirs::home_dir()
        .unwrap_or_default()
        .join(".config/cyberwatch/config.json")
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
