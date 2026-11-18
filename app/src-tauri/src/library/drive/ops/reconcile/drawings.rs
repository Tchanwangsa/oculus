//! Drawings still named `.canvas` (`whiteboards::legacy`) among a subject's
//! own files, renamed to `.whiteboard` before the reconcile looks for rows:
//! one copied in from a backup, another Mac or Finder, or written by an
//! agent, after the launch rename (`migrate::whiteboard`). Another app's
//! `.canvas` stays as it is. One that does not read as whole JSON is left,
//! rowless, for a later pass while it may still be landing (`LANDING`), and
//! is someone else's file after that. A new name never takes one the folder
//! or a row holds, as the volume compares names.

use std::collections::{HashMap, HashSet};
use std::fs::Metadata;
use std::path::Path;
use std::time::{Duration, SystemTime};

use super::super::journal::Journal;
use crate::library::files::whiteboards::legacy::{self, Contents};
use crate::library::layout;
use crate::library::migrate::plan::name_key;

/// One drawing renamed, with the row it had: its id and the path it holds,
/// which may be spelled other than the disk's (`name_key`).
pub struct Converted {
    pub from: String,
    pub to: String,
    pub row: Option<(i64, String)>,
}

/// How long after its last write a `.canvas` that does not read as JSON may
/// still be landing (a Finder copy, a sync client). Misjudging costs one
/// pass either way: a drawing that lands later than this gets an upload row
/// first, and the next pass renames it with that row by id.
const LANDING: Duration = Duration::from_secs(30);

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
    let index = RowIndex::new(rows);
    for (rel, meta) in layout::own_files(data_dir, code) {
        let (dir, name) = rel.rsplit_once('/').unwrap_or(("", &rel));
        if !legacy::is_legacy_name(name) {
            continue;
        }
        match legacy::read(&data_dir.join(&rel)) {
            Contents::Drawing => {}
            Contents::Other => continue,
            Contents::Unreadable(e) if landing(&meta) => {
                elog!("[oculus] {rel} is left as it is until it reads whole: {e}");
                pass.waiting.insert(rel);
                continue;
            }
            Contents::Unreadable(e) => {
                if index.find(&rel).is_none() {
                    elog!("[oculus] {rel} is not a drawing ({e}), so an upload");
                }
                continue;
            }
        }
        let new_name = legacy::whiteboard_name(name, |candidate| {
            std::fs::symlink_metadata(data_dir.join(dir).join(candidate)).is_ok()
                || index.holds(&format!("{dir}/{candidate}"))
        });
        let to = format!("{dir}/{new_name}");
        match journal.rename(&data_dir.join(&rel), &data_dir.join(&to)) {
            Ok(()) => pass.converted.push(Converted {
                row: index.find(&rel),
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

/// The subject's rows as the volume finds them: by exact spelling, else by
/// `name_key` (one Unicode form, no case) when one row alone has it.
struct RowIndex<'a> {
    exact: &'a HashMap<String, i64>,
    /// `None` when two rows fold to the same key.
    folded: HashMap<String, Option<(i64, &'a str)>>,
}

impl<'a> RowIndex<'a> {
    fn new(exact: &'a HashMap<String, i64>) -> Self {
        let mut folded = HashMap::new();
        for (rel, id) in exact {
            folded
                .entry(name_key(rel))
                .and_modify(|found| *found = None)
                .or_insert(Some((*id, rel.as_str())));
        }
        Self { exact, folded }
    }

    /// The row for the file at `rel`: its id and the path it holds.
    fn find(&self, rel: &str) -> Option<(i64, String)> {
        if let Some(id) = self.exact.get(rel) {
            return Some((*id, rel.to_string()));
        }
        let (id, path) = self.folded.get(&name_key(rel)).copied().flatten()?;
        Some((id, path.to_string()))
    }

    /// Some row holds `rel` as the volume compares names.
    fn holds(&self, rel: &str) -> bool {
        self.folded.contains_key(&name_key(rel))
    }
}

/// Written within `LANDING` of now, either side (a clock a little off).
fn landing(meta: &Metadata) -> bool {
    let Ok(at) = meta.modified() else {
        return false;
    };
    let gap = match SystemTime::now().duration_since(at) {
        Ok(age) => age,
        Err(ahead) => ahead.duration(),
    };
    gap < LANDING
}
