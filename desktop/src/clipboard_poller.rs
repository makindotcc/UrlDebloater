use std::time::Duration;
use tokio::time::sleep;

pub struct ClipboardPoller {
    last_text: String,
}

impl ClipboardPoller {
    pub fn new() -> ClipboardPoller {
        Self {
            last_text: String::new(),
        }
    }

    pub async fn poll(&mut self, arboard: &mut arboard::Clipboard) -> Result<&str, arboard::Error> {
        loop {
            sleep(Duration::from_millis(200)).await;
            let new_text = match arboard.get_text() {
                Ok(text) => text,
                Err(arboard::Error::ContentNotAvailable) => continue,
                Err(err) => return Err(err),
            };
            if self.last_text != new_text {
                self.last_text = new_text;
                return Ok(&self.last_text);
            }
        }
    }

    pub fn set_text(
        &mut self,
        arboard: &mut arboard::Clipboard,
        text: String,
    ) -> Result<(), arboard::Error> {
        self.last_text = text;
        arboard.set_text(&self.last_text)
    }
}
