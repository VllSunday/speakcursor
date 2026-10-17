//! Probe Windows' non-activating overlay style for passive panels.
//! The chat input will need an explicit active mode in the real application.

use std::ffi::c_void;
use std::io;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};

type Hwnd = *mut c_void;

const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_FRAMECHANGED: u32 = 0x0020;

#[link(name = "user32")]
unsafe extern "system" {
    fn GetForegroundWindow() -> Hwnd;
    fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
    fn SetWindowPos(
        hwnd: Hwnd,
        after: Hwnd,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
}

pub fn foreground_window() -> isize {
    // SAFETY: This reads the currently focused HWND without modifying it.
    unsafe { GetForegroundWindow() as isize }
}

pub fn report_focus(phase: &str, expected: isize) {
    if expected == 0 {
        eprintln!("focus check {phase}: unavailable (no foreground window)");
    } else if foreground_window() == expected {
        eprintln!("focus check {phase}: PASS — original application still focused");
    } else {
        eprintln!("focus check {phase}: FAIL — foreground window changed");
    }
}

pub fn make_non_activating(window: &slint::Window) -> Result<(), Box<dyn std::error::Error>> {
    let handle = window.window_handle();
    let native = handle.window_handle()?.as_raw();
    let RawWindowHandle::Win32(native) = native else {
        return Err(io::Error::other("Slint returned a non-Windows window handle").into());
    };
    let hwnd = native.hwnd.get() as Hwnd;

    // SAFETY: `hwnd` comes from this live Slint window, on its UI thread. The
    // flags only change extended window styles and preserve its existing bits.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            (style | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) as isize,
        );
        if SetWindowPos(
            hwnd,
            -1_isize as Hwnd,
            0,
            0,
            0,
            0,
            SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        ) == 0
        {
            return Err(io::Error::last_os_error().into());
        }
    }
    Ok(())
}
