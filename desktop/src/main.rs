use crate::{
    clipboard_poller::ClipboardPoller,
    gui::{ConfigWindow, TrayMenu},
};
use anyhow::Context;
use config::AppConfig;
use eframe::{egui, DetachedResult};
use futures::{stream::FuturesUnordered, StreamExt};
use notify_rust::Notification;
use std::{sync::Arc, time::Duration};
use tokio::{
    select,
    sync::watch,
    time::{sleep, sleep_until, Instant},
};
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;
use tray_icon::menu::MenuEvent;
use urlwasher::{text_washer::TextWasher, UrlWasher};
use winit::event_loop::ControlFlow;

mod clipboard_poller;
mod config;
mod gui;

const APP_NAME: &str = "UrlDebloater";
const CLIPBOARD_PAUSE_DURATION: Duration = Duration::from_secs(30);

pub struct AppState {
    text_washer: TextWasher,
    config: AppConfig,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            text_washer: TextWasher {
                url_washer: UrlWasher::new(config.url_washer.clone()),
            },
            config,
        }
    }

    pub fn with_config(&self, config: AppConfig) -> Self {
        AppState::new(config)
    }
}

#[derive(Clone)]
pub struct AppStateFlow {
    pub tx: Arc<watch::Sender<Arc<AppState>>>,
    pub rx: watch::Receiver<Arc<AppState>>,
}

impl AppStateFlow {
    pub fn new(state: AppState) -> Self {
        let (tx, rx) = watch::channel(Arc::new(state));
        Self {
            rx,
            tx: Arc::new(tx),
        }
    }

    pub fn current(&self) -> watch::Ref<'_, Arc<AppState>> {
        self.rx.borrow()
    }

    pub fn modify_config(&self, apply_changes: impl FnOnce(&mut AppConfig)) {
        let mut new_config = self.current().config.clone();
        apply_changes(&mut new_config);
        let _ = self.tx.send(Arc::new(AppState::new(new_config)));
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .pretty()
        .with_line_number(false)
        .with_file(false)
        .init();
    debug!("Hello, world!");

    let config = config::from_file().await.unwrap_or_else(|err| {
        error!("Could not read config file: {err:?}. Using default...");
        AppConfig::default()
    });
    let app_state = AppState::new(config);
    let app_state_flow = AppStateFlow::new(app_state);
    tokio::spawn(persist_config(app_state_flow.rx.clone()));
    tokio::spawn(run_background_jobs_supervisor(app_state_flow.rx.clone()));
    run_gui(app_state_flow);
}

async fn persist_config(mut state_rx: watch::Receiver<Arc<AppState>>) {
    loop {
        if state_rx.changed().await.is_err() {
            return;
        };
        match {
            let app_config = &state_rx.borrow_and_update().config;
            config::save_to_file(app_config)
        }
        .await
        {
            Ok(_) => debug!("Saved config file."),
            Err(err) => error!("Could not save config: {err:?}"),
        };
        sleep(Duration::from_secs(1)).await; // throttle
    }
}

async fn run_background_jobs_supervisor(mut state_rx: watch::Receiver<Arc<AppState>>) {
    loop {
        let state = state_rx.borrow_and_update().to_owned();
        select! {
            _ = run_background_jobs(&state) => {}
            result = state_rx.changed() => {
                if result.is_err() {
                    return;
                }
            }
        }
    }
}

async fn run_background_jobs(app_state: &AppState) {
    let mut tasks = FuturesUnordered::new();

    let config = &app_state.config;
    if config.enable_clipboard_patcher {
        let paused_until = app_state.config.clipboard_patcher_paused_until;
        tasks.push(async move {
            if let Some(paused_until) = paused_until {
                sleep_until(paused_until).await;
            }
            loop {
                info!("Starting clipboard patcher");
                if let Err(err) = run_clipboard_patcher(&app_state.text_washer).await {
                    error!("Could not run clipboard patcher: {err:?}.");
                }
                sleep(Duration::from_secs(5)).await;
            }
        });
    }

    if tasks.is_empty() {
        std::future::pending().await
    } else {
        while let Some(_) = tasks.next().await {}
    }
}

