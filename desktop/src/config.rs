use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    time::{Instant, SystemTime},
};

const CONFIG_FILE: &str = "config.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub url_washer: UrlWasherConfig,
    pub enable_clipboard_patcher: bool,
    #[serde(skip)]
    pub clipboard_patcher_paused_until: Option<Instant>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            url_washer: UrlWasherConfig::default(),
            enable_clipboard_patcher: true,
            clipboard_patcher_paused_until: None,
        }
    }
}

pub fn from_file() -> anyhow::Result<AppConfig> {
    let bytes = fs::read(CONFIG_FILE).context("read file")?;
    let config = serde_json::from_slice(&bytes).context("deserialize config")?;
    Ok(config)
}

pub fn save_to_file(config: &AppConfig) -> anyhow::Result<()> {
    fs::write(
        CONFIG_FILE,
        serde_json::to_vec_pretty(config).context("serialize config")?,
    )
    .context("write config")
}

pub fn modified_time() -> anyhow::Result<SystemTime> {
    fs::metadata(CONFIG_FILE)
        .context("read config metadata")?
        .modified()
        .context("read config modified time")
}
