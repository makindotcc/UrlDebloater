use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use url::Url;
use urlwasher::WasherConfig;

pub const CONFIG_PATH: &str = "config.json";

pub fn load_or_default() -> anyhow::Result<Config> {
    match load() {
        Ok(config) => Ok(config),
        Err(err) if err.kind() == ErrorKind::NotFound => {
            let config = Config::default();
            let serialized_config =
                serde_json::to_string_pretty(&config).context("serialize default config")?;
            std::fs::write(CONFIG_PATH, serialized_config).context("write default config")?;
            Ok(config)
        }
        Err(err) => Err(err).context("read config file"),
    }
}

pub fn load() -> Result<Config, std::io::Error> {
    let config_str = std::fs::read_to_string(CONFIG_PATH)?;
    serde_json::from_str(&config_str).map_err(std::io::Error::other)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub version: u32,
    pub redirect_wash_policy: RedirectWashPolicy,
    pub clipboard_debloater: bool,
    pub washer_config: WasherConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            redirect_wash_policy: RedirectWashPolicy::Locally,
            clipboard_debloater: true,
            washer_config: WasherConfig::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub enum RedirectWashPolicy {
    /// Do not resolve redirection.
    Ignore,
    /// Resolve redirection locally.
    ///
    /// Exposes your IP address that can be corellated with you.
    Locally,
    /// Resolve redirection using urldebloater-mixer.
    ///
    /// Exposes link to person who is running mixer instance you set
    /// (not so scary for tiktoks tho).
    ViaMixer { mixer_url: Url },
}
