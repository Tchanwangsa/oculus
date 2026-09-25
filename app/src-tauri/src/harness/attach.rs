//! Pictures the student puts into the composer, written where the agent can
//! read them.
//!
//! A CLI agent only reads files, so a paste or drop is written into
//! `agents/attachments/` and the composer puts the returned path into the
//! message. `agents/` is the one directory every bridge's sandbox can read and
//! write, and the thread's cwd, so the path is `./attachments/<name>`. The same
//! cap, sniff and naming serve pictures pasted into notes.
//!
//! **The claimed filename never reaches the filesystem**: the extension comes
//! from the sniffed bytes and the stem is this app's stamp, which rules out
//! path traversal, the extension lie and collisions at once.

use std::path::{Path, PathBuf};

use base64::Engine;

/// The most a single picture may be; a video dropped by mistake is refused.
const MAX_BYTES: usize = 20 * 1024 * 1024;

/// The extension for what the bytes actually are, sniffed rather than trusted.
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    let starts = |sig: &[u8]| bytes.starts_with(sig);
    if starts(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    if starts(b"\xff\xd8\xff") {
        return Some("jpg");
    }
    if starts(b"GIF87a") || starts(b"GIF89a") {
        return Some("gif");
    }
    // RIFF____WEBP
    if starts(b"RIFF") && bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    // ISO-BMFF: `ftyp` at offset 4, then the brand (HEIC/AVIF, as phones send).
    if bytes.len() > 12 && &bytes[4..8] == b"ftyp" {
        let brand = &bytes[8..12];
        if brand == b"heic" || brand == b"heix" || brand == b"heim" || brand == b"heis" {
            return Some("heic");
        }
        if brand == b"mif1" || brand == b"msf1" {
            return Some("heic");
        }
        if brand == b"avif" {
            return Some("avif");
        }
    }
    None
}

/// `attachments/` inside the library's `agents/`.
fn attachments_dir(data_dir: &Path) -> PathBuf {
    crate::agents::agents_dir(data_dir).join("attachments")
}

/// `20260918-034512-8f3a1b7c.png` — sortable, unique, nothing of the caller's.
fn filename(ext: &str) -> String {
    let now = crate::clock::now_nanos();
    let secs = (now / 1_000_000_000) as u64;
    // `20260918-034512`, UTC.
    let stamp = crate::paths::iso8601_utc(secs).replace(['-', ':'], "").replace('T', "-");
    // Sub-second nanos: unique within a second without a random crate.
    let tail = (now % 1_000_000_000) as u32;
    format!("{stamp}-{tail:08x}.{ext}")
}

/// Write one picture into `dir`, returning its name. `dir` is an argument
/// because a note keeps its pictures beside itself under `courses/`
/// (`attach_document_image` in `crate::files`).
pub(crate) fn write_image(dir: &Path, bytes: &[u8]) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("that file is empty".into());
    }
    if bytes.len() > MAX_BYTES {
        return Err(format!(
            "that file is {} MB — pictures are capped at {} MB",
            bytes.len() / (1024 * 1024),
            MAX_BYTES / (1024 * 1024)
        ));
    }
    let ext = sniff(bytes).ok_or("that is not a picture — PNG, JPEG, GIF, WebP, HEIC or AVIF")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let name = filename(ext);
    std::fs::write(dir.join(&name), bytes).map_err(|e| format!("{name}: {e}"))?;
    Ok(name)
}

/// Decode a pasted picture's base64; a byte array would cross the IPC as a
/// JSON list of numbers.
pub(crate) fn decode(data: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| format!("could not read the pasted image: {e}"))
}

/// Read a picture dropped from Finder by path. The size is checked first, so a
/// video dropped by mistake is refused unread.
pub(crate) fn read_dropped(path: &str) -> Result<Vec<u8>, String> {
    let src = PathBuf::from(path);
    let meta = std::fs::metadata(&src).map_err(|e| format!("{path}: {e}"))?;
    if meta.len() as usize > MAX_BYTES {
        return Err(format!(
            "that file is {} MB — pictures are capped at {} MB",
            meta.len() / (1024 * 1024),
            MAX_BYTES / (1024 * 1024)
        ));
    }
    std::fs::read(&src).map_err(|e| format!("{path}: {e}"))
}

/// The path the agent opens the picture by, relative to the thread's cwd.
fn attachment_ref(name: &str) -> String {
    format!("./attachments/{name}")
}

/// A picture pasted into the composer, as base64 (see [`decode`]).
#[tauri::command]
pub async fn harness_attach_image(data: String) -> Result<String, String> {
    let bytes = decode(&data)?;
    tokio::task::spawn_blocking(move || {
        let dir = attachments_dir(&crate::paths::data_dir());
        write_image(&dir, &bytes).map(|name| attachment_ref(&name))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A picture dropped onto the composer from Finder, by path.
#[tauri::command]
pub async fn harness_attach_file(path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let bytes = read_dropped(&path)?;
        let dir = attachments_dir(&crate::paths::data_dir());
        write_image(&dir, &bytes).map(|name| attachment_ref(&name))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_the_formats_a_screenshot_arrives_as() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n\x00\x00"), Some("png"));
        assert_eq!(sniff(b"\xff\xd8\xff\xe0 JFIF"), Some("jpg"));
        assert_eq!(sniff(b"GIF89a....."), Some("gif"));
        assert_eq!(sniff(b"RIFF\x00\x00\x00\x00WEBPVP8 "), Some("webp"));
        assert_eq!(sniff(b"\x00\x00\x00\x18ftypheic\x00\x00"), Some("heic"));
    }

    #[test]
    fn refuses_what_is_not_a_picture() {
        assert_eq!(sniff(b"#!/bin/sh\nrm -rf /"), None);
        assert_eq!(sniff(b"%PDF-1.7"), None);
        assert_eq!(sniff(b""), None);
        let dir = crate::test_support::Scratch::new("attach");
        assert!(write_image(&dir, b"#!/bin/sh").is_err());
        assert!(write_image(&dir, b"").is_err());
    }

    #[test]
    fn names_are_this_app_s_own() {
        let a = filename("png");
        let b = filename("png");
        assert_ne!(a, b);
        assert!(a.ends_with(".png"));
        assert!(!a.contains('/') && !a.contains(".."));
    }
}
