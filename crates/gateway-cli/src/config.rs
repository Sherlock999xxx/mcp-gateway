use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliConfig {
    #[serde(default)]
    pub admin_base: Option<String>,
    #[serde(default)]
    pub data_base: Option<String>,
    #[serde(default)]
    pub admin_token: Option<String>,
}

pub fn default_config_path() -> anyhow::Result<PathBuf> {
    let base = if let Ok(v) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(v)
    } else {
        let home = std::env::var("HOME").context("HOME is not set")?;
        PathBuf::from(home).join(".config")
    };
    Ok(base.join("unrelated").join("gateway-admin.json"))
}

pub fn load_config(path: &Path) -> anyhow::Result<CliConfig> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(CliConfig::default()),
        Err(e) => return Err(e).with_context(|| format!("read config {}", path.display())),
    };
    let cfg: CliConfig =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    Ok(cfg)
}

pub fn save_config(path: &Path, cfg: &CliConfig) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(cfg).context("serialize config as json")?;
    std::fs::write(path, bytes).with_context(|| format!("write config {}", path.display()))?;
    Ok(())
}
