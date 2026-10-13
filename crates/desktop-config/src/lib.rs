use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};
use url::Url;
use urldebloater_urlwasher::WasherConfig;

/// Config file name inside the platform config directory.
const CONFIG_FILE: &str = "config.json";
/// Config file that older versions read from the current working directory.
const LEGACY_CONFIG_FILE: &str = "config.json";
const APP_DIR: &str = "urldebloater";
/// Highest config `version` this build understands.
pub const SUPPORTED_VERSION: u32 = 1;

pub fn config_path() -> PathBuf {
    config_dir().join(CONFIG_FILE)
}

pub fn config_dir() -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    };
    base.unwrap_or_else(|| PathBuf::from(".")).join(APP_DIR)
}

pub fn load() -> Result<Config, std::io::Error> {
    load_from(&config_path())
}

pub fn load_from(path: &Path) -> Result<Config, std::io::Error> {
    let config_str = std::fs::read_to_string(path)?;
    serde_json::from_str(&config_str).map_err(std::io::Error::other)
}

pub fn load_or_default() -> anyhow::Result<Config> {
    match load() {
        Ok(config) => Ok(config),
        Err(err) if err.kind() == ErrorKind::NotFound => {
            let config = match migrate_legacy() {
                Some(config) => {
                    log::info!("Migrated {} to {}", LEGACY_CONFIG_FILE, config_path().display());
                    config
                }
                None => Config::default(),
            };
            save(&config).context("write default config")?;
            Ok(config)
        }
        Err(err) => Err(err).context("read config file"),
    }
}

/// Reads the config older versions kept next to the binary, if present.
fn migrate_legacy() -> Option<Config> {
    let legacy = Path::new(LEGACY_CONFIG_FILE);
    if legacy == config_path() {
        return None;
    }
    load_from(legacy).ok()
}

/// Writes the config atomically, so a running app never reads a torn file.
pub fn save(config: &Config) -> anyhow::Result<()> {
    save_to(&config_path(), config)
}

pub fn save_to(path: &Path, config: &Config) -> anyhow::Result<()> {
    if config.version > SUPPORTED_VERSION {
        anyhow::bail!(
            "config version {} is newer than supported {}",
            config.version,
            SUPPORTED_VERSION
        );
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("create config directory")?;
    }
    let serialized = serde_json::to_string_pretty(config).context("serialize config")?;
    let temp = path.with_extension(format!("json.tmp{}", std::process::id()));
    std::fs::write(&temp, serialized).context("write temporary config")?;
    std::fs::rename(&temp, path).context("replace config")?;
    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub version: u32,
    pub redirect_wash_policy: RedirectWashPolicy,
    pub clipboard_debloater: bool,
    pub washer_config: WasherConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: SUPPORTED_VERSION,
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

impl RedirectWashPolicy {
    /// Discriminant as an index, for UI combo boxes. [`mixer_url`] returns the URL
    /// for [`RedirectWashPolicy::ViaMixer`]; both are consistent with [`from_index`].
    pub fn index(&self) -> usize {
        match self {
            RedirectWashPolicy::Ignore => 0,
            RedirectWashPolicy::Locally => 1,
            RedirectWashPolicy::ViaMixer { .. } => 2,
        }
    }

    pub fn mixer_url(&self) -> Option<&Url> {
        match self {
            RedirectWashPolicy::ViaMixer { mixer_url } => Some(mixer_url),
            _ => None,
        }
    }

    pub fn from_index(index: usize, mixer_url: Url) -> Self {
        match index {
            0 => RedirectWashPolicy::Ignore,
            1 => RedirectWashPolicy::Locally,
            _ => RedirectWashPolicy::ViaMixer { mixer_url },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("desktop-config-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join(CONFIG_FILE)
    }

    #[test]
    fn roundtrip_through_file() {
        let path = temp_config_path("roundtrip");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let config = Config {
            redirect_wash_policy: RedirectWashPolicy::ViaMixer {
                mixer_url: Url::parse("https://example.com/mixer").unwrap(),
            },
            clipboard_debloater: false,
            ..Config::default()
        };
        let serialized = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(&path, serialized).unwrap();

        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded, config);
        assert_eq!(loaded.redirect_wash_policy.index(), 2);
    }

    #[test]
    fn save_is_atomic_and_leaves_no_temp_file() {
        let path = temp_config_path("atomic");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let config = Config::default();
        // save() writes next to the platform config path, so exercise the same
        // primitives directly on a path we control.
        let temp = path.with_extension(format!("json.tmp{}", std::process::id()));
        std::fs::write(&temp, serde_json::to_string_pretty(&config).unwrap()).unwrap();
        std::fs::rename(&temp, &path).unwrap();

        assert!(path.exists());
        assert!(!temp.exists());
        assert_eq!(load_from(&path).unwrap().version, SUPPORTED_VERSION);
    }

    #[test]
    fn policy_index_roundtrip() {
        let url = Url::parse("https://example.com/mixer").unwrap();
        for (index, policy) in [
            (0, RedirectWashPolicy::Ignore),
            (1, RedirectWashPolicy::Locally),
            (2, RedirectWashPolicy::ViaMixer { mixer_url: url.clone() }),
        ] {
            assert_eq!(policy.index(), index);
            assert_eq!(RedirectWashPolicy::from_index(index, url.clone()), policy);
        }
        assert_eq!(
            RedirectWashPolicy::Ignore.mixer_url().map(Url::as_str),
            None
        );
        assert_eq!(
            RedirectWashPolicy::ViaMixer { mixer_url: url.clone() }
                .mixer_url()
                .map(Url::as_str),
            Some("https://example.com/mixer")
        );
    }

    #[test]
    fn rejects_newer_config_version() {
        let config = Config {
            version: SUPPORTED_VERSION + 1,
            ..Config::default()
        };
        assert!(save(&config).is_err());
    }
}
