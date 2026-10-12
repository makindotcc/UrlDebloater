use arboard::Clipboard;
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;

pub struct ClipboardService {
    clipboard: Clipboard,
    last_text: Arc<str>,
}

impl ClipboardService {
    pub fn new(clipboard: Clipboard) -> ClipboardService {
        Self {
            clipboard,
            last_text: Arc::from(""),
        }
    }

    pub async fn poll(&mut self) -> Result<(), arboard::Error> {
        // todo: windows/macos poll changeCount
        //       linux wayland data-control
        loop {
            sleep(Duration::from_millis(200)).await;
            let new_text = match self.clipboard.get_text() {
                Ok(text) => text,
                Err(arboard::Error::ContentNotAvailable) => continue,
                Err(err) => return Err(err),
            };
            if &*self.last_text != new_text {
                self.last_text = Arc::from(new_text);
                return Ok(());
            }
            self.last_text = Arc::from(new_text);
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
