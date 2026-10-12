use crate::{
    clipboard::ClipboardService,
    config::{self, Config, RedirectWashPolicy},
    tray,
};
use anyhow::Context;
use arboard::Clipboard;
use log::{error, info};
use std::{
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{select, sync::mpsc, time::Interval};
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
    Exit,
}

pub enum AppEvent {
    ClipboardPauseProgressUpdate { expires_at: Option<Instant> },
    Exit,
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

pub fn spawn_app(
    app_command_rx: mpsc::Receiver<AppCommand>,
    app_event_tx: mpsc::Sender<AppEvent>,
) -> anyhow::Result<()> {
    let mut app = App::new(app_event_tx).context("initialize application")?;
    tokio::spawn(async move {
        app.run(app_command_rx).await;
    });
    Ok(())
}

pub async fn run() -> anyhow::Result<()> {
    let (app_command_tx, app_command_rx) = mpsc::channel(16);
    let (app_event_tx, app_event_rx) = mpsc::channel(16);

    spawn_app(app_command_rx, app_event_tx).context("spawn app")?;

    info!("Application workers started");
    tray::run(app_command_tx, app_event_rx).await
}
