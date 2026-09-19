//! Pictures the student puts into the composer, written where the agent can
//! read them.
//!
//! A CLI agent has no channel for an image: it reads files, so an image has
//! to *be* a file before it can be talked about. So a paste or a drop lands
//! here as bytes, gets written into `agents/attachments/`, and the composer
//! puts the path it gets back into the message — the same move
//! [`crate::chapters::app::lecture_grab_frames`] already makes for the frames a
//! dock message carries, and the same move the `@` menu makes for a course
//! file. Nothing about the message format is special: it is a backticked
//! path, and the agent opens it with its own image tool.
//!
//! `agents/attachments/` and not a folder of its own, because `agents/` is
//! the one directory every bridge can both read and write (`harness/mod.rs`),
//! and a path outside it would be refused by Claude's and Codex's sandboxes
//! at the moment the agent tried to look. The path handed back is relative to
//! that folder — `./attachments/<name>` — since that is the thread's cwd.
//!
//! **The claimed filename never reaches the filesystem.** The bytes are
//! sniffed, the extension comes from what they actually are, and the stem is
//! this app's own stamp. A name from a drag payload is somebody else's
//! string; treating it as one removes path traversal, the extension lie and
//! the collision at once.

use std::path::{Path, PathBuf};

use base64::Engine;

/// The most a single picture may be. A screenshot is a megabyte or two; this
/// is loose enough never to be met by one and tight enough that a video
/// dropped by mistake is refused here rather than filling the library.
const MAX_BYTES: usize = 20 * 1024 * 1024;

/// What the bytes actually are, and the extension that follows from it.
///
/// Sniffed rather than trusted: every one of these formats is identified by
/// its first few bytes, and the agent is about to be told this file is a
/// picture.
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
    // ISO-BMFF: `ftyp` at offset 4, then the brand. macOS screenshots of a
    // photo, and anything straight off a phone, arrive as one of these.
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

/// `20260918-034512-8f3a1b7c.png` — sortable, unique, and nothing of the
/// caller's in it.
fn filename(ext: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let secs = (now / 1_000_000_000) as u64;
    // `20260918-034512`, UTC: sortable in a listing and the same order the
    // library's own logs are stamped in.
    let stamp = crate::paths::iso8601_utc(secs).replace(['-', ':'], "").replace('T', "-");
    // The sub-second part of the same clock: unique within a second without
    // reaching for a random-number crate.
    let tail = (now % 1_000_000_000) as u32;
    format!("{stamp}-{tail:08x}.{ext}")
}

/// Write one picture into `agents/attachments/`, answering with the path the
/// agent opens it by.
fn write(bytes: &[u8]) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("that file is empty".into());
    }
    if bytes.len() > MAX_BYTES {
        return Err(format!(
            "that file is {} MB — attachments are capped at {} MB",
            bytes.len() / (1024 * 1024),
            MAX_BYTES / (1024 * 1024)
        ));
    }
    let ext = sniff(bytes).ok_or("that is not an image the agent can open")?;
    let dir = attachments_dir(&crate::paths::data_dir());
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let name = filename(ext);
    std::fs::write(dir.join(&name), bytes).map_err(|e| format!("{name}: {e}"))?;
    Ok(format!("./attachments/{name}"))
}

/// A picture pasted into the composer: base64 of the clipboard's bytes.
///
/// Base64 rather than a byte array because the array would be a JSON list of
/// numbers — roughly seven characters per byte of screenshot across the IPC.
#[tauri::command]
pub async fn harness_attach_image(data: String) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| format!("could not read the pasted image: {e}"))?;
    tokio::task::spawn_blocking(move || write(&bytes))
        .await
        .map_err(|e| e.to_string())?
}

/// A picture dropped onto the composer from Finder: the OS hands the webview
/// a path, so the bytes never cross the IPC at all.
#[tauri::command]
pub async fn harness_attach_file(path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let src = PathBuf::from(&path);
        let meta = std::fs::metadata(&src).map_err(|e| format!("{path}: {e}"))?;
        if meta.len() as usize > MAX_BYTES {
            return Err(format!(
                "that file is {} MB — attachments are capped at {} MB",
                meta.len() / (1024 * 1024),
                MAX_BYTES / (1024 * 1024)
            ));
        }
        let bytes = std::fs::read(&src).map_err(|e| format!("{path}: {e}"))?;
        write(&bytes)
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

    /// The whole point of sniffing: a `.png` that is a shell script is not
    /// one, and neither is an empty file.
    #[test]
    fn refuses_what_is_not_a_picture() {
        assert_eq!(sniff(b"#!/bin/sh\nrm -rf /"), None);
        assert_eq!(sniff(b"%PDF-1.7"), None);
        assert_eq!(sniff(b""), None);
        assert!(write(b"#!/bin/sh").is_err());
    }

    /// Nothing of the caller's reaches the filesystem, and two pictures in
    /// the same second are two files.
    #[test]
    fn names_are_this_app_s_own() {
        let a = filename("png");
        let b = filename("png");
        assert_ne!(a, b);
        assert!(a.ends_with(".png"));
        assert!(!a.contains('/') && !a.contains(".."));
    }
}
