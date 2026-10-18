//! Clipboard-first insertion. If synthetic paste fails, the text remains
//! available for the user to paste manually.

use std::{ffi::c_void, io};
use std::mem;
use std::ptr;
use std::thread;
use std::time::Duration;

use windows_sys::Win32::Foundation::GlobalFree;
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber, OpenClipboard,
    SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
use windows_sys::Win32::System::Ole::{CF_UNICODETEXT, OleGetClipboard, OleInitialize, OleSetClipboard, OleUninitialize};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VK_C, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_V,
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

/// Copy the current selection without permanently replacing the user's
/// clipboard. OLE keeps the original clipboard data object, including rich
/// formats, while Ctrl+C obtains text from the focused application.
pub fn selected_text(expected_window: usize) -> io::Result<Option<String>> {
    if expected_window == 0 || foreground_window() != expected_window {
        return Ok(None);
    }
    let initialized = unsafe { OleInitialize(ptr::null()) };
    if initialized < 0 { return Err(io::Error::other("could not initialize OLE clipboard")); }
    let mut original: *mut c_void = ptr::null_mut();
    let saved = unsafe { OleGetClipboard(&mut original) };
    if saved < 0 || original.is_null() {
        unsafe { OleUninitialize() };
        return Err(io::Error::other("could not preserve the clipboard"));
    }
    let result = copy_selection_and_read(expected_window);
    let restored = unsafe { OleSetClipboard(original) };
    unsafe { release_unknown(original); OleUninitialize(); }
    if restored < 0 { return Err(io::Error::other("could not restore the clipboard after reading selection")); }
    result
}

unsafe fn release_unknown(object: *mut c_void) {
    // IUnknown::Release is the third pointer in every COM interface vtable.
    let vtable = unsafe { *(object.cast::<*const *const c_void>()) };
    let release: unsafe extern "system" fn(*mut c_void) -> u32 =
        unsafe { std::mem::transmute(*vtable.add(2)) };
    unsafe { release(object) };
}

fn copy_selection_and_read(expected_window: usize) -> io::Result<Option<String>> {
    let modifiers = [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN];
    for _ in 0..100 {
        if modifiers.iter().all(|key| unsafe { GetAsyncKeyState(*key as i32) } & i16::MIN == 0) {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    if modifiers.iter().any(|key| unsafe { GetAsyncKeyState(*key as i32) } & i16::MIN != 0)
        || foreground_window() != expected_window
    {
        return Ok(None);
    }
    let sequence = unsafe { GetClipboardSequenceNumber() };
    let key = |vk, flags| INPUT { r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, dwFlags: flags, ..KEYBDINPUT::default() } } };
    let events = [key(VK_CONTROL, 0), key(VK_C, 0), key(VK_C, KEYEVENTF_KEYUP), key(VK_CONTROL, KEYEVENTF_KEYUP)];
    let sent = unsafe { SendInput(events.len() as u32, events.as_ptr(), mem::size_of::<INPUT>() as i32) };
    if sent != events.len() as u32 { return Err(io::Error::last_os_error()); }
    for _ in 0..30 {
        if unsafe { GetClipboardSequenceNumber() } != sequence {
            return clipboard_text();
        }
        thread::sleep(Duration::from_millis(20));
    }
    Ok(None)
}

fn clipboard_text() -> io::Result<Option<String>> {
    for _ in 0..5 {
        if unsafe { OpenClipboard(ptr::null_mut()) } != 0 {
            let memory = unsafe { GetClipboardData(CF_UNICODETEXT as u32) };
            let text = if memory.is_null() { None } else {
                let pointer = unsafe { GlobalLock(memory) };
                if pointer.is_null() { None } else {
                    let len = unsafe { GlobalSize(memory) } / mem::size_of::<u16>();
                    let wide = unsafe { std::slice::from_raw_parts(pointer.cast::<u16>(), len) };
                    let end = wide.iter().position(|ch| *ch == 0).unwrap_or(len);
                    let text = String::from_utf16_lossy(&wide[..end]);
                    unsafe { GlobalUnlock(memory) };
                    Some(text)
                }
            };
            unsafe { CloseClipboard() };
            return Ok(text.filter(|text| !text.trim().is_empty()));
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err(io::Error::last_os_error())
}

/// Copy first, then paste only if the target window is still active. On any
/// paste failure the caller can tell the user that Ctrl+V will work manually.
pub fn paste_text_if_foreground(text: &str, expected_window: usize) -> io::Result<()> {
    copy_text(text)?;
    // A hotkey may finish quickly while Ctrl or Alt is still physically held.
    // Sending Ctrl+V then could become another shortcut in the target app.
    let modifiers = [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN];
    let mut released = false;
    for _ in 0..100 {
        if modifiers
            .iter()
            .all(|key| unsafe { GetAsyncKeyState(*key as i32) } & i16::MIN == 0)
        {
            released = true;
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    if !released {
        return Err(io::Error::other("hotkey modifiers are still pressed"));
    }
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
