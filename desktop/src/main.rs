use std::sync::Arc;

use anyhow::Context;
use eframe::{egui, DetachedResult};
use tracing::{debug, error};
use tracing_subscriber::EnvFilter;
use tray_icon::menu::MenuEvent;
use urlwasher::text_washer::TextWasher;
use winit::event_loop::ControlFlow;

use crate::{
    clipboard_poller::ClipboardPoller,
    gui::{ConfigWindow, TrayMenu},
};

mod clipboard_poller;
mod gui;

pub struct AppState {
    text_washer: TextWasher,
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

    let app_state = Arc::new(AppState {
        text_washer: TextWasher::default(),
    });
    tokio::spawn({
        let app_state = Arc::clone(&app_state);
        async move {
            run_clipboard_patcher(&app_state)
                .await
                .expect("Could run clipboard patcher");
        }
    });
    run_gui(Arc::clone(&app_state));
}

async fn run_clipboard_patcher(app_state: &AppState) -> anyhow::Result<()> {
    let mut arboard = arboard::Clipboard::new().context("Could not create clipboard accessor")?;
    let mut clipboard_poller = ClipboardPoller::new();
    loop {
        let dirty_text = clipboard_poller
            .poll(&mut arboard)
            .await
            .context("Could not poll clipboard")?;
        debug!("Detected clipboard change: {dirty_text}");
        let clean_text = app_state.text_washer.wash(dirty_text).await;
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

fn run_gui(app_state: Arc<AppState>) -> ! {
    let event_loop = eframe::EventLoopBuilder::<eframe::UserEvent>::with_user_event().build();
    let tray_menu = TrayMenu::new();
    let menu_receiver = MenuEvent::receiver();

    let mut detached_app = eframe::run_detached_native(
        "UrlDebloater",
        &event_loop,
        eframe::NativeOptions {
            initial_window_size: Some(egui::vec2(320.0, 240.0)),
            ..Default::default()
        },
        Box::new(|_cc| Box::new(ConfigWindow::new(app_state))),
    );
    event_loop.run(move |event, event_loop, control_flow| {
        if let Ok(event) = menu_receiver.try_recv() {
            if event.id() == tray_menu.open_config.id() {
                if let Some(window) = detached_app.window() {
                    window.set_visible(true);
                }
            }
        }
        *control_flow = match detached_app.on_event(&event, event_loop).unwrap() {
            DetachedResult::Exit => ControlFlow::Exit,
            DetachedResult::UpdateNext => ControlFlow::Poll,
            DetachedResult::UpdateAt(next_paint) => ControlFlow::WaitUntil(next_paint),
        }
    });
}
