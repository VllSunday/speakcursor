use std::error::Error;
use std::path::PathBuf;

use slint::ComponentHandle;
use speakcursor_core::{CloudTask, DictationStyle, LocalModel, RecognitionMode, Settings};

use crate::SettingsPanel;

fn settings_dir() -> PathBuf {
    #[cfg(windows)]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("SpeakCursor");
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("SpeakCursor")
}

pub fn settings_path() -> PathBuf {
    settings_dir().join("settings.json")
}

#[cfg(windows)]
pub fn key_path() -> PathBuf {
    settings_dir().join("openai-key.dat")
}

pub fn initialize(panel: &SettingsPanel) -> Result<(), Box<dyn Error>> {
    let settings_path = settings_path();
    let loaded = match Settings::load(&settings_path) {
        Ok(settings) => settings,
        Err(error) => {
            panel.set_feedback_text(format!("Настройки не прочитаны: {error}").into());
            Settings::default()
        }
    };
    fill(panel, &loaded);

    let weak = panel.as_weak();
    let save_path = settings_path.clone();
    panel.on_save_requested(move || {
        if let Some(panel) = weak.upgrade() {
            match read(&panel).and_then(|settings| {
                #[cfg(windows)]
                crate::windows_hotkeys::validate_shortcuts(&settings)?;
                settings
                    .save(&save_path)
                    .map_err(|error| error.to_string())?;
                #[cfg(windows)]
                {
                    let new_key = panel.get_api_key_input().to_string();
                    if !new_key.trim().is_empty() {
                        speakcursor_windows::OpenAiKeyStore::new(
                            save_path.with_file_name("openai-key.dat"),
                        )
                        .save(new_key.trim())
                        .map_err(|error| error.to_string())?;
                        panel.set_api_key_input("".into());
                    }
                    crate::windows_hotkeys::reload();
                }
                Ok(())
            }) {
                Ok(()) => panel.set_feedback_text("Настройки сохранены".into()),
                Err(error) => panel.set_feedback_text(error.into()),
            }
        }
    });

    let weak = panel.as_weak();
    panel.on_delete_key_requested(move || {
        if let Some(panel) = weak.upgrade() {
            #[cfg(windows)]
            let result = speakcursor_windows::OpenAiKeyStore::new(key_path()).delete();
            #[cfg(not(windows))]
            let result: std::io::Result<()> = Ok(());
            match result {
                Ok(()) => {
                    panel.set_api_key_input("".into());
                    panel.set_feedback_text("Ключ OpenAI удалён".into());
                }
                Err(error) => panel.set_feedback_text(error.to_string().into()),
            }
        }
    });
    Ok(())
}

fn fill(panel: &SettingsPanel, settings: &Settings) {
    panel.set_use_cloud(settings.recognition_mode == RecognitionMode::OpenAi);
    panel.set_launch_at_login(settings.launch_at_login);
    panel.set_local_model_path(
        settings
            .active_model_id
            .as_ref()
            .and_then(|id| settings.local_models.iter().find(|model| &model.id == id))
            .map(|model| model.path.to_string_lossy().to_string())
            .unwrap_or_default()
            .into(),
    );
    panel.set_language_code(settings.language.clone().into());
    panel.set_asr_model(model(settings, CloudTask::SpeechRecognition).into());
    panel.set_processing_model(model(settings, CloudTask::SpeechProcessing).into());
    panel.set_prompt_model(model(settings, CloudTask::PromptGeneration).into());
    panel.set_default_style_name(
        match settings.default_style {
            DictationStyle::Raw => "Без обработки",
            DictationStyle::Proofread => "Пунктуация и ошибки",
            DictationStyle::Structure => "ТЗ / README",
            DictationStyle::Custom => "Свой стиль",
        }
        .into(),
    );
    panel.set_custom_prompt(settings.custom_style_prompt.clone().into());
    panel.set_general_shortcut(settings.dictation_shortcut.clone().into());
    panel.set_raw_shortcut(shortcut(settings, DictationStyle::Raw).into());
    panel.set_proofread_shortcut(shortcut(settings, DictationStyle::Proofread).into());
    panel.set_structure_shortcut(shortcut(settings, DictationStyle::Structure).into());
    panel.set_custom_shortcut(shortcut(settings, DictationStyle::Custom).into());
    panel.set_prompt_shortcut(settings.prompt_shortcut.clone().into());
    panel.set_unload_minutes(
        settings
            .unload_after_minutes
            .map(|value| value.to_string())
            .unwrap_or_default()
            .into(),
    );
}

