use arboard::Clipboard;
use log::{debug, info, warn};
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc;
use tokio::time::sleep;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod wayland_events;
#[cfg(target_os = "windows")]
mod windows;

const POLL_INTERVAL: Duration = Duration::from_millis(200);
const RETRY_INTERVAL: Duration = Duration::from_secs(1);
const MAX_RETRY_INTERVAL: Duration = Duration::from_secs(10);

pub struct ClipboardService {
    clipboard: Clipboard,
    last_text: Arc<str>,
    changes: Option<mpsc::UnboundedReceiver<()>>,
    #[cfg(target_os = "macos")]
    counter: macos::ChangeCounter,
    #[cfg(target_os = "windows")]
    counter: windows::ChangeCounter,
    retry_in: Option<Duration>,
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
            retry_in: None,
        }
    }

    /// Resolves once the clipboard holds text other than the last one handed out.
    ///
    /// `Err` reports a broken clipboard connection: the caller is expected to keep
    /// calling, the connection is re-established with a growing retry delay.
    pub async fn poll(&mut self) -> Result<(), arboard::Error> {
        loop {
            match self.retry_in {
                Some(delay) => sleep(delay).await,
                None => self.wait_for_change().await,
            }

            #[cfg(any(target_os = "macos", target_os = "windows"))]
            if self.retry_in.is_none() && !self.counter.moved() {
                continue;
            }

            let new_text = match self.clipboard.get_text() {
                Ok(text) => text,
                Err(arboard::Error::ContentNotAvailable) => continue,
                Err(err) => {
                    self.retry(&err).await;
                    return Err(err);
                }
            };

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

    async fn wait_for_change(&mut self) {
        if let Some(changes) = self.changes.as_mut() {
            match changes.recv().await {
                Some(_) => return,
                None => {
                    debug!("The event thread is gone; the connection it watched is gone with it.");
                    self.changes = None;
                }
            }
        }
        sleep(POLL_INTERVAL).await;
    }

    async fn retry(&mut self, err: &arboard::Error) {
        let delay = match self.retry_in {
            Some(delay) => (delay * 2).min(MAX_RETRY_INTERVAL),
            None => RETRY_INTERVAL,
        };
        warn!("Clipboard unavailable ({err}); retrying in {delay:?}");
        sleep(delay).await;
        if self.reconnect().is_ok() {
            info!("Reconnected to the clipboard");
            self.retry_in = None;
        } else {
            self.retry_in = Some(delay);
        }
    }

    fn reconnect(&mut self) -> Result<(), ()> {
        #[cfg(target_os = "linux")]
        {
            // Rebuilding the clipboard without a compositor makes arboard fall back to
            // the X11 clipboard, which this app never reads from.
            if !wayland_events::reachable() {
                return Err(());
            }
            self.changes = wayland_events::watch();
        }
        match Clipboard::new() {
            Ok(clipboard) => {
                self.clipboard = clipboard;
                Ok(())
            }
            Err(err) => {
                debug!("Clipboard reconnect failed ({err})");
                Err(())
            }
        }
    }
}
