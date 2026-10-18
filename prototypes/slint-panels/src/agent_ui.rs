//! Minimal agent conversation. Network, recording and playback never block
//! Slint's event loop; the entire conversation stays in memory for this run.

use std::{
    ffi::c_void,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Mutex, mpsc},
};

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use speakcursor_audio::{AudioCapture, play_wav_blocking, stop_playback};
use speakcursor_cloud::{ChatTurn, OpenAiClient};
use speakcursor_core::{CloudTask, LocalCompute, Settings};
use speakcursor_dictation::DictationPipeline;
use speakcursor_speech::ProcessingDevice;
use speakcursor_windows::{OpenAiKeyStore, copy_text, paste_text_if_foreground};

use crate::{AgentPanel, settings_ui, windows_focus, windows_hotkeys};

#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(
        hwnd: *mut c_void,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show: i32,
    ) -> *mut c_void;
}
#[link(name = "user32")]
unsafe extern "system" {
    fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
}

#[derive(Clone)]
struct OwnedTurn {
    assistant: bool,
    text: String,
}

#[derive(Default)]
pub struct AgentState {
    turns: Vec<OwnedTurn>,
    selected_text: Option<String>,
    target_window: usize,
    busy: bool,
    voice_stop: Option<mpsc::Sender<()>>,
    voice_finishing: bool,
    generation: u64,
    speech_epoch: Arc<AtomicU64>,
}

pub type SharedAgentState = Arc<Mutex<AgentState>>;

pub fn initialize(panel: &AgentPanel) -> SharedAgentState {
    let state = Arc::new(Mutex::new(AgentState::default()));
    let speech_epoch = state.lock().unwrap().speech_epoch.clone();
    {
        let panel = panel.as_weak();
        let state = Arc::clone(&state);
        let speech_epoch = Arc::clone(&speech_epoch);
        panel
            .upgrade()
            .unwrap()
            .on_submit_question(move |question| {
                submit(
                    panel.clone(),
                    Arc::clone(&state),
                    Arc::clone(&speech_epoch),
                    question.to_string(),
                );
            });
    }
    {
        let panel = panel.as_weak();
        panel.upgrade().unwrap().on_copy_answer(move || {
            if let Some(panel) = panel.upgrade() {
                let text = panel.get_answer_text();
                if !text.trim().is_empty() {
                    match copy_text(&text) {
                        Ok(()) => panel.set_status_text("Ответ скопирован".into()),
                        Err(error) => {
                            panel.set_status_text(format!("Не удалось скопировать: {error}").into())
                        }
                    }
                }
            }
        });
    }
    {
        let panel = panel.as_weak();
        let state = Arc::clone(&state);
        let speech_epoch = Arc::clone(&speech_epoch);
        panel.upgrade().unwrap().on_new_chat(move || {
            speech_epoch.fetch_add(1, Ordering::AcqRel);
            stop_playback();
            if let Ok(mut state) = state.lock() {
                if let Some(stop) = state.voice_stop.take() {
                    let _ = stop.send(());
                }
                state.turns.clear();
                state.selected_text = None;
                state.target_window = 0;
                state.busy = false;
                state.voice_finishing = false;
                state.generation += 1;
            }
            if let Some(panel) = panel.upgrade() {
                panel.set_answer_text("".into());
                panel.set_history_text("".into());
                panel.set_result_links(ModelRc::from(Rc::new(VecModel::from(
                    Vec::<SharedString>::new(),
                ))));
                panel.set_selected_preview("".into());
                panel.set_has_selection(false);
                panel.set_question_input("".into());
                panel.set_status_text("Новый диалог".into());
                panel.set_error_page(-1);
                panel.set_recording(false);
                panel.set_busy(false);
            }
        });
    }
    {
        let panel = panel.as_weak();
        let state = Arc::clone(&state);
        let speech_epoch = Arc::clone(&speech_epoch);
        panel.upgrade().unwrap().on_toggle_voice(move || {
            toggle_voice(panel.clone(), Arc::clone(&state), Arc::clone(&speech_epoch));
        });
    }
    {
        let speech_epoch = Arc::clone(&speech_epoch);
        let weak = panel.as_weak();
        panel.on_stop_speaking(move || {
            speech_epoch.fetch_add(1, Ordering::AcqRel);
            stop_playback();
            if let Some(panel) = weak.upgrade() {
                panel.set_status_text("Озвучивание остановлено".into());
            }
        });
    }
    {
        let weak = panel.as_weak();
        let state = Arc::clone(&state);
        panel.on_apply_answer(move || {
            let Some(panel) = weak.upgrade() else {
                return;
            };
            let answer = panel.get_answer_text().to_string();
            let target = state
                .lock()
                .ok()
                .map(|state| state.target_window)
                .unwrap_or_default();
            if answer.trim().is_empty() {
                return;
            }
            if let Err(error) = copy_text(&answer) {
                panel.set_status_text(format!("Копирование не удалось: {error}").into());
                return;
            }
            if target == 0 || unsafe { SetForegroundWindow(target as *mut c_void) } == 0 {
                panel.set_status_text("Текст в буфере · вернись в редактор и нажми Ctrl+V".into());
                return;
            }
            let weak = weak.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(70));
                let result = paste_text_if_foreground(&answer, target);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(panel) = weak.upgrade() {
                        panel.set_status_text(match result {
                            Ok(()) => "Вставлено".into(),
                            Err(error) => format!(
                                "Вставка не удалась: {error}. Текст в буфере — нажми Ctrl+V"
                            )
                            .into(),
                        });
                    }
                });
            });
        });
    }
    {
        let weak = panel.as_weak();
        panel.on_open_link(move |url| {
            let url = url.to_string();
            if !url.starts_with("https://") && !url.starts_with("http://") {
                return;
            }
            let action = "open\0".encode_utf16().collect::<Vec<_>>();
            let url = format!("{url}\0").encode_utf16().collect::<Vec<_>>();
            let opened = unsafe {
                ShellExecuteW(
                    std::ptr::null_mut(),
                    action.as_ptr(),
                    url.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    1,
                )
            } as isize;
            if opened <= 32 {
                if let Some(panel) = weak.upgrade() {
                    panel.set_status_text("Не удалось открыть ссылку".into());
                }
            }
        });
    }
    state
}