async fn run_clipboard_patcher(text_washer: &TextWasher) -> anyhow::Result<()> {
    let mut arboard = arboard::Clipboard::new().context("Could not create clipboard accessor")?;
    let mut clipboard_poller = ClipboardPoller::new();
    loop {
        let dirty_text = clipboard_poller
            .poll(&mut arboard)
            .await
            .context("Could not poll clipboard")?;
        debug!("Detected clipboard change: {dirty_text}");
        let clean_text = text_washer.wash(dirty_text).await;
        if clean_text != dirty_text
            && arboard
                .get_text()
                .is_ok_and(|current_clipboard| dirty_text == current_clipboard)
        {
            debug!("Cleaned text: {clean_text}");
            if let Err(err) = clipboard_poller.set_text(&mut arboard, clean_text) {
                error!("Could not copy cleaned text to clipboard: {err:?}");
            }
        }
    }
}

fn run_gui(app_state_flow: AppStateFlow) -> ! {
    let event_loop = eframe::EventLoopBuilder::<eframe::UserEvent>::with_user_event().build();
    let tray_menu = TrayMenu::new();
    let menu_receiver = MenuEvent::receiver();

    let mut detached_app = eframe::run_detached_native(
        APP_NAME,
        &event_loop,
        eframe::NativeOptions {
            initial_window_size: Some(egui::vec2(620.0, 340.0)),
            ..Default::default()
        },
        Box::new({
            let app_state_flow = app_state_flow.clone();
            |_cc| Box::new(ConfigWindow::new(app_state_flow))
        }),
    );

    event_loop.run(move |event, event_loop, control_flow| {
        if let Ok(event) = menu_receiver.try_recv() {
            let event_id = event.id();
            if event_id == tray_menu.open_config.id() {
                if let Some(window) = detached_app.window() {
                    window.set_visible(true);
                }
            } else if event_id == tray_menu.wash_clipboard.id() {
                info!("Debloating clipboard from tray...");
                let app_state = app_state_flow.rx.borrow().to_owned();
                tokio::spawn(async move {
                    if let Err(err) = tray_wash_clipboard(&app_state).await {
                        error!("Could not wash clipboard from tray: {err:?}");
                        if let Err(err) = Notification::new()
                            .summary(APP_NAME)
                            .body(&err.to_string())
                            .show()
                        {
                            error!("Could not show error notification: {err}");
                        }
                    }
                });
            } else if event_id == tray_menu.pause_clipboard_washer.id() {
                app_state_flow.modify_config(|config| {
                    if config.clipboard_patcher_paused_until.is_some() {
                        config.clipboard_patcher_paused_until = None;
                    } else {
                        config.clipboard_patcher_paused_until =
                            Some(Instant::now() + CLIPBOARD_PAUSE_DURATION);
                    }
                });
            }
        }

        update_tray_state(&tray_menu, &app_state_flow.current());

        *control_flow = match detached_app.on_event(&event, event_loop).unwrap() {
            DetachedResult::Exit => ControlFlow::Exit,
            DetachedResult::UpdateNext => ControlFlow::Poll,
            DetachedResult::UpdateAt(next_paint) => {
                let max_next_paint = std::time::Instant::now() + Duration::from_millis(200);
                ControlFlow::WaitUntil(if next_paint > max_next_paint {
                    max_next_paint
                } else {
                    next_paint
                })
            }
        };
    });
}

fn update_tray_state(tray_menu: &TrayMenu, app_state: &AppState) {
    tray_menu
        .pause_clipboard_washer
        .set_enabled(app_state.config.enable_clipboard_patcher);
    let (active, new_text) = if app_state.config.enable_clipboard_patcher {
        match app_state.config.clipboard_patcher_paused_until {
            Some(paused_until) if paused_until > Instant::now() => (
                true,
                format!(
                    "Clipboard debloater paused for {} sec.",
                    paused_until.duration_since(Instant::now()).as_secs()
                ),
            ),
            _ => (
                false,
                format!(
                    "Pause clipboard debloater for {} sec.",
                    CLIPBOARD_PAUSE_DURATION.as_secs()
                ),
            ),
        }
    } else {
        (
            false,
            String::from("Clipboard debloater disabled in config"),
        )
    };
    tray_menu.pause_clipboard_washer.set_checked(active);
    // check if changed, because too frequent changes causes text blinking (on windows at least)
    if tray_menu.pause_clipboard_washer.text() != new_text {
        tray_menu.pause_clipboard_washer.set_text(new_text);
    }
}

async fn tray_wash_clipboard(app_state: &AppState) -> anyhow::Result<()> {
    let mut clipboard = arboard::Clipboard::new().context("Could not create clipboard accessor")?;
    let clipboard_text = clipboard
        .get_text()
        .context("Could not get text from clipboard")?;
    clipboard
        .set_text(app_state.text_washer.wash(&clipboard_text).await)
        .context("Could not copy clean text to clipboard")?;
    Ok(())
}