fn model(settings: &Settings, task: CloudTask) -> String {
    settings
        .cloud_models
        .get(&task)
        .cloned()
        .unwrap_or_default()
}

fn shortcut(settings: &Settings, style: DictationStyle) -> String {
    settings
        .style_shortcuts
        .get(&style)
        .cloned()
        .unwrap_or_default()
}

fn read(panel: &SettingsPanel) -> Result<Settings, String> {
    let mut settings = Settings::load(&settings_path()).unwrap_or_default();
    settings.recognition_mode = if panel.get_use_cloud() {
        RecognitionMode::OpenAi
    } else {
        RecognitionMode::Local
    };
    settings.launch_at_login = panel.get_launch_at_login();
    settings.language = panel.get_language_code().trim().to_owned();
    let model_path = panel.get_local_model_path().trim().to_owned();
    settings.local_models.retain(|model| model.id != "manual");
    if model_path.is_empty() && settings.active_model_id.as_deref() == Some("manual") {
        settings.active_model_id = None;
    }
    if !model_path.is_empty() {
        settings.local_models.push(LocalModel {
            id: "manual".into(),
            name: "Выбранная модель".into(),
            path: PathBuf::from(model_path),
        });
        settings.active_model_id = Some("manual".into());
    }
    for (task, value) in [
        (CloudTask::SpeechRecognition, panel.get_asr_model()),
        (CloudTask::SpeechProcessing, panel.get_processing_model()),
        (CloudTask::PromptGeneration, panel.get_prompt_model()),
    ] {
        if value.trim().is_empty() {
            settings.cloud_models.remove(&task);
        } else {
            settings.cloud_models.insert(task, value.trim().to_owned());
        }
    }
    settings.default_style = match panel.get_default_style_name().as_str() {
        "Без обработки" => DictationStyle::Raw,
        "Пунктуация и ошибки" => DictationStyle::Proofread,
        "ТЗ / README" => DictationStyle::Structure,
        "Свой стиль" => DictationStyle::Custom,
        _ => return Err("Неизвестный стиль диктовки".into()),
    };
    settings.custom_style_prompt = panel.get_custom_prompt().trim().to_owned();
    settings.dictation_shortcut = panel.get_general_shortcut().trim().to_owned();
    settings.prompt_shortcut = panel.get_prompt_shortcut().trim().to_owned();
    for (style, value) in [
        (DictationStyle::Raw, panel.get_raw_shortcut()),
        (DictationStyle::Proofread, panel.get_proofread_shortcut()),
        (DictationStyle::Structure, panel.get_structure_shortcut()),
        (DictationStyle::Custom, panel.get_custom_shortcut()),
    ] {
        if value.trim().is_empty() {
            settings.style_shortcuts.remove(&style);
        } else {
            settings
                .style_shortcuts
                .insert(style, value.trim().to_owned());
        }
    }
    let unload = panel.get_unload_minutes().trim().to_owned();
    settings.unload_after_minutes = if unload.is_empty() {
        None
    } else {
        Some(
            unload
                .parse()
                .map_err(|_| "Время выгрузки должно быть числом минут")?,
        )
    };
    settings.validate().map_err(|error| error.to_string())?;
    Ok(settings)
}
