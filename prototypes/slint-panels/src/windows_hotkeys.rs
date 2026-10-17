//! Windows hotkeys and dictation worker. All blocking work stays off Slint's UI thread.

use std::{
    collections::HashSet,
    ffi::c_void,
    io,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use slint::ComponentHandle;
use speakcursor_audio::AudioCapture;
use speakcursor_core::{
    DictationAction, DictationController, DictationShortcut, DictationStyle, Settings,
};
use speakcursor_dictation::{DictationMode, DictationPipeline, PipelineError};
use speakcursor_speech::ProcessingDevice;
use speakcursor_windows::{OpenAiKeyStore, copy_text, foreground_window, paste_text_if_foreground};

use crate::{AssistantPanel, RecordingPill, SettingsPanel, settings_ui, windows_focus};

const FIRST_ID: i32 = 100;
const ESC_ID: i32 = 2;
const SETTINGS_ID: i32 = 3;
const ALT: u32 = 1;
const CTRL: u32 = 2;
const SHIFT: u32 = 4;
const WIN: u32 = 8;
const NOREPEAT: u32 = 0x4000;
const WM_HOTKEY: u32 = 0x0312;
const WM_RELOAD: u32 = 0x8004;
static THREAD_ID: AtomicU32 = AtomicU32::new(0);

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
struct Message {
    hwnd: *mut c_void,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    point: Point,
    private: u32,
}
#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterHotKey(hwnd: *mut c_void, id: i32, mods: u32, key: u32) -> i32;
    fn UnregisterHotKey(hwnd: *mut c_void, id: i32) -> i32;
    fn GetMessageW(message: *mut Message, hwnd: *mut c_void, min: u32, max: u32) -> i32;
    fn PostThreadMessageW(thread: u32, message: u32, w_param: usize, l_param: isize) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThreadId() -> u32;
}

#[derive(Clone, Copy)]
struct Binding {
    id: i32,
    mode: DictationMode,
    mods: u32,
    key: u32,
}

pub fn start(
    pill: slint::Weak<RecordingPill>,
    alert: slint::Weak<AssistantPanel>,
    settings_panel: slint::Weak<SettingsPanel>,
) {
    std::thread::spawn(move || {
        if let Err(error) = run(pill, alert.clone(), settings_panel) {
            show_error(&alert, format!("Горячие клавиши недоступны: {error}"));
        }
    });
}

pub fn reload() {
    let id = THREAD_ID.load(Ordering::Acquire);
    if id != 0 {
        unsafe { PostThreadMessageW(id, WM_RELOAD, 0, 0) };
    }
}

pub fn validate_shortcuts(settings: &Settings) -> Result<(), String> {
    bindings(settings).map(|_| ())
}

fn bindings(settings: &Settings) -> Result<Vec<Binding>, String> {
    let mut choices = vec![(
        settings.dictation_shortcut.as_str(),
        DictationMode::Style(settings.default_style),
    )];
    for style in [
        DictationStyle::Raw,
        DictationStyle::Proofread,
        DictationStyle::Structure,
        DictationStyle::Custom,
    ] {
        if let Some(key) = settings.style_shortcuts.get(&style) {
            choices.push((key.as_str(), DictationMode::Style(style)));
        }
    }
    choices.push((
        settings.prompt_shortcut.as_str(),
        DictationMode::PromptEngineer,
    ));
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for (text, mode) in choices {
        if text.trim().is_empty() {
            continue;
        }
        let (mods, key) = parse_shortcut(text)?;
        if !seen.insert((mods, key)) {
            return Err(format!("Сочетание {text} назначено дважды"));
        }
        result.push(Binding {
            id: FIRST_ID + result.len() as i32,
            mode,
            mods,
            key,
        });
    }
    if result.is_empty() {
        return Err("Назначь хотя бы одно сочетание".into());
    }
    Ok(result)
}

