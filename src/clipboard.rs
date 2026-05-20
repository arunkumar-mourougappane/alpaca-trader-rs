//! Thin wrapper around [`arboard`] for writing text to the system clipboard.
//!
//! All errors are returned as `String` so callers can forward them directly
//! to the status bar without pulling in the `arboard` type into every module.

/// Copies `text` to the system clipboard.
///
/// Returns `Ok(())` on success, or an error message that can be shown in the
/// status bar on failure (e.g. when running in a headless environment).
pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut cb| cb.set_text(text))
        .map_err(|e| format!("Clipboard error: {e}"))
}
