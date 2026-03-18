use super::AppState;

impl AppState {
    /// Lazily initializes the system clipboard.
    ///
    /// On headless/SSH environments where clipboard access fails,
    /// `self.clipboard` remains `None` and clipboard ops silently no-op.
    pub(super) fn ensure_clipboard(&mut self) {
        if self.clipboard.is_none() {
            match arboard::Clipboard::new() {
                Ok(cb) => self.clipboard = Some(cb),
                Err(e) => log::warn!("Failed to access clipboard: {e}"),
            }
        }
    }

    /// Copies the given text to the system clipboard.
    ///
    /// Logs a warning if clipboard access fails.
    pub(super) fn copy_to_clipboard(text: &str) {
        match arboard::Clipboard::new() {
            Ok(mut clipboard) => {
                if let Err(e) = clipboard.set_text(text) {
                    log::warn!("Failed to copy to clipboard: {e}");
                }
            }
            Err(e) => {
                log::warn!("Failed to access clipboard: {e}");
            }
        }
    }
}