pub fn open(
    panel: slint::Weak<AgentPanel>,
    state: SharedAgentState,
    selected: Option<String>,
    target: usize,
) {
    let (preview, reset, has_selection, recording) = {
        let mut state = match state.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        let selected = selected.filter(|text| !text.trim().is_empty());
        let reset = selected != state.selected_text;
        if reset {
            if let Some(stop) = state.voice_stop.take() {
                let _ = stop.send(());
            }
            state.selected_text = selected;
            state.turns.clear();
            state.busy = false;
            state.voice_finishing = false;
            state.generation += 1;
            state.speech_epoch.fetch_add(1, Ordering::AcqRel);
            stop_playback();
        }
        state.target_window = target;
        let preview = state
            .selected_text
            .as_ref()
            .map(|text| format!("Выделено: {}", text.chars().take(110).collect::<String>()))
            .unwrap_or_else(|| "Вопрос без выделенного текста".into());
        (
            preview,
            reset,
            state.selected_text.is_some(),
            state.voice_stop.is_some(),
        )
    };
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(panel) = panel.upgrade() {
            panel.set_has_selection(has_selection);
            panel.set_recording(recording);
            panel.set_selected_preview(preview.into());
            if reset {
                panel.set_answer_text("".into());
                panel.set_history_text("".into());
                panel.set_result_links(ModelRc::from(Rc::new(VecModel::from(
                    Vec::<SharedString>::new(),
                ))));
                panel.set_busy(false);
                panel.set_status_text("Готов к вопросу".into());
                panel.set_error_page(-1);
            }
            if let Err(error) = windows_focus::place_near_screen_edge(panel.window(), true, target)
            {
                panel.set_status_text(format!("Не удалось разместить окно: {error}").into());
            }
            let _ = panel.show();
            panel.invoke_focus_question();
        }
    });
}

