//! `dump_files`: the files a run left behind, which Ed's web app lists under
//! the output (a course's `display_automaton` draws its picture this way).
//! Ed hides sources and dotfiles ([`shown`]). What each file's value looks
//! like is not pinned down, so [`decode`] takes a string (base64, a `data:`
//! URL or plain text) or an object carrying one, and logs the shape of
//! anything else by its name and kind only, never its contents.

use std::path::Path;

use base64::alphabet;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig, STANDARD};
use base64::engine::DecodePaddingMode;
use base64::Engine as _;
use serde_json::Value;

use crate::sources::ed::workspace_session::events::RunFile;

/// A file from the dump, as bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dumped {
    pub name: String,
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}

/// Files up to this size go to the pane inline, as a `data:` URL.
pub const INLINE_LIMIT: usize = 5 * 1024 * 1024;

/// The extensions Ed's web app takes for sources (its pattern's list; `""`
/// is a name ending in a dot).
const SOURCES: &[&str] = &[
    "", "sage", "kt", "nim", "cr", "o", "ali", "adb", "c", "cpp", "cxx", "cc", "h", "hpp", "java",
    "class", "jar", "r", "go", "rb", "jl", "sh", "m", "py", "f95", "sql", "swift", "cs", "vb",
];

/// Extensions whose string value is base64 whatever it decodes to.
const BINARY: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tif", "tiff", "pdf", "zip", "gz",
];

/// Whether Ed's web app lists `name` as an output file: not a dotfile, not
/// a source, and not a name without a dot (a compiled program).
pub fn shown(name: &str) -> bool {
    if name.starts_with('.') {
        return false;
    }
    match name.rsplit_once('.') {
        Some((_, ext)) => !SOURCES.contains(&ext),
        None => false,
    }
}

/// The output files in a `dump_files` frame's data: an object keyed by file
/// name, or a list of entries naming theirs (`filename`, `name`, `path`).
pub fn decode(data: &Value) -> Vec<Dumped> {
    let mut files = Vec::new();
    for (name, value) in entries(data) {
        if !shown(&name) {
            continue;
        }
        match bytes_of(&name, value) {
            Some(bytes) => files.push(Dumped {
                mime: mime_of(&name, &bytes),
                name,
                bytes,
            }),
            None => elog!(
                "[oculus] ed run: dump_files {name} is {}, not shown",
                kind(value)
            ),
        }
    }
    files
}

fn entries(data: &Value) -> Vec<(String, &Value)> {
    match data {
        Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v)).collect(),
        Value::Array(list) => list
            .iter()
            .filter_map(|item| {
                let name = ["filename", "name", "path"]
                    .iter()
                    .find_map(|k| item.get(*k)?.as_str());
                if name.is_none() {
                    elog!(
                        "[oculus] ed run: a dump_files entry is {} with no name, not shown",
                        kind(item)
                    );
                }
                Some((name?.to_string(), item))
            })
            .collect(),
        Value::Null => Vec::new(),
        other => {
            elog!("[oculus] ed run: dump_files is {}, not shown", kind(other));
            Vec::new()
        }
    }
}

/// A value's kind for the log, with an object's keys (never its values).
fn kind(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(_) => "a boolean".into(),
        Value::Number(_) => "a number".into(),
        Value::String(s) if is_link(s) => "a link".into(),
        Value::String(_) => "a string".into(),
        Value::Array(_) => "a list".into(),
        Value::Object(o) => {
            let keys: Vec<&str> = o.keys().map(String::as_str).collect();
            format!("an object with keys [{}]", keys.join(", "))
        }
    }
}

fn bytes_of(name: &str, value: &Value) -> Option<Vec<u8>> {
    match value {
        Value::String(text) => from_text(name, text, false),
        Value::Object(o) => {
            let base64 = o
                .get("encoding")
                .and_then(Value::as_str)
                .is_some_and(|e| e.eq_ignore_ascii_case("base64"));
            ["data", "content", "contents", "url"]
                .iter()
                .find_map(|k| o.get(*k)?.as_str())
                .and_then(|text| from_text(name, text, base64))
        }
        _ => None,
    }
}

