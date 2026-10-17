//! Platform-independent behavior for SpeakCursor.

mod dictation;

pub use dictation::{
    DictationAction, DictationController, DictationPhase, DictationShortcut, DictationStyle,
};