pub fn close(state: &SharedAgentState) {
    if let Ok(mut state) = state.lock() {
        if let Some(stop) = state.voice_stop.take() {
            let _ = stop.send(());
        }
        state.speech_epoch.fetch_add(1, Ordering::AcqRel);
        state.generation += 1;
        state.busy = false;
        state.voice_finishing = false;
    }
    stop_playback();
}

fn submit(
    panel: slint::Weak<AgentPanel>,
    state: SharedAgentState,
    speech_epoch: Arc<AtomicU64>,
    question: String,
) {
    let question = question.trim().to_owned();
    if question.is_empty() {
        return;
    }
    let (history, selection, generation) = {
        let mut state = match state.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        if state.busy {
            return;
        }
        state.busy = true;
        state.generation += 1;
        (
            state.turns.clone(),
            state.selected_text.clone(),
            state.generation,
        )
    };
    let speech_request_epoch = speech_epoch.fetch_add(1, Ordering::AcqRel) + 1;
    let history_prefix = history
        .iter()
        .map(|turn| {
            format!(
                "{}: {}",
                if turn.assistant { "Агент" } else { "Вы" },
                turn.text
            )
        })
        .chain(std::iter::once(format!("Вы: {question}")))
        .collect::<Vec<_>>()
        .join("\n\n");
    stop_playback();
    if let Some(panel) = panel.upgrade() {
        panel.set_busy(true);
        panel.set_answer_text("".into());
        panel.set_history_text(format!("{history_prefix}\n\nАгент: …").into());
        panel.set_error_page(-1);
        panel.set_result_links(ModelRc::from(Rc::new(VecModel::from(
            Vec::<SharedString>::new(),
        ))));
        panel.set_status_text("Получаю ответ…".into());
    }
    std::thread::spawn(move || {
        let result = (|| -> Result<(String, Settings, String, String), String> {
            let settings =
                Settings::load(&settings_ui::settings_path()).map_err(|e| e.to_string())?;
            let task = if selection.is_some() {
                CloudTask::SelectedText
            } else {
                CloudTask::Assistant
            };
            let model = settings
                .cloud_models
                .get(&task)
                .filter(|s| !s.trim().is_empty())
                .ok_or("Выбери модель агента в настройках")?
                .clone();
            let key = OpenAiKeyStore::new(settings_ui::key_path())
                .load()
                .map_err(|e| e.to_string())?
                .ok_or("Сохрани ключ OpenAI в настройках")?;
            let client = OpenAiClient::new().map_err(|e| e.to_string())?;
            let turns = history
                .iter()
                .map(|turn| ChatTurn {
                    assistant: turn.assistant,
                    text: &turn.text,
                })
                .collect::<Vec<_>>();
            let streamed = Arc::new(Mutex::new(String::new()));
            let output = Arc::clone(&streamed);
            let panel_for_delta = panel.clone();
            let state_for_delta = Arc::clone(&state);
            let history_prefix_for_delta = history_prefix.clone();
            let answer = client
                .answer_chat_streaming(
                    &key,
                    &model,
                    &question,
                    &turns,
                    selection.as_deref(),
                    move |delta| {
                        let rendered = if let Ok(mut output) = output.lock() {
                            output.push_str(delta);
                            output.clone()
                        } else {
                            return;
                        };
                        let panel = panel_for_delta.clone();
                        let state = Arc::clone(&state_for_delta);
                        let history_prefix = history_prefix_for_delta.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if state
                                .lock()
                                .ok()
                                .is_some_and(|state| state.generation == generation)
                            {
                                if let Some(panel) = panel.upgrade() {
                                    panel.set_answer_text(rendered.into());
                                    panel.set_history_text(
                                        format!(
                                            "{}\n\nАгент: {}",
                                            history_prefix,
                                            panel.get_answer_text()
                                        )
                                        .into(),
                                    );
                                }
                            }
                        });
                    },
                )
                .map_err(|e| e.to_string())?;
            Ok((answer, settings, key, model))
        })();
        let (speech, status) = match result {
            Ok((answer, settings, key, _model)) => {
                let (history_text, copied) = {
                    let mut state = match state.lock() {
                        Ok(state) => state,
                        Err(_) => return,
                    };
                    if state.generation != generation {
                        return;
                    }
                    let copied = if selection.is_some() {
                        copy_text(&answer).map_err(|error| error.to_string()).err()
                    } else {
                        None
                    };
                    state.busy = false;
                    state.turns.push(OwnedTurn {
                        assistant: false,
                        text: question.clone(),
                    });
                    state.turns.push(OwnedTurn {
                        assistant: true,
                        text: answer.clone(),
                    });
                    let history_text = state
                        .turns
                        .iter()
                        .map(|turn| {
                            format!(
                                "{}: {}",
                                if turn.assistant { "Агент" } else { "Вы" },
                                turn.text
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    (history_text, copied)
                };
                let panel_done = panel.clone();
                let state_done = Arc::clone(&state);
                let answer_done = answer.clone();
                let links = windows_hotkeys::extract_urls(&answer)
                    .into_iter()
                    .map(SharedString::from)
                    .collect::<Vec<_>>();
                let _ = slint::invoke_from_event_loop(move || {
                    if !state_done
                        .lock()
                        .ok()
                        .is_some_and(|state| state.generation == generation)
                    {
                        return;
                    }
                    if let Some(panel) = panel_done.upgrade() {
                        panel.set_answer_text(answer_done.into());
                        panel.set_history_text(history_text.into());
                        panel.set_result_links(ModelRc::from(Rc::new(VecModel::from(links))));
                        if !windows_hotkeys::extract_urls(&panel.get_answer_text()).is_empty() {
                            panel.set_expanded(true);
                        }
                        panel.set_question_input("".into());
                        panel.set_busy(false);
                        panel.set_status_text(if let Some(error) = copied {
                            format!("Ответ готов, копирование не удалось: {error}").into()
                        } else if panel.get_has_selection() {
                            "Ответ скопирован · Ctrl+V или «Вставить»".into()
                        } else {
                            "Ответ готов · можно уточнить".into()
                        });
                    }
                });
                (
                    if settings.speak_answers {
                        Some((answer, settings, key))
                    } else {
                        None
                    },
                    String::new(),
                )
            }
            Err(error) => {
                let valid = state
                    .lock()
                    .ok()
                    .is_some_and(|state| state.generation == generation);
                if let Ok(mut state) = state.lock() {
                    if state.generation == generation {
                        state.busy = false;
                    }
                }
                if valid {
                    (None, error)
                } else {
                    return;
                }
            }
        };
        if !status.is_empty() {
            let state_for_error = Arc::clone(&state);
            let _ = slint::invoke_from_event_loop(move || {
                if !state_for_error
                    .lock()
                    .ok()
                    .is_some_and(|state| state.generation == generation)
                {
                    return;
                }
                if let Some(panel) = panel.upgrade() {
                    panel.set_busy(false);
                    panel.set_status_text(format!("Запрос не выполнен: {status}").into());
                    panel.set_error_page(if status.contains("ключ OpenAI") {
                        0
                    } else if status.contains("модель") {
                        5
                    } else {
                        -1
                    });
                }
            });
            return;
        }
        if let Some((answer, settings, key)) = speech {
            speak(
                panel,
                speech_epoch,
                speech_request_epoch,
                settings,
                key,
                answer,
            );
        }
    });
}

fn toggle_voice(
    panel: slint::Weak<AgentPanel>,
    state: SharedAgentState,
    speech_epoch: Arc<AtomicU64>,
) {
    let (sender, receiver) = mpsc::channel();
    let generation = {
        let mut state = match state.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        if let Some(stop) = state.voice_stop.take() {
            let _ = stop.send(());
            state.voice_finishing = true;
            if let Some(panel) = panel.upgrade() {
                panel.set_recording(false);
                panel.set_status_text("Распознаю вопрос…".into());
            }
            return;
        }
        if state.busy || state.voice_finishing {
            return;
        }
        state.voice_stop = Some(sender);
        state.generation += 1;
        state.generation
    };
    if let Some(panel) = panel.upgrade() {
        panel.set_recording(true);
        panel.set_status_text("Слушаю · нажми «Стоп» для отправки".into());
        panel.set_error_page(-1);
    }
    std::thread::spawn(move || {
        let result = (|| -> Result<String, String> {
            let recording = AudioCapture::start_default().map_err(|e| e.to_string())?;
            receiver.recv().map_err(|e| e.to_string())?;
            let samples = recording.finish().map_err(|e| e.to_string())?;
            let settings =
                Settings::load(&settings_ui::settings_path()).map_err(|e| e.to_string())?;
            let key = OpenAiKeyStore::new(settings_ui::key_path())
                .load()
                .map_err(|e| e.to_string())?;
            let device = match settings.local_compute {
                LocalCompute::Cpu => ProcessingDevice::Cpu,
                LocalCompute::NvidiaCuda => ProcessingDevice::Cuda,
            };
            DictationPipeline::new()
                .map_err(|e| e.to_string())?
                .recognize(&samples, &settings, key.as_deref(), device)
                .map_err(|e| e.to_string())
        })();
        let panel = panel.clone();
        let state = Arc::clone(&state);
        let _ = slint::invoke_from_event_loop(move || {
            let valid = if let Ok(mut state) = state.lock() {
                let valid = state.generation == generation;
                if valid {
                    state.voice_stop = None;
                    state.voice_finishing = false;
                }
                valid
            } else {
                false
            };
            if !valid {
                return;
            }
            if let Some(panel_ref) = panel.upgrade() {
                panel_ref.set_recording(false);
            }
            match result {
                Ok(question) => submit(panel, state, speech_epoch, question),
                Err(error) => {
                    if let Some(panel) = panel.upgrade() {
                        panel.set_status_text(format!("Голосовой ввод: {error}").into());
                        panel.set_error_page(if error.contains("ключ OpenAI") {
                            0
                        } else if error.contains("модель") {
                            0
                        } else {
                            -1
                        });
                    }
                }
            }
        });
    });
}

fn speak(
    panel: slint::Weak<AgentPanel>,
    speech_epoch: Arc<AtomicU64>,
    epoch: u64,
    settings: Settings,
    key: String,
    answer: String,
) {
    let result = (|| -> Result<(), String> {
        let model = settings
            .cloud_models
            .get(&CloudTask::SpeechSynthesis)
            .ok_or("Модель озвучивания не выбрана")?;
        let client = OpenAiClient::new().map_err(|e| e.to_string())?;
        for chunk in speech_chunks(&answer) {
            if speech_epoch.load(Ordering::Acquire) != epoch {
                return Ok(());
            }
            let wav = client
                .synthesize_speech(&key, model, &settings.speech_voice, &chunk)
                .map_err(|e| e.to_string())?;
            if speech_epoch.load(Ordering::Acquire) != epoch {
                return Ok(());
            }
            play_wav_blocking(&wav).map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = slint::invoke_from_event_loop(move || {
            if speech_epoch.load(Ordering::Acquire) == epoch
                && let Some(panel) = panel.upgrade()
            {
                panel.set_status_text(
                    format!("Текст готов, озвучивание не удалось: {error}").into(),
                );
                panel.set_error_page(
                    if error.contains("Модель")
                        || error.contains("model")
                        || error.contains("voice")
                    {
                        5
                    } else {
                        -1
                    },
                );
            }
        });
    }
}

fn speech_chunks(text: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for word in text
        .split_whitespace()
        .filter(|word| !word.starts_with("https://") && !word.starts_with("http://"))
    {
        if word.chars().count() > 3500 {
            if !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
            }
            let mut part = String::new();
            for ch in word.chars() {
                if part.chars().count() >= 3500 {
                    chunks.push(std::mem::take(&mut part));
                }
                part.push(ch);
            }
            if !part.is_empty() {
                chunks.push(part);
            }
            continue;
        }
        if current.chars().count() + word.chars().count() + 1 > 3500 && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}
