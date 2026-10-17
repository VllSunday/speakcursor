use std::error::Error;
use std::path::PathBuf;
#[cfg(windows)]
use std::rc::Rc;

use slint::ComponentHandle;
#[cfg(windows)]
use slint::{ModelRc, SharedString, VecModel};
use speakcursor_core::{
    CloudTask, DictationStyle, DictionaryEntry, LocalCompute, LocalModel, RecognitionMode, Settings,
};

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

pub fn history_path() -> PathBuf {
    settings_dir().join("history.json")
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
    #[cfg(windows)]
    {
        let weak = panel.as_weak();
        panel.on_choose_model_folder(move || {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                if let Some(panel) = weak.upgrade() {
                    panel.set_local_model_path(path.to_string_lossy().to_string().into());
                    panel.set_model_status(
                        "Папка изменена · нажми «Сохранить», чтобы выбрать модель".into(),
                    );
                }
            }
        });
    }
    #[cfg(windows)]
    if let Err(error) = speakcursor_windows::set_launch_at_login(loaded.launch_at_login) {
        panel.set_feedback_text(format!("Не удалось настроить автозапуск: {error}").into());
    }

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
                    speakcursor_windows::set_launch_at_login(settings.launch_at_login)
                        .map_err(|error| error.to_string())?;
                    let new_key = panel.get_api_key_input().to_string();
                    if !new_key.trim().is_empty() {
                        speakcursor_windows::OpenAiKeyStore::new(
                            save_path.with_file_name("openai-key.dat"),
                        )
                        .save(new_key.trim())
                        .map_err(|error| error.to_string())?;
                        panel.set_api_key_input("".into());
                        panel.set_show_api_key(false);
                    }
                    crate::windows_hotkeys::reload();
                    verify_cloud_settings(settings, panel.as_weak());
                }
                Ok(())
            }) {
                Ok(()) => {
                    panel.set_feedback_text("Настройки сохранены".into());
                    panel.set_model_status(if panel.get_use_cloud() {
                        "Выбрано распознавание через OpenAI".into()
                    } else {
                        format!(
                            "Выбрана: {} · файлы проверены · загрузится при первой диктовке",
                            local_model_name(&panel.get_local_model_path())
                        )
                        .into()
                    });
                }
                Err(error) => {
                    panel.set_feedback_text(error.clone().into());
                    if error.contains("model") || error.contains("модел") {
                        panel.set_model_status(format!("Модель не выбрана: {error}").into());
                    }
                }
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
                    panel.set_show_api_key(false);
                    panel.set_feedback_text("Ключ OpenAI удалён".into());
                }
                Err(error) => panel.set_feedback_text(error.to_string().into()),
            }
        }
    });
    #[cfg(windows)]
    {
        let weak = panel.as_weak();
        panel.on_show_key_requested(move || {
            if let Some(panel) = weak.upgrade() {
                if panel.get_show_api_key() {
                    panel.set_show_api_key(false);
                } else {
                    if panel.get_api_key_input().is_empty() {
                        match speakcursor_windows::OpenAiKeyStore::new(key_path()).load() {
                            Ok(Some(key)) => panel.set_api_key_input(key.into()),
                            Ok(None) => {
                                panel.set_feedback_text("Ключ OpenAI ещё не сохранён".into());
                                return;
                            }
                            Err(error) => {
                                panel.set_feedback_text(error.to_string().into());
                                return;
                            }
                        }
                    }
                    panel.set_show_api_key(true);
                }
            }
        });
        let weak = panel.as_weak();
        panel.on_refresh_models(move || refresh_model_catalog(weak.clone()));
    }
    Ok(())
}

#[cfg(windows)]
fn refresh_model_catalog(panel: slint::Weak<SettingsPanel>) {
    if let Some(panel) = panel.upgrade() {
        panel.set_feedback_text("Обновляю каталог моделей OpenAI…".into());
    }
    std::thread::spawn(move || {
        let result = (|| -> Result<Vec<String>, String> {
            let key = speakcursor_windows::OpenAiKeyStore::new(key_path())
                .load()
                .map_err(|error| error.to_string())?
                .ok_or("Сначала сохрани ключ OpenAI")?;
            speakcursor_cloud::OpenAiClient::new()
                .map_err(|error| error.to_string())?
                .list_models(&key)
                .map_err(|error| error.to_string())
        })();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(panel) = panel.upgrade() {
                match result {
                    Ok(models) => {
                        let count = models.len();
                        set_model_options(&panel, models);
                        panel
                            .set_feedback_text(format!("Каталог обновлён: {count} моделей").into());
                    }
                    Err(error) => panel.set_feedback_text(error.into()),
                }
            }
        });
    });
}

