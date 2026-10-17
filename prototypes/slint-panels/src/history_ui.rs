use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, VecModel};
use speakcursor_core::Settings;
use speakcursor_history::{Entry, History};

use crate::{HistoryPanel, HistoryRow, SettingsPanel, settings_ui};

pub fn initialize(panel: &HistoryPanel, settings_panel: &SettingsPanel) {
    let weak = panel.as_weak();
    settings_panel.on_open_history(move || {
        if let Some(panel) = weak.upgrade() {
            panel.set_clear_armed(false);
            refresh(&panel);
            let _ = panel.show();
        }
    });
    let weak = panel.as_weak();
    panel.on_search_changed(move |_| {
        if let Some(panel) = weak.upgrade() {
            refresh(&panel);
        }
    });
    let weak = panel.as_weak();
    panel.on_copy_original(move |id| {
        if let Some(panel) = weak.upgrade() {
            copy(&panel, id as u64, false);
        }
    });
    let weak = panel.as_weak();
    panel.on_copy_processed(move |id| {
        if let Some(panel) = weak.upgrade() {
            copy(&panel, id as u64, true);
        }
    });
    let weak = panel.as_weak();
    panel.on_clear_all(move || {
        if let Some(panel) = weak.upgrade() {
            if !panel.get_clear_armed() {
                panel.set_clear_armed(true);
                panel.set_feedback_text("Нажми ещё раз, чтобы удалить всю историю".into());
                return;
            }
            panel.set_clear_armed(false);
            let result = open().and_then(|mut history| history.clear());
            match result {
                Ok(()) => {
                    refresh(&panel);
                    panel.set_feedback_text("История очищена".into());
                }
                Err(error) => panel.set_feedback_text(error.to_string().into()),
            }
        }
    });
    let weak = panel.as_weak();
    panel.on_retry(move |id| {
        if let Some(panel) = weak.upgrade() {
            retry(&panel, id as u64);
        }
    });
}

fn open() -> std::io::Result<History> {
    let settings = Settings::load(&settings_ui::settings_path()).map_err(std::io::Error::other)?;
    History::open(settings_ui::history_path(), settings.keep_history_forever)
}

fn refresh(panel: &HistoryPanel) {
    match open() {
        Ok(history) => {
            let rows = history
                .search(panel.get_search_query().as_str())
                .into_iter()
                .map(|entry| HistoryRow {
                    id: entry.id as i32,
                    original: entry.original.clone().into(),
                    processed: entry.processed.clone().unwrap_or_default().into(),
                    failed: entry.error.is_some() && entry.processed.is_none(),
                })
                .collect::<Vec<_>>();
            panel.set_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
        }
        Err(error) => panel.set_feedback_text(error.to_string().into()),
    }
}

fn entry(id: u64) -> std::io::Result<Option<Entry>> {
    Ok(open()?
        .entries()
        .iter()
        .find(|entry| entry.id == id)
        .cloned())
}

fn copy(panel: &HistoryPanel, id: u64, processed: bool) {
    let result =
        entry(id).and_then(|entry| entry.ok_or_else(|| std::io::Error::other("Запись не найдена")));
    let text = match result {
        Ok(entry) if processed => entry.processed,
        Ok(entry) => Some(entry.original),
        Err(error) => {
            panel.set_feedback_text(error.to_string().into());
            return;
        }
    };
    let Some(text) = text else {
        panel.set_feedback_text("Обработанного текста пока нет".into());
        return;
    };
    #[cfg(windows)]
    let result = speakcursor_windows::copy_text(&text);
    #[cfg(not(windows))]
    let _ = &text;
    #[cfg(not(windows))]
    let result: std::io::Result<()> = Err(std::io::Error::other("Копирование доступно в Windows"));
    match result {
        Ok(()) => panel.set_feedback_text("Текст скопирован".into()),
        Err(error) => panel.set_feedback_text(error.to_string().into()),
    }
}

fn retry(panel: &HistoryPanel, id: u64) {
    #[cfg(not(windows))]
    {
        let _ = id;
        panel.set_feedback_text("Повтор доступен в Windows".into());
    }
    #[cfg(windows)]
    {
        use speakcursor_dictation::{DictationMode, DictationPipeline};
        let saved = match entry(id) {
            Ok(Some(entry)) => entry,
            Ok(None) => {
                panel.set_feedback_text("Запись не найдена".into());
                return;
            }
            Err(error) => {
                panel.set_feedback_text(error.to_string().into());
                return;
            }
        };
        let weak = panel.as_weak();
        panel.set_feedback_text("Повторная обработка…".into());
        std::thread::spawn(move || {
            let result = (|| -> Result<String, String> {
                let settings = Settings::load(&settings_ui::settings_path())
                    .map_err(|error| error.to_string())?;
                let key = speakcursor_windows::OpenAiKeyStore::new(settings_ui::key_path())
                    .load()
                    .map_err(|error| error.to_string())?;
                let mut pipeline = DictationPipeline::new().map_err(|error| error.to_string())?;
                let mode = saved
                    .style
                    .map(DictationMode::Style)
                    .unwrap_or(DictationMode::PromptEngineer);
                let output = pipeline
                    .process_recognized(&saved.original, &settings, key.as_deref(), mode)
                    .map_err(|error| error.to_string())?;
                let mut history =
                    History::open(settings_ui::history_path(), settings.keep_history_forever)
                        .map_err(|error| error.to_string())?;
                if !history
                    .update_result(id, output.final_text)
                    .map_err(|error| error.to_string())?
                {
                    return Err("Запись уже удалена".into());
                }
                Ok("Готово. Результат сохранён в этой записи".into())
            })();
            let message = result.unwrap_or_else(|error| format!("Повтор не удался: {error}"));
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(panel) = weak.upgrade() {
                    refresh(&panel);
                    panel.set_feedback_text(message.into());
                }
            });
        });
    }
}
