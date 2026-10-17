use std::{io, ptr};

use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW,
    RegSetValueExW,
};

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "SpeakCursor";

pub fn set_launch_at_login(enabled: bool) -> io::Result<()> {
    let command = if enabled {
        let exe = std::env::current_exe()?;
        Some(wide(&format!("\"{}\"", exe.display())))
    } else {
        None
    };
    let key_name = wide(RUN_KEY);
    let value_name = wide(VALUE_NAME);
    let mut key: HKEY = ptr::null_mut();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            key_name.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut key,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let status = if let Some(command) = command {
        unsafe {
            RegSetValueExW(
                key,
                value_name.as_ptr(),
                0,
                REG_SZ,
                command.as_ptr().cast(),
                (command.len() * 2) as u32,
            )
        }
    } else {
        unsafe { RegDeleteValueW(key, value_name.as_ptr()) }
    };
    unsafe { RegCloseKey(key) };
    if status == 0 || (!enabled && status == 2) {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status as i32))
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