fn parse_shortcut(text: &str) -> Result<(u32, u32), String> {
    let parts: Vec<String> = text
        .split('+')
        .map(|part| part.trim().to_ascii_uppercase())
        .collect();
    let mut mods = NOREPEAT;
    for part in &parts[..parts.len() - 1] {
        mods |= match part.as_str() {
            "CTRL" | "CONTROL" => CTRL,
            "ALT" => ALT,
            "SHIFT" => SHIFT,
            "WIN" => WIN,
            _ => return Err(format!("Неизвестное сочетание: {text}")),
        };
    }
    let name = parts.last().unwrap();
    let key = if let Some(number) = name.strip_prefix('F') {
        match number.parse::<u32>() {
            Ok(n @ 1..=24) => 0x70 + n - 1,
            _ => return Err(format!("Неизвестная клавиша: {text}")),
        }
    } else if name == "SPACE" {
        if mods == NOREPEAT {
            return Err("Пробел без модификатора мешает обычной печати".into());
        }
        0x20
    } else if name.len() == 1 && name.as_bytes()[0].is_ascii_alphanumeric() {
        name.as_bytes()[0] as u32
    } else {
        return Err(format!("Неизвестная клавиша: {text}"));
    };
    Ok((mods, key))
}

fn register(settings: &Settings) -> io::Result<Vec<Binding>> {
    let requested = bindings(settings).map_err(io::Error::other)?;
    let mut active: Vec<Binding> = Vec::new();
    for binding in requested {
        if unsafe { RegisterHotKey(std::ptr::null_mut(), binding.id, binding.mods, binding.key) }
            == 0
        {
            let error = io::Error::last_os_error();
            unregister(&active);
            return Err(error);
        }
        active.push(binding);
    }
    Ok(active)
}

fn unregister(bindings: &[Binding]) {
    for binding in bindings {
        unsafe { UnregisterHotKey(std::ptr::null_mut(), binding.id) };
    }
}

fn reload_settings(
    settings: &mut Settings,
    active: &mut Vec<Binding>,
    alert: &slint::Weak<AssistantPanel>,
) {
    match Settings::load(&settings_ui::settings_path()) {
        Ok(next) => {
            unregister(active);
            match register(&next) {
                Ok(next_active) => {
                    *settings = next;
                    *active = next_active;
                }
                Err(error) => show_error(alert, format!("Не удалось применить сочетания: {error}")),
            }
        }
        Err(error) => show_error(alert, format!("Не удалось загрузить настройки: {error}")),
    }
}

