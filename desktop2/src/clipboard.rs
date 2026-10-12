use arboard::Clipboard;
use log::debug;
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc;
use tokio::time::sleep;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod wayland_events;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(any(target_os = "macos", target_os = "windows"))]
const POLL_INTERVAL: Duration = Duration::from_millis(200);
// Fallback if wayland events are not available.
#[cfg(target_os = "linux")]
const POLL_INTERVAL: Duration = Duration::from_millis(500);

pub struct ClipboardService {
    clipboard: Clipboard,
    last_text: Arc<str>,
    changes: Option<mpsc::UnboundedReceiver<()>>,
    #[cfg(target_os = "macos")]
    counter: macos::ChangeCounter,
    #[cfg(target_os = "windows")]
    counter: windows::ChangeCounter,
}

impl ClipboardService {
    pub fn new(clipboard: Clipboard) -> ClipboardService {
        #[cfg(target_os = "linux")]
        let changes = wayland_events::watch();
        #[cfg(not(target_os = "linux"))]
        let changes = None;

        Self {
            clipboard,
            last_text: Arc::from(""),
            changes,
            #[cfg(target_os = "macos")]
            counter: macos::ChangeCounter::new(),
            #[cfg(target_os = "windows")]
            counter: windows::ChangeCounter::new(),
        }
    }

    /// Resolves once the clipboard holds text other than the last one handed out.
    pub async fn poll(&mut self) -> Result<(), arboard::Error> {
        loop {
            match self.changes.as_mut() {
                Some(changes) => {
                    if changes.recv().await.is_none() {
                        self.changes = None;
                    }
                }
                None => sleep(POLL_INTERVAL).await,
            }

            #[cfg(any(target_os = "macos", target_os = "windows"))]
            if !self.counter.moved() {
                continue;
            }

            let new_text = match self.clipboard.get_text() {
                Ok(text) => text,
                Err(arboard::Error::ContentNotAvailable) => continue,
                Err(err) => return Err(err),
            };
            debug!("New clipboard text detected: {new_text}");
            if *self.last_text == *new_text {
                continue;
            }
            self.last_text = Arc::from(new_text);
            return Ok(());
        }
    }

    pub fn text(&self) -> Arc<str> {
        Arc::clone(&self.last_text)
    }

    pub fn set_text(&mut self, text: Arc<str>) -> Result<(), arboard::Error> {
        self.last_text = Arc::clone(&text);
        self.clipboard.set_text(&*text)
    }
}
