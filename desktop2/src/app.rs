use crate::{clipboard::ClipboardService, tray};
use anyhow::{Context, anyhow};
use arboard::Clipboard;
use log::{error, info};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tokio::{select, sync::mpsc, time::Interval};
use urldebloater_config::{self as config, Config, RedirectWashPolicy};
use urlwasher::{
    UrlWasher,
    redirect_resolver::{
        DefaultRedirectResolver, WithMixerRedirectResolver, WithoutMixerRedirectResolver,
    },
    text_washer::TextWasher,
};

pub(crate) const CLIPBOARD_PAUSE_DURATION: Duration = Duration::from_secs(30);

struct App {
    config: Config,
    clipboard_service: ClipboardService,
    config_gui_running: Arc<AtomicBool>,
    text_washer: Arc<TextWasher>,
    wash_task: Option<Pin<Box<dyn Future<Output = String> + Send>>>,
    app_event_tx: mpsc::Sender<AppEvent>,
    clipboard_pause_state: Option<ClipboardPauseState>,
}

struct ClipboardPauseState {
    until: Instant,
    progress_timer: Interval,
}

impl App {
    fn new(app_event_tx: mpsc::Sender<AppEvent>) -> anyhow::Result<Self> {
        let config = config::load_or_default()?;
        let clipboard = Clipboard::new().context("initialize clipboard")?;
        let clipboard_service = ClipboardService::new(clipboard);

        Ok(Self {
            text_washer: Arc::new(text_washer(&config)),
            config,
            clipboard_service,
            config_gui_running: Arc::new(AtomicBool::new(false)),
            wash_task: None,
            clipboard_pause_state: None,
            app_event_tx,
        })
    }

    async fn run(&mut self, mut app_command_rx: mpsc::Receiver<AppCommand>) {
        loop {
            select! {
                result = self.clipboard_service.poll(),
                if self.config.clipboard_debloater &&
                    self.clipboard_pause_state
                        .as_ref()
                        .map_or(true, |s| Instant::now() >= s.until) => {
                    match result {
                        Ok(()) if self.config.clipboard_debloater => self.clear_clipboard(),
                        Ok(()) => continue,
                        Err(err) => {
                            error!("Failed to poll clipboard: {err:#}");
                            continue;
                        }
                    }
                }
                Some(tray_command) = app_command_rx.recv()=> {
                    match tray_command {
                        AppCommand::ClearClipboard => self.clear_clipboard(),
                        AppCommand::ToggleClipboardDebloater => self.toggle_clipboard_debloater(),
                        AppCommand::ReloadConfig => self.reload_config(),
                        AppCommand::OpenConfigGui => self.open_config_gui(),
                        AppCommand::Exit => {
                            info!("Exiting application...");
                            break;
                        },
                    }
                }
                washed = async { self.wash_task.as_mut().unwrap().await },
                if self.wash_task.is_some() => {
                    self.wash_task = None;
                    let current = self.clipboard_service.text();
                    if washed != *current {
                        let _ = self.clipboard_service.set_text(Arc::from(washed));
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
                    self.progress_clipboard_pause();
                }
            };
        }

        let _ = self.app_event_tx.send(AppEvent::Exit).await;
    }

    fn clear_clipboard(&mut self) {
        info!("Washing clipboard text...");
        let text = self.clipboard_service.text();
        let washer = Arc::clone(&self.text_washer);
        self.wash_task = Some(Box::pin(async move { washer.wash(&text).await }));
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
        self.progress_clipboard_pause();
    }

    fn progress_clipboard_pause(&mut self) {
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
            .try_send(AppEvent::ClipboardPauseProgressUpdate {
                expires_at: new_expires_at,
            });
    }

    fn open_config_gui(&self) {
        if self.config_gui_running.swap(true, Ordering::Relaxed) {
            info!("Configuration window is already open");
            return;
        }
        let Some(exe) = config_gui_path() else {
            error!("Configuration window not found next to the application binary");
            self.config_gui_running.store(false, Ordering::Relaxed);
            return;
        };
        let path = urldebloater_config::config_path();
        match std::process::Command::new(&exe).arg("--config").arg(&path).spawn() {
            Ok(mut child) => {
                // The application outlives the window, so reap the child instead of
                // leaving a zombie behind.
                let running = Arc::clone(&self.config_gui_running);
                tokio::task::spawn_blocking(move || {
                    match child.wait() {
                        Ok(status) if !status.success() => {
                            error!("Configuration window exited with {status}");
                        }
                        Ok(_) => {}
                        Err(err) => error!("Configuration window failed: {err}"),
                    }
                    running.store(false, Ordering::Relaxed);
                });
            }
            Err(err) => {
                self.config_gui_running.store(false, Ordering::Relaxed);
                error!("Failed to start {}: {err}", exe.display());
            }
        }
    }

    fn reload_config(&mut self) {
        info!("Reloading configuration...");
        match config::load_or_default() {
            Ok(config) => {
                self.set_config(config);
            }
            Err(err) => {
                error!("Failed to reload configuration: {:?}", err);
            }
        }
    }

    fn set_config(&mut self, config: Config) {
        self.config = config;
        self.text_washer = Arc::new(text_washer(&self.config));
    }
}

pub enum AppCommand {
    ClearClipboard,
    ToggleClipboardDebloater,
    ReloadConfig,
    OpenConfigGui,
    Exit,
}

pub enum AppEvent {
    ClipboardPauseProgressUpdate { expires_at: Option<Instant> },
    Exit,
}

const RELOAD_DEBOUNCE: Duration = Duration::from_millis(300);
const CONFIG_GUI_BIN: &str = "urldebloater-config-gui";

fn config_gui_path() -> Option<std::path::PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let name = if cfg!(windows) {
        format!("{CONFIG_GUI_BIN}.exe")
    } else {
        CONFIG_GUI_BIN.to_owned()
    };
    let path = dir.join(name);
    path.is_file().then_some(path)
}

