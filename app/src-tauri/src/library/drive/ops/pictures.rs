//! A note's pictures follow it to another folder. They sit in the pictures
//! folder beside the note (`layout::note_assets_dir`) and the note links
//! them as `.assets/<file>`, so a note moved away takes each picture it links
//! into the new folder's `.assets/`: moved, or copied when a note left
//! behind still links the same file. A name taken there by different bytes
//! steps aside to `<stem>-2.<ext>` (no space or bracket, which would end the
//! link), and the note's link is rewritten to match; links stay
//! `.assets/<file>`.

use std::collections::BTreeSet;
use std::path::Path;

use super::guard::parent_rel;
use crate::library::layout::{self, OwnKind};

/// The picture file names `text` links in the pictures folder.
pub fn linked(text: &str) -> BTreeSet<String> {
    let prefix = format!("{}/", layout::note_assets_dir());
    let mut names = BTreeSet::new();
    let mut rest = text;
    while let Some(at) = rest.find(&prefix) {
        let before = rest[..at].chars().next_back();
        rest = &rest[at + prefix.len()..];
        // `./.assets/x` and `(.assets/x` link it; `foo.assets/x` does not.
        if before.is_some_and(|c| !matches!(c, '(' | '/' | '<' | ' ' | '"' | '\'' | '\n')) {
            continue;
        }
        let end = rest
            .find(|c: char| matches!(c, ')' | '>' | '"' | '\'' | ']') || c.is_whitespace())
            .unwrap_or(rest.len());
        let name = &rest[..end];
        if !name.is_empty() && !name.contains('/') && !name.starts_with('.') {
            names.insert(name.to_string());
        }
    }
    names
}

/// True when `rel` is a note of the student's (not a parser's text).
pub fn is_note(data_dir: &Path, rel: &str) -> bool {
    layout::own_file_ok(rel, OwnKind::Document) && !layout::is_derived_text(data_dir, rel)
}

/// Carry the pictures of the note now at `to_rel`, which was at `from_rel`
/// (or, with `copy`, still is), into its new folder. Nothing to do when the
/// folder did not change.
pub fn carry(data_dir: &Path, from_rel: &str, to_rel: &str, copy: bool) -> Result<(), String> {
    let (from_dir, to_dir) = (parent_rel(from_rel), parent_rel(to_rel));
    if from_dir == to_dir {
        return Ok(());
    }
    let note = data_dir.join(to_rel);
    let mut text = std::fs::read_to_string(&note).map_err(|e| e.to_string())?;
    let assets = layout::note_assets_dir();
    let src = data_dir.join(from_dir).join(assets);
    let dst = data_dir.join(to_dir).join(assets);
    let mut rewritten = false;
    for name in linked(&text) {
        let from = src.join(&name);
        let Ok(bytes) = std::fs::read(&from) else {
            continue;
        };
        std::fs::create_dir_all(&dst).map_err(|e| e.to_string())?;
        let free = free_picture_name(&dst, &name, &bytes);
        let to = dst.join(&free);
        let keep = copy || linked_elsewhere(data_dir, from_dir, &name);
        if !to.exists() {
            if keep {
                std::fs::copy(&from, &to).map_err(|e| e.to_string())?;
            } else {
                std::fs::rename(&from, &to).map_err(|e| e.to_string())?;
            }
        } else if !keep {
            std::fs::remove_file(&from).ok();
        }
        if free != name {
            text = text.replace(&format!("{assets}/{name}"), &format!("{assets}/{free}"));
            rewritten = true;
        }
    }
    if rewritten {
        let tmp = crate::library::files::documents::note_temp_path(&note)?;
        crate::runtime::atomic_write::write(&note, &tmp, text.as_bytes())?;
    }
    Ok(())
}

/// `name` in `dir`, else `stem-2.ext`, `stem-3.ext`… — the first free, or
/// already holding these bytes.
fn free_picture_name(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    (1..)
        .map(|n| match n {
            1 => name.to_string(),
            n => format!("{stem}-{n}{ext}"),
        })
        .find(|candidate| match std::fs::read(dir.join(candidate)) {
            Ok(existing) => existing == bytes,
            Err(_) => std::fs::symlink_metadata(dir.join(candidate)).is_err(),
        })
        .unwrap_or_else(|| name.to_string())
}

/// True when a note still in `dir_rel` links the picture `name`.
fn linked_elsewhere(data_dir: &Path, dir_rel: &str, name: &str) -> bool {
    let Ok(read) = std::fs::read_dir(data_dir.join(dir_rel)) else {
        return false;
    };
    read.flatten().any(|entry| {
        let rel = format!("{dir_rel}/{}", entry.file_name().to_string_lossy());
        is_note(data_dir, &rel)
            && std::fs::read_to_string(entry.path()).is_ok_and(|text| linked(&text).contains(name))
    })
}
