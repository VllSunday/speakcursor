//! Persistent non-secret settings for the first dictation milestone.
//! The OpenAI key is deliberately outside this structure; the Windows layer
//! stores it with OS-provided protection.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::DictationStyle;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudTask {
    SpeechRecognition,
    SpeechProcessing,
    PromptGeneration,
    SelectedText,
    ScreenHelp,
    Assistant,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecognitionMode {
    #[default]
    Local,
    OpenAi,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalCompute {
    #[default]
    Cpu,
    NvidiaCuda,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DictionaryEntry {
    pub heard: String,
    pub written: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LocalModel {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub local_models: Vec<LocalModel>,
    pub active_model_id: Option<String>,
    pub recognition_mode: RecognitionMode,
    pub local_compute: LocalCompute,
    pub language: String,
    pub unload_after_minutes: Option<u32>,
    pub default_style: DictationStyle,
    pub custom_style_prompt: String,
    pub dictionary: Vec<DictionaryEntry>,
    pub keep_history_forever: bool,
    pub cloud_models: BTreeMap<CloudTask, String>,
    pub dictation_shortcut: String,
    pub style_shortcuts: BTreeMap<DictationStyle, String>,
    pub prompt_shortcut: String,
    pub screen_shortcut: String,
    pub launch_at_login: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            local_models: Vec::new(),
            active_model_id: None,
            recognition_mode: RecognitionMode::Local,
            local_compute: LocalCompute::Cpu,
            language: "auto".into(),
            unload_after_minutes: None,
            default_style: DictationStyle::Raw,
            custom_style_prompt: String::new(),
            dictionary: Vec::new(),
            keep_history_forever: false,
            cloud_models: BTreeMap::new(),
            dictation_shortcut: "Ctrl+Alt+F8".into(),
            style_shortcuts: BTreeMap::new(),
            prompt_shortcut: "Ctrl+Alt+F9".into(),
            screen_shortcut: "Ctrl+Alt+F10".into(),
            launch_at_login: true,
        }
    }
}

#[derive(Debug)]
pub enum SettingsError {
    Io(std::io::Error),
    InvalidJson(serde_json::Error),
    InvalidValue(String),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "settings I/O error: {error}"),
            Self::InvalidJson(error) => write!(f, "invalid settings JSON: {error}"),
            Self::InvalidValue(message) => write!(f, "invalid settings: {message}"),
        }
    }
}

impl std::error::Error for SettingsError {}

impl Settings {
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        let data = match fs::read(path) {
            Ok(data) => data,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(SettingsError::Io(error)),
        };
        let settings: Self = serde_json::from_slice(&data).map_err(SettingsError::InvalidJson)?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(SettingsError::Io)?;
        }
        let data = serde_json::to_vec_pretty(self).map_err(SettingsError::InvalidJson)?;
        fs::write(path, data).map_err(SettingsError::Io)
    }

    pub fn validate(&self) -> Result<(), SettingsError> {
        let mut ids = HashSet::new();
        for model in &self.local_models {
            if model.id.trim().is_empty()
                || model.name.trim().is_empty()
                || model.path.as_os_str().is_empty()
            {
                return Err(SettingsError::InvalidValue(
                    "each local model needs an id, name and path".into(),
                ));
            }
            if !ids.insert(&model.id) {
                return Err(SettingsError::InvalidValue(
                    "local model ids must be unique".into(),
                ));
            }
        }
        if self
            .active_model_id
            .as_ref()
            .is_some_and(|id| !ids.contains(id))
        {
            return Err(SettingsError::InvalidValue(
                "active local model is not in the saved list".into(),
            ));
        }
        if self.language.trim().is_empty() {
            return Err(SettingsError::InvalidValue("language is empty".into()));
        }
        if (self.default_style == DictationStyle::Custom
            || self.style_shortcuts.contains_key(&DictationStyle::Custom))
            && self.custom_style_prompt.trim().is_empty()
        {
            return Err(SettingsError::InvalidValue(
                "custom style needs a prompt".into(),
            ));
        }
        if self
            .dictionary
            .iter()
            .any(|entry| entry.heard.trim().is_empty() || entry.written.trim().is_empty())
        {
            return Err(SettingsError::InvalidValue(
                "dictionary entries need both recognized and written text".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_settings_keep_model_selection_without_api_secret() {
        let path = std::env::temp_dir().join(format!(
            "speakcursor-settings-test-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut settings = Settings::default();
        settings.local_models.push(LocalModel {
            id: "model-1".into(),
            name: "Whisper".into(),
            path: PathBuf::from("model-dir"),
        });
        settings.active_model_id = Some("model-1".into());
        settings
            .cloud_models
            .insert(CloudTask::SpeechProcessing, "chosen-model".into());
        settings.save(&path).unwrap();
        let data = fs::read_to_string(&path).unwrap();
        assert!(!data.contains("api_key"));
        assert_eq!(Settings::load(&path).unwrap(), settings);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_selection_of_a_model_that_is_not_saved() {
        let settings = Settings {
            active_model_id: Some("missing".into()),
            ..Settings::default()
        };
        assert!(matches!(
            settings.validate(),
            Err(SettingsError::InvalidValue(_))
        ));
    }
}
