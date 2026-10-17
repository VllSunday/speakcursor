//! Encrypt the single OpenAI key with Windows DPAPI for the current user.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::ptr;

use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};

pub struct OpenAiKeyStore {
    path: PathBuf,
}

impl OpenAiKeyStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn save(&self, key: &str) -> io::Result<()> {
        if key.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "API key is empty",
            ));
        }
        let mut input = CRYPT_INTEGER_BLOB {
            cbData: u32::try_from(key.len())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "API key is too long"))?,
            pbData: key.as_ptr() as *mut u8,
        };
        let mut encrypted = CRYPT_INTEGER_BLOB::default();
        // SAFETY: input points to the live key bytes for the call; DPAPI
        // allocates `encrypted.pbData`, which is copied and released below.
        if unsafe {
            CryptProtectData(
                &mut input,
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut encrypted,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let bytes = unsafe {
            std::slice::from_raw_parts(encrypted.pbData, encrypted.cbData as usize).to_vec()
        };
        unsafe { LocalFree(encrypted.pbData as _) };
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, bytes)
    }

    pub fn load(&self) -> io::Result<Option<String>> {
        let mut bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let input = CRYPT_INTEGER_BLOB {
            cbData: u32::try_from(bytes.len())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "key file is too large"))?,
            pbData: bytes.as_mut_ptr(),
        };
        let mut decrypted = CRYPT_INTEGER_BLOB::default();
        // SAFETY: `input` points to the live file buffer; DPAPI allocates
        // `decrypted.pbData`, which is copied and released below.
        if unsafe {
            CryptUnprotectData(
                &input,
                ptr::null_mut(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut decrypted,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let plaintext = unsafe {
            std::slice::from_raw_parts(decrypted.pbData, decrypted.cbData as usize).to_vec()
        };
        unsafe { LocalFree(decrypted.pbData as _) };
        String::from_utf8(plaintext)
            .map(Some)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "stored API key is not UTF-8"))
    }

    pub fn delete(&self) -> io::Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_encrypted_at_rest_and_can_be_removed() {
        let path = std::env::temp_dir().join(format!(
            "speakcursor-dpapi-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = OpenAiKeyStore::new(path.clone());
        store.save("test-key-not-a-real-credential").unwrap();
        assert!(
            !fs::read(&path)
                .unwrap()
                .windows(10)
                .any(|w| w == b"test-key-")
        );
        assert_eq!(
            store.load().unwrap().as_deref(),
            Some("test-key-not-a-real-credential")
        );
        store.delete().unwrap();
        assert!(store.load().unwrap().is_none());
    }
}
