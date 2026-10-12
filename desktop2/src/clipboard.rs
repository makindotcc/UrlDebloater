use arboard::Clipboard;
use log::{debug, info};
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc;
use tokio::time::sleep;

#[cfg(target_os = "linux")]
mod wayland_events;

const POLL_INTERVAL: Duration = Duration::from_millis(500);

pub struct ClipboardService {
    clipboard: Clipboard,
    last_text: Arc<str>,
    changes: Option<mpsc::UnboundedReceiver<()>>,
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
        }
    }

    /// Resolves once the clipboard holds text other than the last one handed out.
    pub async fn poll(&mut self) -> Result<(), arboard::Error> {
        // todo: windows/macos poll changeCount
        loop {
            match self.changes.as_mut() {
                Some(changes) => {
                    if changes.recv().await.is_none() {
                        self.changes = None;
                    }
                }
                None => sleep(POLL_INTERVAL).await,
            }

            let new_text = match self.clipboard.get_text() {
                Ok(text) => text,
                Err(arboard::Error::ContentNotAvailable) => continue,
                Err(err) => return Err(err),
            };
            debug!("New clipboard text detected: {}", new_text);
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
