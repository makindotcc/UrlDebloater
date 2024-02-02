use anyhow::Context;
use futures::Future;
use serde::{Deserialize, Serialize};
use tokio::{fs, time::Instant};
use urlwasher::UrlWasherConfig;

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

pub async fn from_file() -> anyhow::Result<AppConfig> {
    let bytes = fs::read(CONFIG_FILE).await.context("read file")?;
    let config = serde_json::from_slice(&bytes).context("deserialize config")?;
    Ok(config)
}

pub fn save_to_file(config: &AppConfig) -> impl Future<Output = anyhow::Result<()>> {
    let serialized = serde_json::to_vec_pretty(config);
    async move {
        fs::write(CONFIG_FILE, serialized.context("serialize config")?)
            .await
            .context("write config")
    }
}
