//! Connects captured audio, the selected recognizer, local replacements, and
//! the selected text transformation. The UI owns capture and insertion.

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use speakcursor_audio::encode_wav_16khz_mono;
use speakcursor_cloud::{CloudError, OpenAiClient};
use speakcursor_core::{CloudTask, DictationStyle, RecognitionMode, Settings};
use speakcursor_speech::{
    LocalWhisper, ProcessingDevice, SpeechError, default_hugging_face_hub_cache,
    find_cached_faster_whisper_model,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DictationMode {
    Style(DictationStyle),
    PromptEngineer,
}

#[derive(Debug)]
pub struct DictationResult {
    pub recognized: String,
    pub final_text: String,
}

#[derive(Debug)]
pub enum PipelineError {
    NoLocalModel,
    NoApiKey,
    NoModelFor(CloudTask),
    Audio(std::io::Error),
    Speech(SpeechError),
    Cloud(CloudError),
    Processing {
        recognized: String,
        source: Box<PipelineError>,
    },
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLocalModel => write!(f, "no local CTranslate2 Whisper model was found"),
            Self::NoApiKey => write!(f, "OpenAI API key is not configured"),
            Self::NoModelFor(task) => write!(f, "no OpenAI model selected for {task:?}"),
            Self::Audio(error) => write!(f, "audio preparation failed: {error}"),
            Self::Speech(error) => write!(f, "local recognition failed: {error}"),
            Self::Cloud(error) => write!(f, "cloud recognition failed: {error}"),
            Self::Processing { source, .. } => write!(f, "text processing failed: {source}"),
        }
    }
}

impl Error for PipelineError {}

struct CachedModel {
    path: PathBuf,
    device: ProcessingDevice,
    model: LocalWhisper,
    last_used: Instant,
}

/// Intended to live on a single worker thread so a local model can be reused
/// across dictations without blocking Slint's UI thread.
pub struct DictationPipeline {
    cloud: OpenAiClient,
    local: Option<CachedModel>,
}

impl DictationPipeline {
    pub fn new() -> Result<Self, CloudError> {
        Ok(Self {
            cloud: OpenAiClient::new()?,
            local: None,
        })
    }

    pub fn unload_if_idle(&mut self, minutes: Option<u32>) {
        if let (Some(minutes), Some(loaded)) = (minutes, &self.local) {
            if loaded.last_used.elapsed() >= Duration::from_secs(minutes as u64 * 60) {
                self.local = None;
            }
        }
    }

    pub fn process(
        &mut self,
        samples_16khz: &[f32],
        settings: &Settings,
        api_key: Option<&str>,
        device: ProcessingDevice,
        mode: DictationMode,
    ) -> Result<DictationResult, PipelineError> {
        self.unload_if_idle(settings.unload_after_minutes);
        let recognized = match settings.recognition_mode {
            RecognitionMode::Local => {
                let path = selected_local_model(settings)?;
                if self
                    .local
                    .as_ref()
                    .is_none_or(|loaded| loaded.path != path || loaded.device != device)
                {
                    self.local = Some(CachedModel {
                        path: path.clone(),
                        device,
                        model: LocalWhisper::open(&path, device).map_err(PipelineError::Speech)?,
                        last_used: Instant::now(),
                    });
                }
                let loaded = self.local.as_mut().expect("model just loaded");
                let text = loaded
                    .model
                    .transcribe(samples_16khz, &settings.language)
                    .map_err(PipelineError::Speech)?;
                loaded.last_used = Instant::now();
                text
            }
            RecognitionMode::OpenAi => {
                let key = api_key
                    .filter(|key| !key.trim().is_empty())
                    .ok_or(PipelineError::NoApiKey)?;
                let model = selected_cloud_model(settings, CloudTask::SpeechRecognition)?;
                let wav = encode_wav_16khz_mono(samples_16khz).map_err(PipelineError::Audio)?;
                self.cloud
                    .transcribe_wav(key, model, wav, &settings.language)
                    .map_err(PipelineError::Cloud)?
            }
        };
        self.process_recognized(&recognized, settings, api_key, mode)
    }

    /// Reprocesses a saved transcription without microphone capture or ASR.
    pub fn process_recognized(
        &self,
        text: &str,
        settings: &Settings,
        api_key: Option<&str>,
        mode: DictationMode,
    ) -> Result<DictationResult, PipelineError> {
        let recognized = apply_dictionary(text, settings);
        let final_text = (|| -> Result<String, PipelineError> {
            match mode {
                DictationMode::Style(DictationStyle::Raw) => Ok(recognized.clone()),
                DictationMode::Style(style) => {
                    let key = api_key
                        .filter(|key| !key.trim().is_empty())
                        .ok_or(PipelineError::NoApiKey)?;
                    let model = selected_cloud_model(settings, CloudTask::SpeechProcessing)?;
                    self.cloud
                        .process_dictation(
                            key,
                            model,
                            style,
                            &settings.custom_style_prompt,
                            &recognized,
                        )
                        .map_err(PipelineError::Cloud)
                }
                DictationMode::PromptEngineer => {
                    let key = api_key
                        .filter(|key| !key.trim().is_empty())
                        .ok_or(PipelineError::NoApiKey)?;
                    let model = selected_cloud_model(settings, CloudTask::PromptGeneration)?;
                    self.cloud
                        .prepare_prompt(key, model, &recognized)
                        .map_err(PipelineError::Cloud)
                }
            }
        })()
        .map_err(|source| PipelineError::Processing {
            recognized: recognized.clone(),
            source: Box::new(source),
        })?;
        Ok(DictationResult {
            recognized,
            final_text,
        })
    }
}

fn selected_cloud_model(settings: &Settings, task: CloudTask) -> Result<&str, PipelineError> {
    settings
        .cloud_models
        .get(&task)
        .map(String::as_str)
        .filter(|model| !model.trim().is_empty())
        .ok_or(PipelineError::NoModelFor(task))
}

fn selected_local_model(settings: &Settings) -> Result<PathBuf, PipelineError> {
    if let Some(id) = &settings.active_model_id {
        return settings
            .local_models
            .iter()
            .find(|model| &model.id == id)
            .map(|model| model.path.clone())
            .ok_or(PipelineError::NoLocalModel);
    }
    let cache = default_hugging_face_hub_cache().ok_or(PipelineError::NoLocalModel)?;
    find_cached_faster_whisper_model(Path::new(&cache), "large-v3")
        .map_err(PipelineError::Speech)?
        .ok_or(PipelineError::NoLocalModel)
}

fn apply_dictionary(text: &str, settings: &Settings) -> String {
    settings
        .dictionary
        .iter()
        .fold(text.to_owned(), |current, entry| {
            if entry.heard.is_empty() {
                current
            } else {
                current.replace(&entry.heard, &entry.written)
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use speakcursor_core::DictionaryEntry;

    #[test]
    fn processing_failure_retains_recognized_text_for_clipboard_or_retry() {
        let mut settings = Settings::default();
        settings.dictionary.push(DictionaryEntry {
            heard: "опен ай".into(),
            written: "OpenAI".into(),
        });
        let pipeline = DictationPipeline::new().unwrap();
        let result = pipeline.process_recognized(
            "проверь опен ай",
            &settings,
            None,
            DictationMode::Style(DictationStyle::Proofread),
        );
        assert!(matches!(
            result,
            Err(PipelineError::Processing { recognized, .. }) if recognized == "проверь OpenAI"
        ));
    }
}
