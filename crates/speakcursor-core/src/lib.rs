//! Platform-independent behavior for SpeakCursor.

mod dictation;
mod settings;

pub use dictation::{
    DictationAction, DictationController, DictationPhase, DictationShortcut, DictationStyle,
};
pub use settings::{
    CloudTask, DictionaryEntry, LocalModel, RecognitionMode, Settings, SettingsError,
};
