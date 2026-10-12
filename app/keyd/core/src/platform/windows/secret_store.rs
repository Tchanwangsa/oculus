//! DPAPI user-scope protects the random vault master key on disk.
use crate::vault::{KeyError, KeySource, MasterKey};
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::*;

pub(crate) struct Dpapi;
impl KeySource for Dpapi {
    fn get_or_create(&self) -> Result<MasterKey, KeyError> {
        let load = || -> Result<MasterKey, String> {
            let directory = crate::paths::data_dir();
            std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
            let path = directory.join("master-key.dpapi");
            let _lock = super::files::lock(&directory.join("master-key.dpapi.lock"))
                .map_err(|e| e.to_string())?;
            match std::fs::read(&path) {
                Ok(sealed) => {
                    if !super::files::is_owner_only(&path).map_err(|e| e.to_string())? {
                        return Err("DPAPI master key file is not owner-only".into());
                    }
                    let mut plain = protect(&sealed, false)?;
                    let bytes: [u8; 32] = plain
                        .as_slice()
                        .try_into()
                        .map_err(|_| "DPAPI master key has the wrong length")?;
                    plain.fill(0);
                    Ok(MasterKey::from_bytes(bytes))
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    let mut bytes = [0u8; 32];
                    getrandom::getrandom(&mut bytes).map_err(|e| e.to_string())?;
                    let sealed = protect(&bytes, true)?;
                    super::files::replace_file(&path, |tmp| {
                        super::files::write_private(tmp, &sealed)
                    })?;
                    Ok(MasterKey::from_bytes(bytes))
                }
                Err(e) => Err(e.to_string()),
            }
        };
        load().map_err(KeyError::Platform)
    }
}
fn protect(bytes: &[u8], seal: bool) -> Result<Vec<u8>, String> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes
            .len()
            .try_into()
            .map_err(|_| "DPAPI input too large")?,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let mut output: CRYPT_INTEGER_BLOB = unsafe { std::mem::zeroed() };
    let ok = unsafe {
        if seal {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let value =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    if !seal {
        unsafe {
            std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize).fill(0);
        }
    }
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dpapi_roundtrips_without_putting_the_key_in_the_blob() {
        let key = [0x5au8; 32];
        let sealed = protect(&key, true).unwrap();
        assert!(!sealed.windows(key.len()).any(|window| window == key));
        assert_eq!(protect(&sealed, false).unwrap(), key);
        let mut altered = sealed;
        altered[0] ^= 1;
        assert!(protect(&altered, false).is_err());
    }
}

pub(crate) struct NoLegacy;
impl crate::vault::LegacySource for NoLegacy {
    fn read(&self, _service: &str, _account: &str) -> Result<Option<String>, KeyError> {
        Ok(None)
    }
    fn remove(&self, _service: &str, _account: &str) -> Result<bool, KeyError> {
        Ok(false)
    }
    // WebView2 keeps no WebKit keychain item to import.
    fn read_webcrypto(&self, _key: &crate::names::WebCrypto) -> Result<Option<Vec<u8>>, KeyError> {
        Ok(None)
    }
}
