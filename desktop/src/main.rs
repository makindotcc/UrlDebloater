use std::time::Duration;

use anyhow::Context;
use eframe::{egui, DetachedResult};
use futures::{stream::FuturesUnordered, StreamExt};
use tokio::{select, sync::watch, time::sleep};
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;
use tray_icon::menu::MenuEvent;
use urlwasher::{text_washer::TextWasher, UrlWasher, UrlWasherConfig};
use winit::event_loop::ControlFlow;

use crate::{
    clipboard_poller::ClipboardPoller,
    gui::{ConfigWindow, TrayMenu},
};

mod clipboard_poller;
mod gui;

#[derive(Clone)]
pub struct AppConfig {
    url_washer: UrlWasherConfig,
    enable_clipboard_patcher: bool,
}

pub struct AppConfigFlow {
    pub tx: watch::Sender<AppConfig>,
    pub rx: watch::Receiver<AppConfig>,
}

impl AppConfigFlow {
    pub fn new(config: AppConfig) -> Self {
        let (tx, rx) = watch::channel(config);
        Self { rx, tx }
    }

    pub fn current(&self) -> watch::Ref<'_, AppConfig> {
        self.rx.borrow()
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

    let config = AppConfig {
        url_washer: UrlWasherConfig {
            mixer_instance: None,
            tiktok_policy: urlwasher::RedirectWashPolicy::Locally,
        },
        enable_clipboard_patcher: true,
    };
    let config_flow = AppConfigFlow::new(config);
    {
        let mut config_rx = config_flow.rx.clone();
        tokio::spawn(async move {
            loop {
                let config = config_rx.borrow_and_update().to_owned();
                select! {
                    _ = run_background_jobs(config) => {}
                    result = config_rx.changed() => {
                        if result.is_err() {
                            return;
                        }
                    }
                }
            }
        });
    }
    run_gui(config_flow);
}

async fn run_background_jobs(config: AppConfig) {
    let mut tasks = FuturesUnordered::new();
    let text_washer = TextWasher {
        url_washer: UrlWasher::new(config.url_washer.clone()),
    };

    if config.enable_clipboard_patcher {
        tasks.push(async move {
            loop {
                info!("Starting clipboard patcher");
                if let Err(err) = run_clipboard_patcher(&text_washer).await {
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

fn run_gui(config_flow: AppConfigFlow) -> ! {
    let event_loop = eframe::EventLoopBuilder::<eframe::UserEvent>::with_user_event().build();
    let tray_menu = TrayMenu::new();
    let menu_receiver = MenuEvent::receiver();

    let mut detached_app = eframe::run_detached_native(
        "UrlDebloater",
        &event_loop,
        eframe::NativeOptions {
            initial_window_size: Some(egui::vec2(620.0, 340.0)),
            ..Default::default()
        },
        Box::new(|_cc| Box::new(ConfigWindow::new(config_flow))),
    );
    event_loop.run(move |event, event_loop, control_flow| {
        if let Ok(event) = menu_receiver.try_recv() {
            let event_id = event.id();
            if event_id == tray_menu.open_config.id() {
                if let Some(window) = detached_app.window() {
                    window.set_visible(true);
                }
            } else if event_id == tray_menu.wash_clipboard.id() {
            }
        }
        *control_flow = match detached_app.on_event(&event, event_loop).unwrap() {
            DetachedResult::Exit => ControlFlow::Exit,
            DetachedResult::UpdateNext => ControlFlow::Poll,
            DetachedResult::UpdateAt(next_paint) => ControlFlow::WaitUntil(next_paint),
        }
    });
}
