use std::{ops::Add, time::Duration};

use anyhow::Context;
use futures::Future;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tokio::{fs, time::Instant};
use urlwasher::UrlWasherConfig;

const CONFIG_FILE: &str = "config.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub url_washer: UrlWasherConfig,
    pub enable_clipboard_patcher: bool,
    #[serde(
        serialize_with = "serialize_pause_instant",
        deserialize_with = "deserialize_pause_instant"
    )]
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

pub fn serialize_pause_instant<S>(
    paused_until: &Option<Instant>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let duration_left = paused_until.map(|i| i.duration_since(Instant::now()));
    duration_left.serialize(serializer)
}

pub fn deserialize_pause_instant<'de, D>(deserializer: D) -> Result<Option<Instant>, D::Error>
where
    D: Deserializer<'de>,
{
    let duration_left = Option::<Duration>::deserialize(deserializer)?;
    Ok(duration_left.map(|duration| Instant::now().add(duration)))
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
