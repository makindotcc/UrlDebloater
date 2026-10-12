//! Cheap clipboard change gate for Windows.
//!
//! Every write to the clipboard bumps its sequence number, so the text only has
//! to be fetched when that number moves (fetching opens the clipboard and
//! transfers the whole content).

use log::{debug, warn};
use windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber;

pub struct ChangeCounter {
    seen: Option<u32>,
    warned_unavailable: bool,
}

impl ChangeCounter {
    pub fn new() -> Self {
        debug!("Clipboard change gate: GetClipboardSequenceNumber");
        Self {
            seen: None,
            warned_unavailable: false,
        }
    }

    /// Reports whether the clipboard changed since the previous call.
    pub fn moved(&mut self) -> bool {
        let current = match unsafe { GetClipboardSequenceNumber() } {
            0 => {
                if !self.warned_unavailable {
                    self.warned_unavailable = true;
                    warn!("Clipboard sequence number unavailable; reading the clipboard every cycle");
                }
                return true;
            }
            current => current,
        };
        let moved = self.seen != Some(current);
        self.seen = Some(current);
        moved
    }
}
