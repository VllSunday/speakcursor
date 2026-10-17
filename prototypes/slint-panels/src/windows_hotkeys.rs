//! Temporary Windows hotkeys for the state-machine preview. No audio is recorded.

use std::ffi::c_void;
use std::io;

use speakcursor_core::{DictationAction, DictationController, DictationShortcut, DictationStyle};

use crate::RecordingPill;

const ID_DICTATION: i32 = 1;
const ID_CANCEL: i32 = 2;
const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const MOD_NOREPEAT: u32 = 0x4000;
const VK_F8: u32 = 0x77;
const VK_ESCAPE: u32 = 0x1b;
const WM_HOTKEY: u32 = 0x0312;

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
    fn RegisterHotKey(hwnd: *mut c_void, id: i32, modifiers: u32, key: u32) -> i32;
    fn UnregisterHotKey(hwnd: *mut c_void, id: i32) -> i32;
    fn GetMessageW(message: *mut Message, hwnd: *mut c_void, min: u32, max: u32) -> i32;
}

pub fn start(pill: slint::Weak<RecordingPill>) {
    std::thread::spawn(move || {
        if let Err(error) = run(pill) {
            eprintln!("dictation preview hotkey unavailable: {error}");
        }
    });
}

fn run(pill: slint::Weak<RecordingPill>) -> io::Result<()> {
    // A null HWND attaches the hotkey to this thread's message queue. Windows
    // removes the registration when this preview process exits.
    if unsafe {
        RegisterHotKey(
            std::ptr::null_mut(),
            ID_DICTATION,
            MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
            VK_F8,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    eprintln!(
        "Dictation state preview: Ctrl+Alt+F8 starts/finishes; Esc cancels while active. No audio is captured."
    );

    let mut controller = DictationController::default();
    let mut cancel_registered = false;
    loop {
        let mut message = std::mem::MaybeUninit::<Message>::uninit();
        let result = unsafe { GetMessageW(message.as_mut_ptr(), std::ptr::null_mut(), 0, 0) };
        if result <= 0 {
            break;
        }
        let message = unsafe { message.assume_init() };
        if message.message != WM_HOTKEY {
            continue;
        }
        let action = match message.w_param as i32 {
            ID_DICTATION => controller.press(DictationShortcut(1), DictationStyle::Raw),
            ID_CANCEL if cancel_registered => controller.escape(),
            _ => DictationAction::None,
        };
        let (label, recording) = match action {
            DictationAction::BeginCapture(_) => {
                cancel_registered = unsafe {
                    RegisterHotKey(std::ptr::null_mut(), ID_CANCEL, MOD_NOREPEAT, VK_ESCAPE)
                } != 0;
                if !cancel_registered {
                    eprintln!(
                        "Esc cancellation hotkey unavailable: {}",
                        io::Error::last_os_error()
                    );
                }
                ("Тест сеанса", true)
            }
            DictationAction::FinishCapture(_) => {
                unregister_cancel(&mut cancel_registered);
                controller.finalization_ended();
                ("Завершено", false)
            }
            DictationAction::DiscardCapture => {
                unregister_cancel(&mut cancel_registered);
                ("Отменено", false)
            }
            DictationAction::None => continue,
        };
        let pill = pill.clone();
        if slint::invoke_from_event_loop(move || {
            if let Some(pill) = pill.upgrade() {
                pill.set_recording(recording);
                pill.set_status_text(label.into());
            }
        })
        .is_err()
        {
            break;
        }
    }
    unregister_cancel(&mut cancel_registered);
    unsafe { UnregisterHotKey(std::ptr::null_mut(), ID_DICTATION) };
    Ok(())
}

fn unregister_cancel(registered: &mut bool) {
    if *registered {
        unsafe { UnregisterHotKey(std::ptr::null_mut(), ID_CANCEL) };
        *registered = false;
    }
}