fn run(
    pill: slint::Weak<RecordingPill>,
    alert: slint::Weak<AssistantPanel>,
    settings_panel: slint::Weak<SettingsPanel>,
) -> io::Result<()> {
    let mut settings = Settings::load(&settings_ui::settings_path()).map_err(io::Error::other)?;
    let mut active = register(&settings)?;
    if unsafe {
        RegisterHotKey(
            std::ptr::null_mut(),
            SETTINGS_ID,
            CTRL | ALT | NOREPEAT,
            b'O' as u32,
        )
    } == 0
    {
        show_error(
            &alert,
            "Не удалось назначить Ctrl+Alt+O для настроек".into(),
        );
    }
    let mut pipeline = DictationPipeline::new().map_err(io::Error::other)?;
    let mut controller = DictationController::default();
    let mut capture: Option<AudioCapture> = None;
    let mut mode: Option<DictationMode> = None;
    let mut target = 0;
    let mut esc_active = false;
    let mut pending_reload = false;
    THREAD_ID.store(unsafe { GetCurrentThreadId() }, Ordering::Release);

    loop {
        let mut message = std::mem::MaybeUninit::<Message>::uninit();
        if unsafe { GetMessageW(message.as_mut_ptr(), std::ptr::null_mut(), 0, 0) } <= 0 {
            break;
        }
        let message = unsafe { message.assume_init() };
        if message.message == WM_RELOAD {
            if capture.is_some() {
                pending_reload = true;
            } else {
                reload_settings(&mut settings, &mut active, &alert);
            }
            continue;
        }
        if message.message != WM_HOTKEY {
            continue;
        }
        if message.w_param as i32 == SETTINGS_ID {
            let panel = settings_panel.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(panel) = panel.upgrade() {
                    let _ = panel.show();
                }
            });
            continue;
        }
        let action = if message.w_param as i32 == ESC_ID && esc_active {
            controller.escape()
        } else if let Some(binding) = active
            .iter()
            .find(|binding| binding.id == message.w_param as i32)
        {
            let style = match binding.mode {
                DictationMode::Style(style) => style,
                DictationMode::PromptEngineer => DictationStyle::Raw,
            };
            let action = controller.press(DictationShortcut(binding.id as u32), style);
            if matches!(action, DictationAction::BeginCapture(_)) {
                mode = Some(binding.mode);
            }
            action
        } else {
            DictationAction::None
        };
        match action {
            DictationAction::BeginCapture(_) => {
                target = foreground_window();
                match AudioCapture::start_default() {
                    Ok(recording) => {
                        capture = Some(recording);
                        esc_active =
                            unsafe { RegisterHotKey(std::ptr::null_mut(), ESC_ID, NOREPEAT, 0x1b) }
                                != 0;
                        show_pill(&pill, "Запись", true, false);
                    }
                    Err(error) => {
                        controller.escape();
                        mode = None;
                        show_error(&alert, format!("Не удалось включить микрофон: {error}"));
                    }
                }
            }
            DictationAction::FinishCapture(_) => {
                if esc_active {
                    unsafe { UnregisterHotKey(std::ptr::null_mut(), ESC_ID) };
                    esc_active = false;
                }
                show_pill(&pill, "Обработка", false, false);
                if let Some(recording) = capture.take() {
                    let result = recording.finish().map_err(|error| error.to_string()).and_then(|samples| {
                        let key = OpenAiKeyStore::new(settings_ui::key_path()).load().map_err(|error| error.to_string())?;
                        pipeline.process(&samples, &settings, key.as_deref(), ProcessingDevice::Cpu,
                            mode.take().unwrap_or(DictationMode::Style(settings.default_style)))
                            .map_err(|error| match error {
                                PipelineError::Processing { recognized, source } => {
                                    let _ = copy_text(&recognized);
                                    format!("Обработка не удалась: {source}. Исходный текст в буфере.")
                                }
                                other => other.to_string(),
                            })
                    });
                    match result {
                        Ok(output) => match paste_text_if_foreground(&output.final_text, target) {
                            Ok(()) => show_pill(&pill, "Вставлено", false, true),
                            Err(error) => {
                                show_error(
                                    &alert,
                                    format!(
                                        "Вставка не удалась: {error}. Текст в буфере — нажми Ctrl+V."
                                    ),
                                );
                                show_pill(&pill, "Готово", false, true);
                            }
                        },
                        Err(error) => {
                            show_error(&alert, error);
                            show_pill(&pill, "Ошибка", false, true);
                        }
                    }
                }
                controller.finalization_ended();
                if pending_reload {
                    reload_settings(&mut settings, &mut active, &alert);
                    pending_reload = false;
                }
            }
            DictationAction::DiscardCapture => {
                if esc_active {
                    unsafe { UnregisterHotKey(std::ptr::null_mut(), ESC_ID) };
                    esc_active = false;
                }
                capture = None;
                mode = None;
                show_pill(&pill, "Отменено", false, true);
                if pending_reload {
                    reload_settings(&mut settings, &mut active, &alert);
                    pending_reload = false;
                }
            }
            DictationAction::None => {}
        }
    }
    THREAD_ID.store(0, Ordering::Release);
    if esc_active {
        unsafe { UnregisterHotKey(std::ptr::null_mut(), ESC_ID) };
    }
    unregister(&active);
    unsafe { UnregisterHotKey(std::ptr::null_mut(), SETTINGS_ID) };
    Ok(())
}

fn show_pill(pill: &slint::Weak<RecordingPill>, label: &str, recording: bool, auto_hide: bool) {
    let pill = pill.clone();
    let label = label.to_owned();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(panel) = pill.upgrade() {
            panel.set_status_text(label.into());
            panel.set_recording(recording);
            let _ = panel.show();
            let weak = panel.as_weak();
            slint::Timer::single_shot(Duration::from_millis(10), move || {
                if let Some(panel) = weak.upgrade() {
                    let _ = windows_focus::make_non_activating(panel.window());
                }
            });
            if auto_hide {
                let weak = panel.as_weak();
                slint::Timer::single_shot(Duration::from_secs(2), move || {
                    if let Some(panel) = weak.upgrade() {
                        let _ = panel.hide();
                    }
                });
            }
        }
    });
}

fn show_error(alert: &slint::Weak<AssistantPanel>, message: String) {
    let alert = alert.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(panel) = alert.upgrade() {
            panel.set_panel_title("Ошибка".into());
            panel.set_answer_text(message.into());
            panel.set_panel_footer(
                "Если текст сохранён в буфере, его можно вставить вручную".into(),
            );
            let _ = panel.show();
            let weak = panel.as_weak();
            slint::Timer::single_shot(Duration::from_millis(10), move || {
                if let Some(panel) = weak.upgrade() {
                    let _ = windows_focus::make_non_activating(panel.window());
                }
            });
            let weak = panel.as_weak();
            slint::Timer::single_shot(Duration::from_secs(4), move || {
                if let Some(panel) = weak.upgrade() {
                    let _ = panel.hide();
                }
            });
        }
    });
}
