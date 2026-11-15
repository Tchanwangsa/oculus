//! Files the student puts into the composer, written where the agent can
//! read them: pictures, PDFs and text files.
//!
//! A CLI agent only reads files, so a paste or drop is written into
//! `agents/attachments/` and the composer puts the returned path into the
//! message. `agents/` is the one directory every bridge's sandbox can read and
//! write, and the thread's cwd, so the path is `./attachments/<name>`. The same
//! cap, sniff and naming serve pictures pasted into notes, which take
//! pictures only ([`write_image`]).
//!
//! **The claimed filename never reaches the filesystem**: the extension comes
//! from the sniffed bytes (a text file may keep a claimed extension from a
//! short list) and the stem is this app's stamp, which rules out path
//! traversal, the extension lie and collisions at once. Nothing here reads or
//! writes outside `agents/attachments/`, which must itself be a real folder
//! there, not a link elsewhere.

use std::path::{Path, PathBuf};

use base64::Engine;

/// The most a single file may be; a video dropped by mistake is refused.
const MAX_BYTES: usize = 20 * 1024 * 1024;

/// What the composer says for anything that is not a picture, PDF or text.
const NOT_ATTACHABLE: &str =
    "Only pictures, PDFs and text files can be attached — use @ for a course file.";

/// Extensions a text file keeps, since an agent picks its reader by them; any
/// other text is written as `.txt`.
const TEXT_EXTENSIONS: &[&str] = &[
    "md", "txt", "csv", "tsv", "json", "py", "ipynb", "r", "m", "jl", "js", "ts", "jsx", "tsx",
    "java", "c", "h", "cpp", "hpp", "cs", "rs", "go", "rb", "sh", "sql", "tex", "bib", "html",
    "css", "xml", "yaml", "yml", "toml", "log",
];

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

/// UTF-8 with no control byte but tab, line breaks, form feed and escape (a
/// coloured log): a NUL or a zip's `PK\x03\x04` is binary.
fn is_text(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok()
        && !bytes
            .iter()
            .any(|&b| b < 0x20 && !matches!(b, b'\t' | b'\n' | b'\r' | 0x0c | 0x1b))
}

/// The extension for a picture, PDF or text file, sniffed; a text file keeps
/// `claimed`'s extension when it is on [`TEXT_EXTENSIONS`].
fn sniff_file(bytes: &[u8], claimed: Option<&str>) -> Option<&'static str> {
    if let Some(ext) = sniff(bytes) {
        return Some(ext);
    }
    if bytes.starts_with(b"%PDF-") {
        return Some("pdf");
    }
    if !is_text(bytes) {
        return None;
    }
    let ext = claimed
        .and_then(|name| Path::new(name).extension())
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase);
    Some(
        ext.and_then(|ext| TEXT_EXTENSIONS.iter().copied().find(|t| *t == ext))
            .unwrap_or("txt"),
    )
}

/// `attachments/` inside the library's `agents/`, without touching disk.
fn attachments_dir(data_dir: &Path) -> PathBuf {
    crate::agents::agents_dir(data_dir).join("attachments")
}

/// The real attachments folder: it must be `agents/attachments` itself once
/// links are resolved, so a link planted there cannot send a write elsewhere.
fn verified_dir(data_dir: &Path) -> Result<PathBuf, String> {
    let agents = crate::agents::agents_dir(data_dir)
        .canonicalize()
        .map_err(|e| format!("agents: {e}"))?;
    let dir = attachments_dir(data_dir)
        .canonicalize()
        .map_err(|e| format!("attachments: {e}"))?;
    if dir != agents.join("attachments") {
        return Err("the attachments folder is not inside agents/".into());
    }
    Ok(dir)
}

/// [`verified_dir`], created first if missing.
fn attachments_root(data_dir: &Path) -> Result<PathBuf, String> {
    let dir = attachments_dir(data_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    verified_dir(data_dir)
}

/// The file an `agents/attachments/<name>` path names, inside `root` (a
/// [`verified_dir`]). Refused: an absolute path, any other folder, a `..` or
/// empty name, and a link that resolves out of `root`.
fn resolve_attachment(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let name = rel
        .strip_prefix("agents/attachments/")
        .ok_or_else(|| format!("{rel}: not an attachment path"))?;
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
        return Err(format!("{rel}: not an attachment path"));
    }
    let path = root.join(name);
    if let Ok(real) = path.canonicalize() {
        if !real.starts_with(root) {
            return Err(format!("{rel}: resolves outside attachments"));
        }
    }
    Ok(path)
}

/// Whether each attachment is still on disk, for a chip that must say so when
/// it is not. A path [`resolve_attachment`] refuses counts as gone.
fn attachments_exist(data_dir: &Path, paths: &[String]) -> Vec<bool> {
    let Ok(root) = verified_dir(data_dir) else {
        return vec![false; paths.len()];
    };
    paths
        .iter()
        .map(|p| resolve_attachment(&root, p).is_ok_and(|f| f.is_file()))
        .collect()
}