#[cfg(windows)]
fn set_model_options(panel: &SettingsPanel, models: Vec<String>) {
    let mut transcription = models
        .iter()
        .filter(|name| {
            *name == "whisper-1"
                || name.starts_with("gpt-4o-transcribe")
                || name.starts_with("gpt-4o-mini-transcribe")
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut text = models
        .into_iter()
        .filter(|name| {
            (name.starts_with("gpt-4o")
                || name.starts_with("gpt-4.1")
                || name.starts_with("gpt-5")
                || name.starts_with("o1")
                || name.starts_with("o3")
                || name.starts_with("o4"))
                && !name.contains("transcribe")
                && !name.contains("realtime")
                && !name.contains("audio")
                && !name.contains("tts")
        })
        .collect::<Vec<_>>();
    transcription.sort();
    text.sort();
    panel.set_transcription_model_options(ModelRc::from(Rc::new(VecModel::from(
        transcription
            .into_iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    ))));
    panel.set_text_model_options(ModelRc::from(Rc::new(VecModel::from(
        text.into_iter().map(SharedString::from).collect::<Vec<_>>(),
    ))));
}

#[cfg(windows)]
fn verify_cloud_settings(settings: Settings, panel: slint::Weak<SettingsPanel>) {
    let tasks = [
        CloudTask::SpeechRecognition,
        CloudTask::SpeechProcessing,
        CloudTask::PromptGeneration,
    ];
    if !tasks
        .iter()
        .any(|task| settings.cloud_models.contains_key(task))
    {
        if speakcursor_windows::OpenAiKeyStore::new(key_path())
            .load()
            .ok()
            .flatten()
            .is_some()
        {
            refresh_model_catalog(panel);
        }
        return;
    }
    std::thread::spawn(move || {
        let result = (|| -> Result<Vec<String>, String> {
            let key = speakcursor_windows::OpenAiKeyStore::new(key_path())
                .load()
                .map_err(|error| error.to_string())?
                .ok_or("Ключ OpenAI не сохранён")?;
            let client =
                speakcursor_cloud::OpenAiClient::new().map_err(|error| error.to_string())?;
            let available = client
                .list_models(&key)
                .map_err(|error| error.to_string())?;
            let mut checked_text = std::collections::HashSet::new();
            for task in tasks {
                if let Some(model) = settings.cloud_models.get(&task) {
                    if !available.contains(model) {
                        return Err(format!("Модель {model} не видна для этого ключа"));
                    }
                    match task {
                        CloudTask::SpeechRecognition => client
                            .verify_transcription_model(&key, model)
                            .map_err(|error| format!("Модель распознавания: {error}"))?,
                        CloudTask::SpeechProcessing | CloudTask::PromptGeneration
                            if checked_text.insert(model.clone()) =>
                        {
                            client
                                .verify_text_model(&key, model)
                                .map_err(|error| format!("Модель обработки текста: {error}"))?
                        }
                        _ => {}
                    }
                }
            }
            Ok(available)
        })();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(panel) = panel.upgrade() {
                match result {
                    Ok(models) => {
                        set_model_options(&panel, models);
                        panel.set_feedback_text("Ключ и модели OpenAI проверены".into());
                    }
                    Err(error) => {
                        panel.set_page(4);
                        panel.set_feedback_text(format!("Проверка OpenAI: {error}").into());
                    }
                }
            }
        });
    });
}

