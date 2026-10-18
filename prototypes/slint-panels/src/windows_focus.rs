//! Probe Windows' non-activating overlay style for passive panels.
//! The chat input will need an explicit active mode in the real application.

use std::ffi::c_void;
use std::io;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};

type Hwnd = *mut c_void;

const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
const WS_EX_TRANSPARENT: u32 = 0x0000_0020;
const WS_EX_LAYERED: u32 = 0x0008_0000;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_FRAMECHANGED: u32 = 0x0020;
const MONITOR_DEFAULTTONEAREST: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct MonitorInfo {
    size: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
}

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
    fn MonitorFromWindow(hwnd: Hwnd, flags: u32) -> Hwnd;
    fn GetMonitorInfoW(monitor: Hwnd, info: *mut MonitorInfo) -> i32;
}

pub fn place_near_screen_edge(
    window: &slint::Window,
    top: bool,
    target_window: usize,
) -> io::Result<()> {
    let foreground = if target_window != 0 {
        target_window as Hwnd
    } else {
        unsafe { GetForegroundWindow() }
    };
    let monitor = unsafe { MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_null() {
        return Err(io::Error::last_os_error());
    }
    let mut info = MonitorInfo {
        size: std::mem::size_of::<MonitorInfo>() as u32,
        monitor: Rect::default(),
        work: Rect::default(),
        flags: 0,
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let size = window.size();
    let x = info.work.left + (info.work.right - info.work.left - size.width as i32) / 2;
    let y = if top {
        info.work.top + 10
    } else {
        info.work.bottom - size.height as i32 - 10
    };
    window.set_position(slint::PhysicalPosition::new(x, y));
    Ok(())
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

pub fn make_click_through(window: &slint::Window) -> Result<(), Box<dyn std::error::Error>> {
    make_non_activating(window)?;
    let handle = window.window_handle();
    let RawWindowHandle::Win32(native) = handle.window_handle()?.as_raw() else {
        return Err(io::Error::other("Slint returned a non-Windows window handle").into());
    };
    let hwnd = native.hwnd.get() as Hwnd;
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            (style | WS_EX_TRANSPARENT | WS_EX_LAYERED) as isize,
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
