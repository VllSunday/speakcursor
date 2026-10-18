//! Small Windows WAV player for assistant speech. Playback runs on a worker
//! thread so generation and the Slint event loop remain responsive.

use std::{ffi::c_void, io, ptr};

const SND_NODEFAULT: u32 = 0x0002;
const SND_MEMORY: u32 = 0x0004;

#[link(name = "winmm")]
unsafe extern "system" {
    fn PlaySoundW(sound: *const u16, module: *mut c_void, flags: u32) -> i32;
}

pub fn play_wav_blocking(wav: &[u8]) -> io::Result<()> {
    if !wav.starts_with(b"RIFF") || wav.len() < 44 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid WAV response"));
    }
    // SND_MEMORY treats this pointer as bytes; u16 storage guarantees the
    // alignment required by PlaySoundW while staying alive until it returns.
    let mut aligned = vec![0u16; wav.len().div_ceil(2)];
    unsafe { ptr::copy_nonoverlapping(wav.as_ptr(), aligned.as_mut_ptr().cast::<u8>(), wav.len()) };
    if unsafe { PlaySoundW(aligned.as_ptr(), ptr::null_mut(), SND_MEMORY | SND_NODEFAULT) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn stop_playback() {
    unsafe { PlaySoundW(ptr::null(), ptr::null_mut(), 0) };
}
