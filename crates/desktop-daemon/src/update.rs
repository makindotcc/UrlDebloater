use cargo_packager_updater::{
    Config, WindowsConfig, WindowsUpdateInstallMode, check_update, semver::Version,
};
use std::{
    path::Path,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use url::Url;

pub const RELEASES_URL: &str = "https://github.com/makindotcc/UrlDebloater/releases/latest";

const UPDATE_ENDPOINT: &str =
    "https://github.com/makindotcc/UrlDebloater/releases/latest/download/latest.json";
const UPDATE_PUBKEY: &str = include_str!(concat!(
    env!("CARGO_WORKSPACE_DIR"),
    "assets/update-public-key.txt"
));

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    Available {
        version: String,
    },
    Downloading {
        version: String,
        downloaded: u64,
        total: Option<u64>,
    },
    Installing {
        version: String,
    },
    Failed {
        version: Option<String>,
        error: String,
    },
}

impl UpdateStatus {
    pub fn has_update(&self) -> bool {
        matches!(
            self,
            Self::Available { .. }
                | Self::Downloading { .. }
                | Self::Installing { .. }
                | Self::Failed {
                    version: Some(_),
                    ..
                }
        )
    }

    pub fn version(&self) -> Option<&str> {
        match self {
            Self::Available { version }
            | Self::Downloading { version, .. }
            | Self::Installing { version } => Some(version),
            Self::Failed { version, .. } => version.as_deref(),
            Self::Idle | Self::Checking => None,
        }
    }

    pub fn is_busy(&self) -> bool {
        matches!(
            self,
            Self::Checking | Self::Downloading { .. } | Self::Installing { .. }
        )
    }
}

pub fn check() -> Result<Option<String>, String> {
    let current = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|err| err.to_string())?;
    check_update(current, updater_config()?)
        .map_err(|err| err.to_string())
        .map(|update| update.map(|update| update.version))
}

pub fn install<F>(on_progress: F) -> Result<(), String>
where
    F: Fn(u64, Option<u64>),
{
    let current = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|err| err.to_string())?;
    let update = check_update(current, updater_config()?)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| String::from("No update available"))?;

    let downloaded = Arc::new(AtomicU64::new(0));
    let counter = Arc::clone(&downloaded);
    update
        .download_and_install_extended(
            move |chunk, total| {
                let downloaded = counter.fetch_add(chunk as u64, Ordering::Relaxed) + chunk as u64;
                on_progress(downloaded, total);
            },
            || {},
        )
        .map_err(|err| err.to_string())
}

pub fn self_update_supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("APPIMAGE").is_some() && !Path::new("/etc/NIXOS").exists()
    }

    #[cfg(target_os = "macos")]
    {
        app_bundle().is_some()
    }

    #[cfg(target_os = "windows")]
    {
        true
    }
}

pub fn open_releases_page() -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", ""]);
        command
    };

    command.arg(RELEASES_URL);
    command.spawn().map(|_| ())
}

pub fn relaunch() -> ! {
    #[cfg(target_os = "macos")]
    if let Some(bundle) = app_bundle() {
        let _ = Command::new("/bin/sh")
            .args(["-c", "sleep 1; open -n \"$1\"", "sh"])
            .arg(bundle)
            .spawn();
    }

    #[cfg(target_os = "linux")]
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        let _ = Command::new("/bin/sh")
            .args(["-c", "sleep 1; exec \"$1\"", "sh"])
            .arg(appimage)
            .spawn();
    }

    std::process::exit(0);
}

#[cfg(target_os = "macos")]
fn app_bundle() -> Option<std::path::PathBuf> {
    std::env::current_exe()
        .ok()?
        .ancestors()
        .find_map(|path| (path.extension()?.to_str()? == "app").then(|| path.to_path_buf()))
}

fn updater_config() -> Result<Config, String> {
    Ok(Config {
        endpoints: vec![Url::parse(UPDATE_ENDPOINT).map_err(|err| err.to_string())?],
        pubkey: UPDATE_PUBKEY.trim().to_owned(),
        windows: Some(WindowsConfig {
            install_mode: Some(WindowsUpdateInstallMode::Passive),
            ..Default::default()
        }),
        ..Default::default()
    })
}
