//! A drawing still named `.canvas`, the extension whiteboards had: told
//! apart from another app's `.canvas` (Obsidian's) by its JSON `type`, and
//! given the `.whiteboard` name it takes. The launch rename
//! (`library::migrate::whiteboard`), Drive's reconcile and imports all decide
//! with these.

use std::io::BufReader;
use std::path::Path;

use serde::Deserialize;

use super::authority::model::is_drawing_type;
use super::DrawingKind;

/// The extension a drawing had.
pub const EXTENSION: &str = ".canvas";

/// What a `.canvas` file holds.
#[derive(Debug, PartialEq, Eq)]
pub enum Contents {
    /// An Oculus drawing: its JSON `type` says so.
    Drawing,
    /// Whole JSON, or a file, of something else.
    Other,
    /// Not readable as JSON yet: a read error, or a file still being written.
    Unreadable(String),
}

#[derive(Deserialize)]
struct Head {
    #[serde(rename = "type")]
    kind: Option<String>,
}

fn of_head(head: Result<Head, serde_json::Error>) -> Contents {
    match head {
        Ok(Head { kind: Some(kind) }) if is_drawing_type(&kind) => Contents::Drawing,
        Ok(_) => Contents::Other,
        // Whole JSON that is not an object is someone else's file; only a
        // cut-off or garbled one may still be landing.
        Err(e) if e.is_data() => Contents::Other,
        Err(e) => Contents::Unreadable(e.to_string()),
    }
}

/// What the file at `path` holds. Only the type is kept as the JSON is read.
pub fn read(path: &Path) -> Contents {
    match std::fs::File::open(path) {
        Ok(file) => of_head(serde_json::from_reader(BufReader::new(file))),
        Err(e) => Contents::Unreadable(e.to_string()),
    }
}

/// What `bytes` hold.
pub fn of_bytes(bytes: &[u8]) -> Contents {
    of_head(serde_json::from_slice(bytes))
}

/// True for a `.canvas` name.
pub fn is_legacy_name(name: &str) -> bool {
    name.ends_with(EXTENSION) && name.len() > EXTENSION.len()
}

/// The `.canvas` name `name` as a whiteboard: `<stem>.whiteboard`, else
/// `<stem> (2).whiteboard`… — the first that `taken` refuses.
pub fn whiteboard_name(name: &str, mut taken: impl FnMut(&str) -> bool) -> String {
    let stem = name.strip_suffix(EXTENSION).unwrap_or(name);
    let ext = DrawingKind::Whiteboard.extension();
    (1..)
        .map(|n: usize| match n {
            1 => format!("{stem}{ext}"),
            n => format!("{stem} ({n}){ext}"),
        })
        .find(|candidate| !taken(candidate))
        .expect("an unbounded range has a free name")
}