fn fill(panel: &SettingsPanel, settings: &Settings) {
    panel.set_use_cloud(settings.recognition_mode == RecognitionMode::OpenAi);
    panel.set_compute_choice(
        match settings.local_compute {
            LocalCompute::Cpu => "CPU",
            LocalCompute::NvidiaCuda => "NVIDIA GPU",
        }
        .into(),
    );
    panel.set_launch_at_login(settings.launch_at_login);
    let saved_path = settings
        .active_model_id
        .as_ref()
        .and_then(|id| settings.local_models.iter().find(|model| &model.id == id))
        .map(|model| model.path.to_string_lossy().to_string());
    #[cfg(not(windows))]
    let chosen_path = saved_path.clone();
    #[cfg(windows)]
    let chosen_path = saved_path.clone().or_else(|| {
        speakcursor_speech::default_hugging_face_hub_cache()
            .and_then(|cache| {
                speakcursor_speech::find_cached_faster_whisper_model(&cache, "large-v3")
                    .ok()
                    .flatten()
            })
            .map(|path| path.to_string_lossy().to_string())
    });
    panel.set_model_status(if settings.recognition_mode == RecognitionMode::OpenAi {
        "Выбрано распознавание через OpenAI".into()
    } else if let Some(path) = &saved_path {
        let folder = PathBuf::from(path);
        let files_present = [
            "model.bin",
            "config.json",
            "tokenizer.json",
            "preprocessor_config.json",
        ]
        .iter()
        .all(|name| folder.join(name).is_file());
        if files_present {
            format!(
                "Выбрана: {} · файлы на месте · загрузится при первой диктовке",
                local_model_name(path)
            )
            .into()
        } else {
            "Сохранённая модель недоступна · выбери папку с файлами CTranslate2".into()
        }
    } else if chosen_path.is_some() {
        "Найдена модель faster-whisper large-v3 · нажми «Сохранить», чтобы выбрать её".into()
    } else {
        "Локальная модель не выбрана · укажи папку с моделью".into()
    });
    panel.set_local_model_path(chosen_path.unwrap_or_default().into());
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
    panel.set_dictionary_rules(
        settings
            .dictionary
            .iter()
            .map(|entry| format!("{} = {}", entry.heard, entry.written))
            .collect::<Vec<_>>()
            .join("\n")
            .into(),
    );
    panel.set_keep_history_forever(settings.keep_history_forever);
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

fn local_model_name(path: &str) -> String {
    if let Some(name) = path
        .split(['/', '\\'])
        .find_map(|part| part.strip_prefix("models--Systran--faster-whisper-"))
    {
        return format!("faster-whisper {name}");
    }
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("локальная модель")
        .to_owned()
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
    let mut settings = Settings::load(&settings_path()).map_err(|error| error.to_string())?;
    settings.recognition_mode = if panel.get_use_cloud() {
        RecognitionMode::OpenAi
    } else {
        RecognitionMode::Local
    };
    settings.local_compute = match panel.get_compute_choice().as_str() {
        "CPU" => LocalCompute::Cpu,
        "NVIDIA GPU" => LocalCompute::NvidiaCuda,
        _ => return Err("Неизвестное устройство для локального распознавания".into()),
    };
    if settings.local_compute == LocalCompute::NvidiaCuda && !cfg!(feature = "cuda") {
        return Err("Поддержка NVIDIA GPU не включена в эту сборку приложения".into());
    }
    settings.launch_at_login = panel.get_launch_at_login();
    settings.language = panel.get_language_code().trim().to_owned();
    let model_path = panel.get_local_model_path().trim().to_owned();
    if settings.recognition_mode == RecognitionMode::Local && model_path.is_empty() {
        return Err(
            "Локальная модель не выбрана. Укажи папку CTranslate2 или включи OpenAI".into(),
        );
    }
    settings.local_models.retain(|model| model.id != "manual");
    if model_path.is_empty() && settings.active_model_id.as_deref() == Some("manual") {
        settings.active_model_id = None;
    }
    if !model_path.is_empty() {
        #[cfg(windows)]
        speakcursor_speech::validate_model_directory(&PathBuf::from(&model_path))
            .map_err(|error| error.to_string())?;
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
    settings.keep_history_forever = panel.get_keep_history_forever();
    settings.dictionary = panel
        .get_dictionary_rules()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let (heard, written) = line
                .split_once('=')
                .ok_or("В словаре каждую пару разделяй знаком =")?;
            if heard.trim().is_empty() || written.trim().is_empty() {
                return Err("В словаре обе части пары должны быть заполнены");
            }
            Ok(DictionaryEntry {
                heard: heard.trim().into(),
                written: written.trim().into(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
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
