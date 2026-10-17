//! Clipboard-first insertion. If synthetic paste fails, the text remains
//! available for the user to paste manually.

use std::io;
use std::mem;
use std::ptr;
use std::thread;
use std::time::Duration;

use windows_sys::Win32::Foundation::GlobalFree;
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_CONTROL, VK_V,
};
use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

/// Opaque identity for checking that dictation still targets the same window.
pub fn foreground_window() -> usize {
    unsafe { GetForegroundWindow() as usize }
}

pub fn copy_text(text: &str) -> io::Result<()> {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    let size = wide.len() * mem::size_of::<u16>();
    let memory = unsafe { GlobalAlloc(GMEM_MOVEABLE, size) };
    if memory.is_null() {
        return Err(io::Error::last_os_error());
    }
    let destination = unsafe { GlobalLock(memory) };
    if destination.is_null() {
        unsafe { GlobalFree(memory) };
        return Err(io::Error::last_os_error());
    }
    unsafe { ptr::copy_nonoverlapping(wide.as_ptr(), destination.cast::<u16>(), wide.len()) };
    unsafe { GlobalUnlock(memory) };

    let mut opened = false;
    for _ in 0..5 {
        if unsafe { OpenClipboard(ptr::null_mut()) } != 0 {
            opened = true;
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    if !opened {
        unsafe { GlobalFree(memory) };
        return Err(io::Error::last_os_error());
    }
    let result = if unsafe { EmptyClipboard() } == 0 {
        Err(io::Error::last_os_error())
    } else if unsafe { SetClipboardData(CF_UNICODETEXT as u32, memory) }.is_null() {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    };
    unsafe { CloseClipboard() };
    if result.is_err() {
        unsafe { GlobalFree(memory) };
    }
    result
}

/// Copy first, then paste only if the target window is still active. On any
/// paste failure the caller can tell the user that Ctrl+V will work manually.
pub fn paste_text_if_foreground(text: &str, expected_window: usize) -> io::Result<()> {
    copy_text(text)?;
    if expected_window == 0 || foreground_window() != expected_window {
        return Err(io::Error::other("original text field is no longer active"));
    }
    let key = |vk, flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                dwFlags: flags,
                ..KEYBDINPUT::default()
            },
        },
    };
    let events = [
        key(VK_CONTROL, 0),
        key(VK_V, 0),
        key(VK_V, KEYEVENTF_KEYUP),
        key(VK_CONTROL, KEYEVENTF_KEYUP),
    ];
    let sent = unsafe {
        SendInput(
            events.len() as u32,
            events.as_ptr(),
            mem::size_of::<INPUT>() as i32,
        )
    };
    if sent != events.len() as u32 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