/// `20260918-034512-8f3a1b7c.png` — sortable, unique, nothing of the caller's.
fn filename(ext: &str) -> String {
    let now = crate::runtime::clock::now_nanos();
    let secs = (now / 1_000_000_000) as u64;
    // `20260918-034512`, UTC.
    let stamp = crate::library::paths::iso8601_utc(secs)
        .replace(['-', ':'], "")
        .replace('T', "-");
    // Sub-second nanos: unique within a second without a random crate.
    let tail = (now % 1_000_000_000) as u32;
    format!("{stamp}-{tail:08x}.{ext}")
}

/// "that file is 31 MB — the most is 20 MB".
fn too_big(len: u64) -> String {
    format!(
        "that file is {} MB — the most is {} MB",
        len / (1024 * 1024),
        MAX_BYTES / (1024 * 1024)
    )
}

/// The size checks every write makes before it sniffs.
fn check_size(bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() {
        return Err("that file is empty".into());
    }
    if bytes.len() > MAX_BYTES {
        return Err(too_big(bytes.len() as u64));
    }
    Ok(())
}

/// `bytes` into `dir` under a fresh stamped name ending `.ext`.
fn write_stamped(dir: &Path, bytes: &[u8], ext: &str) -> Result<String, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let name = filename(ext);
    std::fs::write(dir.join(&name), bytes).map_err(|e| format!("{name}: {e}"))?;
    Ok(name)
}

/// Write one picture into `dir`, returning its name. `dir` is an argument
/// because a note keeps its pictures beside itself under `courses/`
/// (`attach_document_image` in `crate::library::files`); a note takes
/// pictures only.
pub(crate) fn write_image(dir: &Path, bytes: &[u8]) -> Result<String, String> {
    check_size(bytes)?;
    let ext = sniff(bytes).ok_or("that is not a picture — PNG, JPEG, GIF, WebP, HEIC or AVIF")?;
    write_stamped(dir, bytes, ext)
}

/// Write one chat attachment into `dir`: a picture, a PDF, or UTF-8 text
/// ([`is_text`]). `claimed` is the name the student knows it by, read only for a
/// text file's extension.
pub(crate) fn write_file(
    dir: &Path,
    bytes: &[u8],
    claimed: Option<&str>,
) -> Result<String, String> {
    check_size(bytes)?;
    let ext = sniff_file(bytes, claimed).ok_or(NOT_ATTACHABLE)?;
    write_stamped(dir, bytes, ext)
}

/// Decode a pasted picture's base64; a byte array would cross the IPC as a
/// JSON list of numbers.
pub(crate) fn decode(data: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| format!("could not read the pasted image: {e}"))
}

/// Read a file dropped from Finder by path. The size is checked first, so a
/// video dropped by mistake is refused unread, and so is a folder.
pub(crate) fn read_dropped(path: &str) -> Result<Vec<u8>, String> {
    let src = PathBuf::from(path);
    let meta = std::fs::metadata(&src).map_err(|e| format!("{path}: {e}"))?;
    if meta.is_dir() {
        return Err("that is a folder, not a file".into());
    }
    if meta.len() as usize > MAX_BYTES {
        return Err(too_big(meta.len()));
    }
    std::fs::read(&src).map_err(|e| format!("{path}: {e}"))
}

/// The path the agent opens the picture by, relative to the thread's cwd.
fn attachment_ref(name: &str) -> String {
    format!("./attachments/{name}")
}

/// A file pasted into the composer, as base64 (see [`decode`]); `name` is the
/// clipboard's name for it.
#[tauri::command]
pub async fn harness_attach_image(data: String, name: Option<String>) -> Result<String, String> {
    let bytes = decode(&data)?;
    crate::runtime::blocking::run(move || {
        let dir = attachments_root(&crate::library::paths::data_dir())?;
        write_file(&dir, &bytes, name.as_deref()).map(|name| attachment_ref(&name))
    })
    .await
}

/// A file dropped onto the composer from Finder, by path.
#[tauri::command]
pub async fn harness_attach_file(path: String) -> Result<String, String> {
    crate::runtime::blocking::run(move || {
        let bytes = read_dropped(&path)?;
        let dir = attachments_root(&crate::library::paths::data_dir())?;
        write_file(&dir, &bytes, Some(&path)).map(|name| attachment_ref(&name))
    })
    .await
}

