use crate::update::{UpdateCommand, UpdateManager, UpdateStatus};
use crate::{clipboard::ClipboardService, config_integration, tray};
use anyhow::Context;
use anyhow::anyhow;
use arboard::Clipboard;
use log::{error, info};
use std::{
    pin::Pin,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use tokio::{select, sync::mpsc, time::Interval};
use urldebloater_desktop_config::{Config, RedirectWashPolicy, config_dir};
use urldebloater_urlwasher::{
    UrlWasher, WasherConfig,
    clearurls::{self, ClearUrls, ReqwestCatalogLoader},
    redirect_resolver::{
        DefaultRedirectResolver, WithMixerRedirectResolver, WithoutMixerRedirectResolver,
    },
    text_washer::TextWasher,
};

pub(crate) const CLIPBOARD_PAUSE_DURATION: Duration = Duration::from_secs(30);

struct App {
    config: Config,
    clipboard_service: ClipboardService,
    #[cfg(feature = "config-gui-integration")]
    config_gui_running: Arc<std::sync::atomic::AtomicBool>,
    text_washer: Arc<TextWasher>,
    wash_task: Option<WashTask>,
    app_event_tx: mpsc::Sender<AppEvent>,
    clipboard_pause_state: Option<ClipboardPauseState>,
    update_command_tx: mpsc::Sender<UpdateCommand>,
}

struct ClipboardPauseState {
    until: Instant,
    progress_timer: Interval,
}

impl App {
    async fn new(app_event_tx: mpsc::Sender<AppEvent>) -> anyhow::Result<Self> {
        let config = urldebloater_desktop_config::load_or_default()?;
        let clipboard = Clipboard::new().context("initialize clipboard")?;
        let clipboard_service = ClipboardService::new(clipboard);
        let text_washer = app_text_washer(&config, None).await;
        let (update_command_tx, update_command_rx) = mpsc::channel(8);
        let update_manager = UpdateManager::new(app_event_tx.clone(), config.check_for_updates);
        tokio::spawn(update_manager.run(update_command_rx));
        Ok(Self {
            text_washer: Arc::new(text_washer),
            config,
            clipboard_service,
            #[cfg(feature = "config-gui-integration")]
            config_gui_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            wash_task: None,
            clipboard_pause_state: None,
            app_event_tx,
            update_command_tx,
        })
    }

    fn clipboard_debloater_enabled(&self) -> bool {
        self.config.clipboard_debloater
    }

    async fn run(&mut self, mut app_command_rx: mpsc::Receiver<AppCommand>) {
        loop {
            select! {
                Some(tray_command) = app_command_rx.recv()=> {
                    match tray_command {
                        AppCommand::ClearClipboard => self.clear_clipboard(WashMode::Manual),
                        AppCommand::ToggleClipboardDebloater => self.toggle_clipboard_debloater(),
                        AppCommand::ReloadConfig => self.reload_config().await,
                        AppCommand::CheckForUpdates => self.send_update_command(UpdateCommand::Check),
                        AppCommand::InstallUpdate => self.send_update_command(UpdateCommand::Install),
                        #[cfg(feature = "config-gui-integration")]
                        AppCommand::OpenConfigGui => self.open_config_gui(),
                        AppCommand::Exit => {
                            info!("Exiting application...");
                            break;
                        },
                    }
                }
                result = self.clipboard_service.poll(),
                    if self.config.clipboard_debloater &&
                        self.clipboard_pause_state
                            .as_ref()
                            .map_or(true, |s| Instant::now() >= s.until)
                => {
                    match result {
                        Ok(()) if self.config.clipboard_debloater => {
                            self.clear_clipboard(WashMode::Automatic)
                        }
                        Ok(()) => continue,
                        Err(err) => {
                            error!("Failed to poll clipboard: {err:#}");
                            continue;
                        }
                    }
                }
                washed = async { self.wash_task.as_mut().unwrap().await },
                    if self.wash_task.is_some() => {
                        self.wash_task = None;
                        let (washed, mode) = washed;
                        if mode == WashMode::Manual || self.config.clipboard_debloater {
                            let current = self.clipboard_service.text();
                            if washed != *current {
                                let _ = self.clipboard_service.set_text(Arc::from(washed));
                            }
                        }
                    }
                Some(()) = async {
                    match self.clipboard_pause_state.as_mut() {
                        Some(state) => {
                            state.progress_timer.tick().await;
                            Some(())
                        }
                        None => None,
                    }
                } => {
                    self.publish_clipboard_status();
                }
            };
        }

        let _ = self.app_event_tx.send(AppEvent::Exit).await;
    }

    fn clear_clipboard(&mut self, mode: WashMode) {
        info!("Washing clipboard text...");
        let text = self.clipboard_service.text();
        let washer = Arc::clone(&self.text_washer);
        self.wash_task = Some(Box::pin(async move { (washer.wash(&text).await, mode) }));
    }

    fn toggle_clipboard_debloater(&mut self) {
        match &self.clipboard_pause_state {
            Some(_) => {
                info!("Resuming clipboard debloater...");
                self.clipboard_pause_state = None;
            }
            None => {
                info!(
                    "Pausing clipboard debloater for {} seconds...",
                    CLIPBOARD_PAUSE_DURATION.as_secs()
                );
                self.clipboard_pause_state = Some(ClipboardPauseState {
                    until: Instant::now() + CLIPBOARD_PAUSE_DURATION,
                    progress_timer: tokio::time::interval(Duration::from_secs(1)),
                });
            }
        }
        self.publish_clipboard_status();
    }

    fn publish_clipboard_status(&mut self) {
        let new_expires_at = match &self.clipboard_pause_state {
            Some(state) if state.until <= Instant::now() => {
                self.clipboard_pause_state = None;
                None
            }
            Some(state) => Some(state.until),
            None => None,
        };
        let _ = self
            .app_event_tx
            .try_send(AppEvent::ClipboardStatusChanged {
                clipboard_debloater_enabled: self.clipboard_debloater_enabled(),
                pause_expires_at: new_expires_at,
            });
    }

    fn send_update_command(&self, command: UpdateCommand) {
        if let Err(err) = self.update_command_tx.try_send(command) {
            error!("Failed to send update command: {err}");
        }
    }

    #[cfg(feature = "config-gui-integration")]
    fn open_config_gui(&self) {
        if self
            .config_gui_running
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            info!("Configuration window is already open");
            return;
        }
        config_integration::open_gui(Arc::clone(&self.config_gui_running));
    }

    async fn reload_config(&mut self) {
        info!("Reloading configuration...");
        match urldebloater_desktop_config::load_or_default() {
            Ok(config) => {
                self.set_config(config).await;
            }
            Err(err) => {
                error!("Failed to reload configuration: {:?}", err);
            }
        }
    }

    async fn set_config(&mut self, config: Config) {
        self.config = config;
        self.publish_clipboard_status();
        let catalog = self
            .text_washer
            .url_washer
            .clear_urls()
            .map(|cu| cu.catalog());
        self.text_washer = Arc::new(app_text_washer(&self.config, catalog).await);
        self.send_update_command(UpdateCommand::SetEnabled(self.config.check_for_updates));
    }
}