fn is_link(text: &str) -> bool {
    text.starts_with("http://") || text.starts_with("https://")
}

/// A file's string: a `data:` URL, base64 (taken when it decodes to a known
/// binary format, the name says binary, or it decodes to readable text),
/// else the text itself. A link is not fetched.
fn from_text(name: &str, text: &str, base64: bool) -> Option<Vec<u8>> {
    if let Some(url) = text.strip_prefix("data:") {
        return data_url(url);
    }
    if is_link(text) {
        return None;
    }
    if base64 {
        return decode_base64(text);
    }
    match decode_base64(text) {
        Some(bytes) if sniff(&bytes).is_some() || binary_name(name) || readable(&bytes) => {
            Some(bytes)
        }
        _ => Some(text.as_bytes().to_vec()),
    }
}

/// `data:[<mime>][;base64],<payload>` after its scheme.
fn data_url(url: &str) -> Option<Vec<u8>> {
    let (meta, payload) = url.split_once(',')?;
    if meta.ends_with(";base64") {
        decode_base64(payload)
    } else {
        Some(payload.as_bytes().to_vec())
    }
}

/// Standard base64, padded or not, with line breaks ignored.
fn decode_base64(text: &str) -> Option<Vec<u8>> {
    const LENIENT: GeneralPurpose = GeneralPurpose::new(
        &alphabet::STANDARD,
        GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
    );
    let clean: String = text.chars().filter(|c| !matches!(c, '\r' | '\n')).collect();
    LENIENT.decode(clean).ok()
}

fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

fn binary_name(name: &str) -> bool {
    BINARY.contains(&extension(name).as_str())
}

/// UTF-8 with no NUL.
fn readable(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok_and(|s| !s.contains('\0'))
}

/// The format the bytes start with, for the ones the pane shows.
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    if bytes.starts_with(b"%PDF-") {
        return Some("application/pdf");
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]);
    let head = head.trim_start();
    if head.starts_with("<svg") || (head.starts_with("<?xml") && head.contains("<svg")) {
        return Some("image/svg+xml");
    }
    None
}

/// The file's type: its bytes first, then its extension.
pub fn mime_of(name: &str, bytes: &[u8]) -> &'static str {
    if let Some(mime) = sniff(bytes) {
        return mime;
    }
    match extension(name).as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        _ if readable(bytes) => "text/plain",
        _ => "application/octet-stream",
    }
}

/// Whether the pane shows a file of this type itself: a picture inline,
/// text in a dialog. Anything else it hands to the file manager.
fn shows_itself(mime: &str) -> bool {
    mime.starts_with("image/") || mime.starts_with("text/") || mime == "application/json"
}

/// The file as the pane gets it: a picture or text up to [`INLINE_LIMIT`]
/// inline, anything else written into `spill` (logged and left out when
/// that fails).
pub fn for_pane(file: Dumped, spill: &Path) -> Option<RunFile> {
    if file.bytes.len() <= INLINE_LIMIT && shows_itself(file.mime) {
        let data_url = format!("data:{};base64,{}", file.mime, STANDARD.encode(&file.bytes));
        return Some(RunFile {
            name: file.name,
            mime: file.mime.to_string(),
            data_url: Some(data_url),
            path: None,
        });
    }
    let base = Path::new(&file.name).file_name()?;
    let target = spill.join(base);
    let written =
        std::fs::create_dir_all(spill).and_then(|()| std::fs::write(&target, &file.bytes));
    if let Err(e) = written {
        elog!("[oculus] ed run: {} was not written: {e}", file.name);
        return None;
    }
    Some(RunFile {
        name: file.name,
        mime: file.mime.to_string(),
        data_url: None,
        path: Some(target.to_string_lossy().into_owned()),
    })
}
