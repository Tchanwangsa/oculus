//! Drawings still named `.canvas` (`whiteboards::legacy`) among a subject's
//! own files, renamed to `.whiteboard` before the reconcile looks for rows:
//! one copied in from a backup, another Mac or Finder, or written by an
//! agent, after the launch rename (`migrate::whiteboard`). Another app's
//! `.canvas` stays as it is. One that does not read as whole JSON yet (still
//! being copied) is left, rowless, for a later pass. A new name never takes
//! one the folder or a row holds, as the volume compares names.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::super::journal::Journal;
use crate::library::files::whiteboards::legacy::{self, Contents};
use crate::library::layout;
use crate::library::migrate::plan::name_key;

/// One drawing renamed, with the row it had.
pub struct Converted {
    pub from: String,
    pub to: String,
    pub row: Option<i64>,
}

/// What a pass did on disk.
#[derive(Default)]
pub struct Pass {
    pub converted: Vec<Converted>,
    /// `.canvas` files to give no row yet: they may still be landing.
    pub waiting: HashSet<String>,
}

/// Rename every `.canvas` drawing among `code`'s own files, each through
/// `journal` so the caller can put them back. `rows` is the subject's row
/// paths to ids. A file that cannot be read or renamed is logged and left.
pub fn convert(
    data_dir: &Path,
    code: &str,
    rows: &HashMap<String, i64>,
    journal: &mut Journal,
) -> Pass {
    let mut pass = Pass::default();
    let row_keys: HashSet<String> = rows.keys().map(|rel| name_key(rel)).collect();
    for (rel, _) in layout::own_files(data_dir, code) {
        let (dir, name) = rel.rsplit_once('/').unwrap_or(("", &rel));
        if !legacy::is_legacy_name(name) {
            continue;
        }
        match legacy::read(&data_dir.join(&rel)) {
            Contents::Drawing => {}
            Contents::Other => continue,
            Contents::Unreadable(e) => {
                elog!("[oculus] {rel} is left as it is until it reads whole: {e}");
                pass.waiting.insert(rel);
                continue;
            }
        }
        let new_name = legacy::whiteboard_name(name, |candidate| {
            std::fs::symlink_metadata(data_dir.join(dir).join(candidate)).is_ok()
                || row_keys.contains(&name_key(&format!("{dir}/{candidate}")))
        });
        let to = format!("{dir}/{new_name}");
        match journal.rename(&data_dir.join(&rel), &data_dir.join(&to)) {
            Ok(()) => pass.converted.push(Converted {
                row: rows.get(&rel).copied(),
                from: rel,
                to,
            }),
            Err(e) => {
                elog!("[oculus] {rel} stays a .canvas: renaming it to {new_name} failed: {e}");
                pass.waiting.insert(rel);
            }
        }
    }
    pass
}
