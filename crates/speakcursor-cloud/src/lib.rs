//! OpenAI operations used by the first dictation milestone.
//! API credentials are supplied per call and are never serialized to settings.

use std::error::Error;
use std::fmt;
use std::time::Duration;

use reqwest::blocking::{Client, multipart};
use serde_json::{Value, json};
use speakcursor_core::DictationStyle;

const API_BASE: &str = "https://api.openai.com/v1";

#[derive(Debug)]
pub enum CloudError {
    MissingKey,
    MissingModel,
    MissingCustomPrompt,
    EmptyAudio,
    Transport(reqwest::Error),
    HttpStatus(u16),
    UnexpectedResponse,
}

impl fmt::Display for CloudError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingKey => write!(f, "OpenAI API key is missing"),
            Self::MissingModel => write!(f, "no OpenAI model is selected for this task"),
            Self::MissingCustomPrompt => write!(f, "custom style prompt is empty"),
            Self::EmptyAudio => write!(f, "recorded audio is empty"),
            Self::Transport(error) => write!(f, "OpenAI request failed: {error}"),
            Self::HttpStatus(status) => write!(f, "OpenAI returned HTTP {status}"),
            Self::UnexpectedResponse => write!(f, "OpenAI returned an unexpected response"),
        }
    }
}

impl Error for CloudError {}

pub struct OpenAiClient {
    http: Client,
}

impl OpenAiClient {
    pub fn new() -> Result<Self, CloudError> {
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(CloudError::Transport)?;
        Ok(Self { http })
    }

    /// Verifies the key and returns the model IDs visible to this account.
    pub fn list_models(&self, key: &str) -> Result<Vec<String>, CloudError> {
        check_key(key)?;
        let response = self
            .http
            .get(format!("{API_BASE}/models"))
            .bearer_auth(key)
            .send()
            .map_err(CloudError::Transport)?;
        let body = checked_json(response)?;
        let models = body["data"]
            .as_array()
            .ok_or(CloudError::UnexpectedResponse)?;
        Ok(models
            .iter()
            .filter_map(|model| model["id"].as_str().map(str::to_owned))
            .collect())
    }

    /// `wav` must be a complete WAV file, not raw PCM bytes.
    pub fn transcribe_wav(
        &self,
        key: &str,
        model: &str,
        wav: Vec<u8>,
        language: &str,
    ) -> Result<String, CloudError> {
        check_key_and_model(key, model)?;
        if wav.is_empty() {
            return Err(CloudError::EmptyAudio);
        }
        let audio = multipart::Part::bytes(wav)
            .file_name("dictation.wav")
            .mime_str("audio/wav")
            .map_err(CloudError::Transport)?;
        let mut form = multipart::Form::new()
            .text("model", model.to_owned())
            .part("file", audio);
        if language != "auto" && !language.is_empty() {
            form = form.text("language", language.to_owned());
        }
        let response = self
            .http
            .post(format!("{API_BASE}/audio/transcriptions"))
            .bearer_auth(key)
            .multipart(form)
            .send()
            .map_err(CloudError::Transport)?;
        checked_json(response)?["text"]
            .as_str()
            .map(str::to_owned)
            .ok_or(CloudError::UnexpectedResponse)
    }

    pub fn process_dictation(
        &self,
        key: &str,
        model: &str,
        style: DictationStyle,
        custom_prompt: &str,
        text: &str,
    ) -> Result<String, CloudError> {
        let instructions = match style {
            DictationStyle::Raw => return Ok(text.to_owned()),
            DictationStyle::Proofread => {
                "Исправь пунктуацию и ошибки в словах. Сохрани смысл, формулировки и язык. Верни только готовый текст."
            }
            DictationStyle::Structure => {
                "Оформи надиктованный текст как понятный структурированный материал для ТЗ или README. Не добавляй факты, которых нет в исходном тексте. Верни только готовый текст."
            }
            DictationStyle::Custom => {
                if custom_prompt.trim().is_empty() {
                    return Err(CloudError::MissingCustomPrompt);
                }
                custom_prompt
            }
        };
        self.generate_text(key, model, instructions, text)
    }

    pub fn prepare_prompt(&self, key: &str, model: &str, text: &str) -> Result<String, CloudError> {
        self.generate_text(
            key,
            model,
            "Преобразуй надиктованную мысль в чёткий структурированный промпт для ИИ. Убери слова-паразиты, сохрани намерение и важные ограничения, не выдумывай требования. Верни только готовый промпт.",
            text,
        )
    }

    fn generate_text(
        &self,
        key: &str,
        model: &str,
        instructions: &str,
        input: &str,
    ) -> Result<String, CloudError> {
        check_key_and_model(key, model)?;
        let response = self
            .http
            .post(format!("{API_BASE}/responses"))
            .bearer_auth(key)
            .json(&json!({"model": model, "instructions": instructions, "input": input}))
            .send()
            .map_err(CloudError::Transport)?;
        extract_response_text(&checked_json(response)?)
    }
}

fn check_key(key: &str) -> Result<(), CloudError> {
    if key.trim().is_empty() {
        Err(CloudError::MissingKey)
    } else {
        Ok(())
    }
}

fn check_key_and_model(key: &str, model: &str) -> Result<(), CloudError> {
    check_key(key)?;
    if model.trim().is_empty() {
        Err(CloudError::MissingModel)
    } else {
        Ok(())
    }
}

fn checked_json(response: reqwest::blocking::Response) -> Result<Value, CloudError> {
    if !response.status().is_success() {
        return Err(CloudError::HttpStatus(response.status().as_u16()));
    }
    response.json().map_err(CloudError::Transport)
}

fn extract_response_text(response: &Value) -> Result<String, CloudError> {
    let mut parts = Vec::new();
    if let Some(output) = response["output"].as_array() {
        for item in output {
            if let Some(content) = item["content"].as_array() {
                for part in content {
                    if part["type"] == "output_text" {
                        if let Some(text) = part["text"].as_str() {
                            parts.push(text);
                        }
                    }
                }
            }
        }
    }
    if parts.is_empty() {
        return Err(CloudError::UnexpectedResponse);
    }
    Ok(parts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_only_text_from_a_response_with_multiple_items() {
        let response = json!({"output": [
            {"type": "reasoning", "content": []},
            {"type": "message", "content": [
                {"type": "output_text", "text": "Первая строка"},
                {"type": "output_text", "text": "Вторая строка"}
            ]}
        ]});
        assert_eq!(
            extract_response_text(&response).unwrap(),
            "Первая строка\nВторая строка"
        );
    }

    #[test]
    fn raw_style_needs_no_cloud_key_or_model() {
        let client = OpenAiClient::new().unwrap();
        assert_eq!(
            client
                .process_dictation("", "", DictationStyle::Raw, "", "Привет")
                .unwrap(),
            "Привет"
        );
    }
}
