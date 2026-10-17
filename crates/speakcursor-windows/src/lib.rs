//! Windows integration for the first SpeakCursor milestone.

#[cfg(windows)]
mod insertion;
#[cfg(windows)]
mod secrets;

#[cfg(windows)]
pub use insertion::{copy_text, foreground_window, paste_text_if_foreground};
#[cfg(windows)]
pub use secrets::OpenAiKeyStore;