/// Whether each `agents/attachments/<name>` path is still a file, in order.
#[tauri::command]
pub async fn harness_attachments_exist(paths: Vec<String>) -> Result<Vec<bool>, String> {
    crate::runtime::blocking::run(move || {
        Ok(attachments_exist(
            &crate::library::paths::data_dir(),
            &paths,
        ))
    })
    .await
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
    fn a_pdf_or_text_is_a_file_but_not_a_picture() {
        assert_eq!(sniff_file(b"%PDF-1.7\n%\xe2\xe3", None), Some("pdf"));
        assert_eq!(sniff(b"%PDF-1.7"), None);
        let dir = crate::test_support::Scratch::new("attach-pdf");
        assert!(write_image(&dir, b"%PDF-1.7\n").is_err());
        let name = write_file(&dir, b"%PDF-1.7\n", Some("notes.png")).unwrap();
        assert!(name.ends_with(".pdf"));
        assert_eq!(std::fs::read(dir.join(&name)).unwrap(), b"%PDF-1.7\n");
    }

    #[test]
    fn text_keeps_a_known_extension_and_is_otherwise_txt() {
        assert_eq!(sniff_file(b"x,y\n1,2\n", Some("data.CSV")), Some("csv"));
        assert_eq!(
            sniff_file(b"print(1)\n", Some("/Users/me/hw/q1.py")),
            Some("py")
        );
        assert_eq!(
            sniff_file("# Notes — ünïcode\n".as_bytes(), Some("a.md")),
            Some("md")
        );
        assert_eq!(sniff_file(b"plain", Some("weird.exe")), Some("txt"));
        assert_eq!(sniff_file(b"plain", Some("no-extension")), Some("txt"));
        assert_eq!(sniff_file(b"plain", None), Some("txt"));
        assert_eq!(sniff_file(b"a\tb\r\n\x1b[31mred\x0c", None), Some("txt"));
        // The claimed name never reaches disk, only a listed extension does.
        let dir = crate::test_support::Scratch::new("attach-text");
        let name = write_file(&dir, b"a,b\n", Some("../../escape.csv")).unwrap();
        assert!(name.ends_with(".csv") && !name.contains("escape"));
        assert!(dir.join(&name).is_file());
    }

    #[test]
    fn binary_is_refused_with_the_composer_s_wording() {
        let dir = crate::test_support::Scratch::new("attach-binary");
        assert_eq!(sniff_file(b"text\0with a nul", Some("a.txt")), None);
        assert_eq!(sniff_file(b"\xff\xfe\x00\x01", None), None);
        assert_eq!(sniff_file(b"PK\x03\x04zip", Some("a.docx")), None);
        assert_eq!(
            write_file(&dir, b"\x7fELF\x02\x01\x00", Some("a.txt")).unwrap_err(),
            NOT_ATTACHABLE
        );
        assert!(write_file(&dir, b"", Some("a.txt")).is_err());
    }

    #[test]
    fn twenty_megabytes_is_the_cap() {
        let dir = crate::test_support::Scratch::new("attach-cap");
        let over = vec![b'a'; MAX_BYTES + 1];
        let err = write_file(&dir, &over, Some("a.txt")).unwrap_err();
        assert!(err.contains("20 MB"), "{err}");
        let at = vec![b'a'; MAX_BYTES];
        assert!(write_file(&dir, &at, Some("a.txt"))
            .unwrap()
            .ends_with(".txt"));
        let path = dir.join("big.txt");
        std::fs::write(&path, &over).unwrap();
        assert!(read_dropped(path.to_str().unwrap())
            .unwrap_err()
            .contains("20 MB"));
        assert!(read_dropped(dir.to_str().unwrap()).is_err());
    }

    #[test]
    fn existence_is_checked_only_inside_attachments() {
        let data = crate::test_support::Scratch::new("attach-exist");
        let root = attachments_root(&data).unwrap();
        let name = write_file(&root, b"hello", Some("a.txt")).unwrap();
        std::fs::write(data.join("agents").join("secret.txt"), b"s").unwrap();
        std::fs::write(data.join("outside.txt"), b"o").unwrap();
        let paths = [
            format!("agents/attachments/{name}"),
            "agents/attachments/gone.png".to_string(),
            "agents/attachments/../secret.txt".to_string(),
            "agents/attachments/..".to_string(),
            "agents/secret.txt".to_string(),
            data.join("outside.txt").to_string_lossy().into_owned(),
            "../outside.txt".to_string(),
            "agents/attachments/".to_string(),
        ];
        assert_eq!(
            attachments_exist(&data, &paths),
            vec![true, false, false, false, false, false, false, false]
        );
        for refused in &paths[2..] {
            assert!(resolve_attachment(&root, refused).is_err(), "{refused}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_link_cannot_lead_out_of_attachments() {
        let data = crate::test_support::Scratch::new("attach-link");
        let elsewhere = crate::test_support::Scratch::new("attach-elsewhere");
        std::fs::write(elsewhere.join("secret.txt"), b"s").unwrap();
        let root = attachments_root(&data).unwrap();
        std::os::unix::fs::symlink(elsewhere.join("secret.txt"), root.join("leak.txt")).unwrap();
        assert!(resolve_attachment(&root, "agents/attachments/leak.txt").is_err());
        assert_eq!(
            attachments_exist(&data, &["agents/attachments/leak.txt".into()]),
            vec![false]
        );

        // The folder itself linked elsewhere: nothing is written or read there.
        let linked = crate::test_support::Scratch::new("attach-linked");
        std::fs::create_dir_all(linked.join("agents")).unwrap();
        std::os::unix::fs::symlink(&*elsewhere, linked.join("agents").join("attachments")).unwrap();
        assert!(attachments_root(&linked).is_err());
        assert_eq!(
            attachments_exist(&linked, &["agents/attachments/secret.txt".into()]),
            vec![false]
        );
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
