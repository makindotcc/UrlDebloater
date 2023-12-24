use anyhow::Context;
use eframe::egui;
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;
use urlwasher::text_washer::TextWasher;

use crate::{
    clipboard_poller::ClipboardPoller,
    gui::{MyApp, TrayMenu},
};

mod clipboard_poller;
mod gui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .pretty()
        .with_line_number(false)
        .with_file(false)
        .init();
    debug!("Hello, world!");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        event_loop_builder: Some(Box::new(|event| {
            info!("eventy loop");
        })),
        ..Default::default()
    };
    eframe::run_native(
        "My egui App",
        options,
        Box::new(|_ccc| Box::<MyApp>::default()),
    )
    .unwrap();
    info!("finished running eframe");

    let mut arboard = arboard::Clipboard::new().context("Could not create clipboard accessor")?;
    let mut clipboard_poller = ClipboardPoller::new();
    let text_washer = TextWasher::default();
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
