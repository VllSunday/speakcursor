//! Windows integration for the first SpeakCursor milestone.

#[cfg(windows)]
mod secrets;

#[cfg(windows)]
pub use secrets::OpenAiKeyStore;
