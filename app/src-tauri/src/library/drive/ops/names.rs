//! Entry names: which the student may give, how a picked file's name is made
//! safe, and the free name a clash steps aside to (` (2)`, as Finder does).

use std::path::Path;

use super::error::{DriveError, DriveResult};
use crate::library::layout;

/// Longest name most filesystems take, in bytes.
const MAX_NAME: usize = 255;

/// Refuses a name the student may not give: empty, `.`/`..`, a dot-name
/// (hidden, and the own-file guard refuses it), a separator, NUL, `:` (a
/// drive or stream on Windows), a control character, a leading or trailing
/// space, and at a subject's root a name the layout keeps (`Course`).
pub fn check_name(name: &str, at_subject_root: bool) -> DriveResult<()> {
    let refuse = |why: &str| Err(DriveError::invalid_name(why));
    if name.is_empty() {
        return refuse("A name can't be empty");
    }
    if name.starts_with('.') {
        return refuse("A name can't start with a dot");
    }
    if name.contains(['/', '\\', ':', '\0']) {
        return refuse("A name can't hold / \\ or :");
    }
    if name.chars().any(char::is_control) {
        return refuse("A name can't hold control characters");
    }
    if name.trim() != name {
        return refuse("A name can't start or end with a space");
    }
    if name.len() > MAX_NAME {
        return refuse("That name is too long");
    }
    if at_subject_root && layout::reserved_name(name) {
        return Err(DriveError::invalid_name(format!(
            "{name} is a name Oculus keeps for itself"
        )));
    }
    Ok(())
}

/// A picked file or folder's name as Drive keeps it: what `check_name`
/// refuses is replaced, a leading dot dropped, trailing dots and spaces
/// trimmed (Windows drops them), a Windows device name prefixed.
pub fn entry_name(picked: &str) -> String {
    let replaced: String = picked
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '\0' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = replaced
        .trim()
        .trim_start_matches('.')
        .trim_end_matches(['.', ' ']);
    let name = if trimmed.is_empty() { "file" } else { trimmed };
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|n| n.len() == 1 && n.chars().all(|c| c.is_ascii_digit() && c != '0'));
    let name = if device {
        format!("_{name}")
    } else {
        name.to_string()
    };
    truncate(&name)
}

fn truncate(name: &str) -> String {
    if name.len() <= MAX_NAME {
        return name.to_string();
    }
    let mut end = MAX_NAME;
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    name[..end].to_string()
}

/// `name`, else `stem (2).ext`, `stem (3).ext`… (a folder: `name (2)`) — the
/// first that nothing in `dir` holds, or that `ours` says already is this
/// entry. Existence is the disk's, so a case-only clash counts on a
/// case-insensitive volume.
pub fn free_name(dir: &Path, name: &str, folder: bool, ours: impl Fn(&Path) -> bool) -> String {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !folder && !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    for n in 1.. {
        let candidate = if n == 1 {
            name.to_string()
        } else {
            format!("{stem} ({n}){ext}")
        };
        let path = dir.join(&candidate);
        if std::fs::symlink_metadata(&path).is_err() || ours(&path) {
            return candidate;
        }
    }
    unreachable!()
}
