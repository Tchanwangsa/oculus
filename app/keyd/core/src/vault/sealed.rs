use std::collections::BTreeMap;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

use super::{Entries, MasterKey, VaultError};

pub(super) const VERSION: u8 = 1;
pub(super) const NONCE_LEN: usize = 12;
pub(super) const TAG_LEN: usize = 16;

pub(super) fn cipher(key: &MasterKey) -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new(Key::from_slice(&key.0))
}

pub(super) fn seal(key: &MasterKey, entries: &Entries) -> Result<Vec<u8>, VaultError> {
    let plain =
        serde_json::to_vec(&entries.0).map_err(|e| VaultError::Io(format!("encoding: {e}")))?;
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce).map_err(|e| VaultError::Io(format!("getrandom: {e}")))?;
    let sealed = cipher(key)
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &plain,
                aad: &[VERSION],
            },
        )
        .map_err(|_| VaultError::Io("encryption failed".to_string()))?;
    let mut out = Vec::with_capacity(1 + NONCE_LEN + sealed.len());
    out.push(VERSION);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

pub(super) fn open_sealed(key: &MasterKey, bytes: &[u8]) -> Result<Entries, VaultError> {
    if bytes.len() < 1 + NONCE_LEN + TAG_LEN {
        return Err(VaultError::Damaged(format!(
            "{} bytes is shorter than any vault",
            bytes.len()
        )));
    }
    if bytes[0] != VERSION {
        return Err(VaultError::Damaged(format!(
            "unknown format version {}",
            bytes[0]
        )));
    }
    let (nonce, sealed) = bytes[1..].split_at(NONCE_LEN);
    let plain = cipher(key)
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: sealed,
                aad: &[VERSION],
            },
        )
        .map_err(|_| VaultError::Undecryptable)?;
    serde_json::from_slice::<BTreeMap<String, String>>(&plain)
        .map(Entries)
        .map_err(|_| VaultError::Damaged("the decrypted body is not a map of strings".to_string()))
}
