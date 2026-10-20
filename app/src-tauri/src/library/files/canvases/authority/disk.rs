//! The authority's file access, through the same scoped path guard and
//! atomic write as `write_canvas`. The content hash is SHA-256 of the bytes.

use std::path::Path;

pub type Hash = [u8; 32];

pub fn hash(bytes: &[u8]) -> Hash {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(digest.as_ref());
    out
}

/// The canvas's bytes, or `None` when the file is gone.
pub fn read(root: &Path, rel: &str) -> Result<Option<Vec<u8>>, String> {
    let path = super::super::checked(root, rel, false)?;
    match std::fs::read(&path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Replace an existing canvas file; never creates one.
pub fn write(root: &Path, rel: &str, text: &str) -> Result<u64, String> {
    super::super::write_at(root, rel, text)
}

/// Remove the canvas file; one that is already gone counts as removed.
pub fn remove(root: &Path, rel: &str) -> Result<(), String> {
    match std::fs::remove_file(super::super::checked(root, rel, false)?) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// A random epoch id: 128 bits as hex.
pub fn epoch() -> String {
    use ring::rand::{SecureRandom, SystemRandom};
    let mut bytes = [0u8; 16];
    if SystemRandom::new().fill(&mut bytes).is_err() {
        // The clock still separates epochs within one process.
        bytes = crate::runtime::clock::now_nanos().to_le_bytes();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