pub enum AppCommand {
    ClearClipboard,
    ToggleClipboardDebloater,
    ReloadConfig,
    CheckForUpdates,
    InstallUpdate,
    #[cfg(feature = "config-gui-integration")]
    OpenConfigGui,
    Exit,
}

pub enum AppEvent {
    ClipboardStatusChanged {
        clipboard_debloater_enabled: bool,
        pause_expires_at: Option<Instant>,
    },
    UpdateStatusChanged(UpdateStatus),
    UpdateProgress {
        downloaded: u64,
        total: Option<u64>,
    },
    Exit,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum WashMode {
    Manual,
    Automatic,
}

type WashTask = Pin<Box<dyn Future<Output = (String, WashMode)> + Send>>;

async fn app_text_washer(
    config: &Config,
    loaded_clear_urls: Option<&clearurls::DataCatalog>,
) -> TextWasher<DefaultRedirectResolver> {
    let redirect_resolver = match &config.redirect_wash_policy {
        RedirectWashPolicy::Ignore => DefaultRedirectResolver::None,
        RedirectWashPolicy::Locally => WithoutMixerRedirectResolver::default().into(),
        RedirectWashPolicy::ViaMixer { mixer_url } => {
            WithMixerRedirectResolver::new(mixer_url.clone()).into()
        }
    };
    let clear_urls = if config.use_clear_urls {
        match loaded_clear_urls {
            Some(catalog) => Some(ClearUrls::new(catalog.clone())),
            None => {
                let catalog = clearurls::cache::load(
                    &ReqwestCatalogLoader,
                    config_dir().join("clearurls-cache.json"),
                    Duration::from_secs(24 * 60 * 60),
                )
                .await
                .unwrap_or_else(|(catalog, err)| {
                    error!("Failed to load clearurls catalog from cache: {err:?}");
                    catalog
                });
                Some(ClearUrls::new(catalog))
            }
        }
    } else {
        None
    };

    let washer_config = WasherConfig {
        clear_urls,
        twitter_policy: config.twitter_policy,
    };
    TextWasher {
        url_washer: UrlWasher::new(redirect_resolver, washer_config),
    }
}

pub fn run() -> anyhow::Result<()> {
    let (app_command_tx, app_command_rx) = mpsc::channel(16);
    let (app_event_tx, app_event_rx) = mpsc::channel(16);

    let _config_watcher = config_integration::watch(app_command_tx.clone());

    #[cfg(target_os = "linux")]
    {
        thread::Builder::new()
            .name("app-worker".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                let result = runtime.block_on(async move {
                    let mut app = App::new(app_event_tx)
                        .await
                        .context("initialize application")?;
                    let clipboard_debloater_enabled = app.clipboard_debloater_enabled();
                    tokio::task::spawn(async move { app.run(app_command_rx).await });
                    tray::platform::run(app_command_tx, app_event_rx, clipboard_debloater_enabled)
                        .await
                });
                if let Err(err) = result {
                    error!("Application worker encountered an error: {:?}", err);
                }
            })
            .context("spawn application worker")?
            .join()
            .map_err(|err| anyhow!("Run application worker thread: {err:?}"))?;
    }

    #[cfg(not(target_os = "linux"))]
    {
        let clipboard_debloater_enabled = urldebloater_desktop_config::load_or_default()
            .map(|config| config.clipboard_debloater)
            .unwrap_or_else(|err| {
                error!("Failed to load configuration for the tray icon: {err:#}");
                Config::default().clipboard_debloater
            });
        thread::Builder::new()
            .name("app-worker".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                let result: anyhow::Result<()> = runtime.block_on(async move {
                    let mut app = App::new(app_event_tx)
                        .await
                        .context("initialize application")?;

                    app.run(app_command_rx).await;
                    Ok(())
                });
                if let Err(err) = result {
                    error!("Application worker encountered an error: {:?}", err);
                }
            })
            .context("spawn application worker")?;
        tray::platform::run(app_command_tx, app_event_rx, clipboard_debloater_enabled)
            .context("run tray platform")?;
    }

    info!("Application workers started");
    Ok(())
}