/// Reloads the config whenever the config window or the user rewrites the file.
fn watch_config(command_tx: mpsc::Sender<AppCommand>) -> Option<RecommendedWatcher> {
    let path = urldebloater_config::config_path();
    let dir = path.parent()?.to_path_buf();
    let watched = path;
    let last_reload = Arc::new(Mutex::new(None::<Instant>));
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else {
            return;
        };
        if !event.paths.iter().any(|event_path| *event_path == watched) {
            return;
        }
        // Atomic replacement emits several events; one reload is enough.
        let mut last = last_reload.lock().expect("config reload lock");
        if last.is_some_and(|at| at.elapsed() < RELOAD_DEBOUNCE) {
            return;
        }
        *last = Some(Instant::now());
        drop(last);
        if let Err(err) = command_tx.try_send(AppCommand::ReloadConfig) {
            error!("Failed to queue config reload: {err}");
        }
    })
    .map_err(|err| error!("Failed to watch {}: {err}", dir.display()))
    .ok()?;
    watcher
        .watch(&dir, RecursiveMode::NonRecursive)
        .map_err(|err| error!("Failed to watch {}: {err}", dir.display()))
        .ok()?;
    Some(watcher)
}

fn text_washer(config: &Config) -> TextWasher<DefaultRedirectResolver> {
    let redirect_resolver = match &config.redirect_wash_policy {
        RedirectWashPolicy::Ignore => DefaultRedirectResolver::None,
        RedirectWashPolicy::Locally => WithoutMixerRedirectResolver::default().into(),
        RedirectWashPolicy::ViaMixer { mixer_url } => {
            WithMixerRedirectResolver::new(mixer_url.clone()).into()
        }
    };
    TextWasher {
        url_washer: UrlWasher::new_with_config(redirect_resolver, config.washer_config.clone()),
    }
}

pub fn run() -> anyhow::Result<()> {
    let (app_command_tx, app_command_rx) = mpsc::channel(16);
    let (app_event_tx, app_event_rx) = mpsc::channel(16);

    let mut app = App::new(app_event_tx).context("initialize application")?;
    let _config_watcher = watch_config(app_command_tx.clone());

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
                    tokio::task::spawn(async move { app.run(app_command_rx).await });
                    tray::platform::run(app_command_tx, app_event_rx).await
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
        thread::Builder::new()
            .name("app-worker".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async move {
                    app.run(app_command_rx).await;
                });
            })
            .context("spawn application worker")?;
        tray::platform::run(app_command_tx, app_event_rx);
    }

    info!("Application workers started");
    Ok(())
}
