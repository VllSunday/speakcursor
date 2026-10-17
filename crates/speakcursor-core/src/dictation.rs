/// The processing rule selected before a dictation session starts.
/// The general hotkey's default style is resolved in settings, not here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DictationStyle {
    Raw,
    Proofread,
    Structure,
    Custom,
}

/// Stable identity of a registered shortcut; the platform layer assigns it.
/// The general shortcut and a style-specific shortcut have different IDs even
/// when both happen to select the same style.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DictationShortcut(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DictationPhase {
    Ready,
    Recording {
        shortcut: DictationShortcut,
        style: DictationStyle,
    },
    Finalizing(DictationStyle),
}

/// Work for the platform layer to perform after an input event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DictationAction {
    BeginCapture(DictationStyle),
    FinishCapture(DictationStyle),
    DiscardCapture,
    None,
}

/// Controls only the recording lifecycle. Audio capture, recognition, cloud
/// processing and insertion are performed by separate services.
#[derive(Default)]
pub struct DictationController {
    phase: DictationPhase,
}

impl Default for DictationPhase {
    fn default() -> Self {
        Self::Ready
    }
}

impl DictationController {
    pub fn phase(&self) -> DictationPhase {
        self.phase
    }

    /// Press an already-resolved dictation shortcut. Only the same shortcut
    /// finishes recording. Another binding cannot silently change the style
    /// or stop the active session.
    pub fn press(&mut self, shortcut: DictationShortcut, style: DictationStyle) -> DictationAction {
        match self.phase {
            DictationPhase::Ready => {
                self.phase = DictationPhase::Recording { shortcut, style };
                DictationAction::BeginCapture(style)
            }
            DictationPhase::Recording {
                shortcut: active_shortcut,
                style: active_style,
            } if active_shortcut == shortcut => {
                self.phase = DictationPhase::Finalizing(active_style);
                DictationAction::FinishCapture(active_style)
            }
            DictationPhase::Recording { .. } | DictationPhase::Finalizing(_) => {
                DictationAction::None
            }
        }
    }

    /// Esc during recording discards audio without recognition or insertion.
    pub fn escape(&mut self) -> DictationAction {
        if matches!(self.phase, DictationPhase::Recording { .. }) {
            self.phase = DictationPhase::Ready;
            DictationAction::DiscardCapture
        } else {
            DictationAction::None
        }
    }

    /// Called after recognition, optional processing and insertion have ended,
    /// regardless of success. Errors are handled by the service that failed.
    pub fn finalization_ended(&mut self) {
        if matches!(self.phase, DictationPhase::Finalizing(_)) {
            self.phase = DictationPhase::Ready;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GENERAL: DictationShortcut = DictationShortcut(1);
    const STYLE_RAW: DictationShortcut = DictationShortcut(2);
    const STYLE_STRUCTURE: DictationShortcut = DictationShortcut(3);

    #[test]
    fn same_shortcut_starts_and_finishes_a_session() {
        let mut controller = DictationController::default();
        assert_eq!(
            controller.press(GENERAL, DictationStyle::Raw),
            DictationAction::BeginCapture(DictationStyle::Raw)
        );
        assert_eq!(
            controller.press(GENERAL, DictationStyle::Raw),
            DictationAction::FinishCapture(DictationStyle::Raw)
        );
        assert_eq!(
            controller.phase(),
            DictationPhase::Finalizing(DictationStyle::Raw)
        );
        controller.finalization_ended();
        assert_eq!(controller.phase(), DictationPhase::Ready);
    }

    #[test]
    fn escape_discards_recording_without_finishing_it() {
        let mut controller = DictationController::default();
        controller.press(GENERAL, DictationStyle::Custom);
        assert_eq!(controller.escape(), DictationAction::DiscardCapture);
        assert_eq!(controller.phase(), DictationPhase::Ready);
        assert_eq!(
            controller.press(GENERAL, DictationStyle::Custom),
            DictationAction::BeginCapture(DictationStyle::Custom)
        );
    }

    #[test]
    fn another_style_cannot_replace_the_active_recording() {
        let mut controller = DictationController::default();
        controller.press(GENERAL, DictationStyle::Proofread);
        assert_eq!(
            controller.press(STYLE_STRUCTURE, DictationStyle::Structure),
            DictationAction::None
        );
        assert_eq!(
            controller.phase(),
            DictationPhase::Recording {
                shortcut: GENERAL,
                style: DictationStyle::Proofread,
            }
        );
        assert_eq!(
            controller.press(GENERAL, DictationStyle::Proofread),
            DictationAction::FinishCapture(DictationStyle::Proofread)
        );
    }

    #[test]
    fn two_shortcuts_for_the_same_style_are_not_the_same_press() {
        let mut controller = DictationController::default();
        controller.press(GENERAL, DictationStyle::Raw);
        assert_eq!(
            controller.press(STYLE_RAW, DictationStyle::Raw),
            DictationAction::None
        );
        assert_eq!(
            controller.press(GENERAL, DictationStyle::Raw),
            DictationAction::FinishCapture(DictationStyle::Raw)
        );
    }

    #[test]
    fn shortcut_and_escape_do_not_start_a_second_session_while_finalizing() {
        let mut controller = DictationController::default();
        controller.press(STYLE_STRUCTURE, DictationStyle::Structure);
        controller.press(STYLE_STRUCTURE, DictationStyle::Structure);
        assert_eq!(
            controller.press(GENERAL, DictationStyle::Raw),
            DictationAction::None
        );
        assert_eq!(controller.escape(), DictationAction::None);
        controller.finalization_ended();
        assert_eq!(
            controller.press(GENERAL, DictationStyle::Raw),
            DictationAction::BeginCapture(DictationStyle::Raw)
        );
    }
}
