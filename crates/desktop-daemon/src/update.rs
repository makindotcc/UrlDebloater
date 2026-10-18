use crate::app::AppEvent;
use cargo_packager_updater::{
    WindowsConfig, WindowsUpdateInstallMode, check_update, semver::Version,
};
use log::{error, info};
#[cfg(target_os = "linux")]
use std::path::Path;
use std::{
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::mpsc,
    task::{JoinError, JoinHandle},
    time::{Interval, interval_at},
};
use url::Url;

pub const RELEASES_URL: &str = "https://github.com/makindotcc/UrlDebloater/releases/latest";

const UPDATE_ENDPOINT: &str =
    "https://github.com/makindotcc/UrlDebloater/releases/latest/download/latest.json";
const UPDATE_PUBKEY: &str = include_str!(concat!(
    env!("CARGO_WORKSPACE_DIR"),
    "assets/update-public-key.txt"
));
const UPDATE_CHECK_DELAY: Duration = Duration::from_secs(30);
const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

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

pub enum UpdateCommand {
    Check,
    Install,
    SetEnabled(bool),
}

pub struct UpdateManager {
    status: UpdateStatus,
    check_task: Option<JoinHandle<Result<Option<String>, String>>>,
    install_task: Option<JoinHandle<Result<(), String>>>,
    timer: Interval,
    manual_check: bool,
    enabled: bool,
    event_tx: mpsc::Sender<AppEvent>,
}

impl UpdateManager {
    pub fn new(event_tx: mpsc::Sender<AppEvent>, enabled: bool) -> Self {
        Self {
            status: UpdateStatus::Idle,
            check_task: None,
            install_task: None,
            timer: interval_at(
                tokio::time::Instant::now() + UPDATE_CHECK_DELAY,
                UPDATE_CHECK_INTERVAL,
            ),
            manual_check: false,
            enabled,
            event_tx,
        }
    }

    pub async fn run(mut self, mut commands: mpsc::Receiver<UpdateCommand>) {
        loop {
            tokio::select! {
                Some(command) = commands.recv() => match command {
                    UpdateCommand::Check => self.start_check(true),
                    UpdateCommand::Install => self.start_install(),
                    UpdateCommand::SetEnabled(enabled) => self.enabled = enabled,
                },
                _ = self.timer.tick(), if self.should_check() => self.start_check(false),
                result = async {
                    match self.check_task.as_mut() {
                        Some(task) => task.await,
                        None => std::future::pending().await,
                    }
                }, if self.check_task.is_some() => {
                    self.check_task = None;
                    self.finish_check(result);
                }
                result = async {
                    match self.install_task.as_mut() {
                        Some(task) => task.await,
                        None => std::future::pending().await,
                    }
                }, if self.install_task.is_some() => {
                    self.install_task = None;
                    self.finish_install(result);
                }
                else => break,
            }
        }
    }

    fn should_check(&self) -> bool {
        self.enabled
            && self.check_task.is_none()
            && self.install_task.is_none()
            && !self.status.is_busy()
            && !self.status.has_update()
    }

    fn start_check(&mut self, manual: bool) {
        if self.check_task.is_some()
            || self.install_task.is_some()
            || self.status.is_busy()
            || self.status.has_update()
        {
            return;
        }

        self.manual_check = manual;
        self.set_status(UpdateStatus::Checking);
        self.check_task = Some(tokio::task::spawn_blocking(check));
    }

    fn finish_check(&mut self, result: Result<Result<Option<String>, String>, JoinError>) {
        let manual = self.manual_check;
        self.manual_check = false;

        match result {
            Ok(Ok(Some(version))) => {
                info!("Update available: {version}");
                self.set_status(UpdateStatus::Available { version });
            }
            Ok(Ok(None)) => self.set_status(UpdateStatus::Idle),
            Ok(Err(err)) => {
                error!("Failed to check for updates: {err}");
                if manual {
                    self.set_status(UpdateStatus::Failed {
                        version: self.status.version().map(str::to_owned),
                        error: err,
                    });
                } else if !self.status.has_update() {
                    self.set_status(UpdateStatus::Idle);
                }
            }
            Err(err) => {
                error!("Update check task failed: {err}");
                if !self.status.has_update() {
                    self.set_status(UpdateStatus::Idle);
                }
            }
        }
    }

    fn start_install(&mut self) {
        if self.install_task.is_some() || self.status.is_busy() {
            return;
        }
        let Some(version) = self.status.version().map(str::to_owned) else {
            return;
        };
        if !self_update_supported() {
            if let Err(err) = open_releases_page() {
                error!("Failed to open the release page: {err}");
            }
            return;
        }

        self.set_status(UpdateStatus::Downloading {
            version,
            downloaded: 0,
            total: None,
        });
        let event_tx = self.event_tx.clone();
        self.install_task = Some(tokio::task::spawn_blocking(move || {
            install(move |downloaded, total| {
                let _ = event_tx.try_send(AppEvent::UpdateProgress { downloaded, total });
            })
        }));
    }

    fn finish_install(&mut self, result: Result<Result<(), String>, JoinError>) {
        match result {
            Ok(Ok(())) => {
                let version = self.status.version().unwrap_or_default().to_owned();
                self.set_status(UpdateStatus::Installing { version });
                relaunch();
            }
            Ok(Err(err)) => {
                error!("Failed to install update: {err}");
                self.set_status(UpdateStatus::Failed {
                    version: self.status.version().map(str::to_owned),
                    error: err,
                });
            }
            Err(err) => {
                error!("Update install task failed: {err}");
                self.set_status(UpdateStatus::Failed {
                    version: self.status.version().map(str::to_owned),
                    error: err.to_string(),
                });
            }
        }
    }

    fn set_status(&mut self, status: UpdateStatus) {
        self.status = status.clone();
        let _ = self
            .event_tx
            .try_send(AppEvent::UpdateStatusChanged(status));
    }
}

pub fn check() -> Result<Option<String>, String> {
    let current = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|err| err.to_string())?;
    check_update(current, updater_config()?)
        .map_err(|err| err.to_string())
        .map(|update| update.map(|update| update.version))
}

fn install<F>(on_progress: F) -> Result<(), String>
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

fn updater_config() -> Result<cargo_packager_updater::Config, String> {
    Ok(cargo_packager_updater::Config {
        endpoints: vec![Url::parse(UPDATE_ENDPOINT).map_err(|err| err.to_string())?],
        pubkey: UPDATE_PUBKEY.trim().to_owned(),
        windows: Some(WindowsConfig {
            install_mode: Some(WindowsUpdateInstallMode::Passive),
            ..Default::default()
        }),
        ..Default::default()
    })
}
