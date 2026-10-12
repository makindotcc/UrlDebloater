//! Cheap clipboard change gate for macOS.
//!
//! `NSPasteboard.changeCount` is bumped by every write, so the text only has to
//! be fetched when that counter moves (fetching transfers the whole content).

use log::debug;
use objc2_app_kit::NSPasteboard;

#[derive(Default)]
pub struct ChangeCounter {
    seen: Option<isize>,
}

impl ChangeCounter {
    pub fn new() -> Self {
        debug!("Clipboard change gate: NSPasteboard changeCount");
        Self::default()
    }

    /// Reports whether the clipboard changed since the previous call.
    pub fn moved(&mut self) -> bool {
        let current = NSPasteboard::generalPasteboard().changeCount();
        let moved = self.seen != Some(current);
        self.seen = Some(current);
        moved
    }
}
