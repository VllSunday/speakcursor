//! Local speech recognition for the first SpeakCursor milestone.
//! Uses the same CTranslate2 model directory format as faster-whisper.

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use ct2rs::Whisper;
use ct2rs::sys::{ComputeType, Config, Device};

pub const SAMPLE_RATE: u32 = 16_000;
const REQUIRED_MODEL_FILES: &[&str] = &[
    "model.bin",
    "config.json",
    "tokenizer.json",
    "preprocessor_config.json",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessingDevice {
    Cpu,
    Cuda,
}

#[derive(Debug)]
pub enum SpeechError {
    Io(std::io::Error),
    MissingModelFile(PathBuf),
    InvalidAudio(&'static str),
    CudaUnavailable,
    Model(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for SpeechError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "model cache I/O error: {error}"),
            Self::MissingModelFile(path) => write!(f, "model file is missing: {}", path.display()),
            Self::InvalidAudio(message) => write!(f, "invalid audio: {message}"),
            Self::CudaUnavailable => write!(f, "NVIDIA GPU support is not included in this build"),
            Self::Model(error) => write!(f, "speech model error: {error}"),
        }
    }
}

impl Error for SpeechError {}

/// Loaded model. Keep it on a worker thread, then unload by dropping it.
pub struct LocalWhisper {
    model: Whisper,
}

impl LocalWhisper {
    pub fn open(model_dir: &Path, device: ProcessingDevice) -> Result<Self, SpeechError> {
        #[cfg(not(feature = "cuda"))]
        if device == ProcessingDevice::Cuda {
            return Err(SpeechError::CudaUnavailable);
        }
        validate_model_directory(model_dir)?;
        let config = match device {
            ProcessingDevice::Cpu => Config {
                device: Device::CPU,
                compute_type: ComputeType::INT8,
                ..Config::default()
            },
            ProcessingDevice::Cuda => Config {
                device: Device::CUDA,
                compute_type: ComputeType::FLOAT16,
                ..Config::default()
            },
        };
        let model =
            Whisper::new(model_dir, config).map_err(|error| SpeechError::Model(error.into()))?;
        Ok(Self { model })
    }

    /// Expects mono f32 samples at 16 kHz. The caller captures and resamples audio.
    pub fn transcribe(&self, samples: &[f32], language: &str) -> Result<String, SpeechError> {
        if samples
            .iter()
            .any(|sample| !sample.is_finite() || sample.abs() > 1.0)
        {
            return Err(SpeechError::InvalidAudio(
                "samples must be finite values in [-1, 1]",
            ));
        }
        if samples.is_empty() {
            return Ok(String::new());
        }
        if self.model.sampling_rate() != SAMPLE_RATE as usize {
            return Err(SpeechError::InvalidAudio(
                "model expects another sampling rate",
            ));
        }
        let language = if language == "auto" {
            None
        } else {
            Some(language)
        };
        let segments = self
            .model
            .generate(samples, language, false, &Default::default())
            .map_err(|error| SpeechError::Model(error.into()))?;
        Ok(segments.join(" ").trim().to_owned())
    }
}

pub fn validate_model_directory(model_dir: &Path) -> Result<(), SpeechError> {
    for name in REQUIRED_MODEL_FILES {
        let path = model_dir.join(name);
        if !path.is_file() {
            return Err(SpeechError::MissingModelFile(path));
        }
    }
    Ok(())
}

/// Finds a previously downloaded faster-whisper model in a Hugging Face Hub
/// cache. `large-v3` maps to `Systran/faster-whisper-large-v3`, the model name
/// and source used by the previous Python application.
pub fn find_cached_faster_whisper_model(
    hub_cache: &Path,
    name: &str,
) -> Result<Option<PathBuf>, SpeechError> {
    let snapshots = hub_cache
        .join(format!("models--Systran--faster-whisper-{name}"))
        .join("snapshots");
    let entries = match std::fs::read_dir(snapshots) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(SpeechError::Io(error)),
    };
    for entry in entries {
        let entry = entry.map_err(SpeechError::Io)?;
        if validate_model_directory(&entry.path()).is_ok() {
            return Ok(Some(entry.path()));
        }
    }
    Ok(None)
}

pub fn default_hugging_face_hub_cache() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("HF_HUB_CACHE") {
        return Some(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("HUGGINGFACE_HUB_CACHE") {
        return Some(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("HF_HOME") {
        return Some(PathBuf::from(path).join("hub"));
    }
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|home| {
            PathBuf::from(home)
                .join(".cache")
                .join("huggingface")
                .join("hub")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_when_selected_model_is_not_a_ctranslate2_directory() {
        let path = std::env::temp_dir().join(format!(
            "speakcursor-no-model-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(matches!(
            validate_model_directory(&path),
            Err(SpeechError::MissingModelFile(_))
        ));
    }

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn gpu_selection_never_silently_runs_on_cpu() {
        let error = LocalWhisper::open(Path::new("unused"), ProcessingDevice::Cuda);
        assert!(matches!(error, Err(SpeechError::CudaUnavailable)));
    }

    #[test]
    fn finds_the_previous_large_v3_cache_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "speakcursor-model-cache-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let snapshot = root
            .join("models--Systran--faster-whisper-large-v3")
            .join("snapshots")
            .join("revision");
        std::fs::create_dir_all(&snapshot).unwrap();
        for file in REQUIRED_MODEL_FILES {
            std::fs::write(snapshot.join(file), []).unwrap();
        }
        assert_eq!(
            find_cached_faster_whisper_model(&root, "large-v3").unwrap(),
            Some(snapshot)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
