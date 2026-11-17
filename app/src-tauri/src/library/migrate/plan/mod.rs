//! Deciding the whole move before anything moves: every rename, in the order
//! it must run, the rules old paths map by, and the markdown to re-point.
//! Each step's `from` is where the thing is when that step runs, so a step
//! that picks a file out of a folder renamed earlier names its new place.
//! Names are chosen against what is on disk now plus what earlier steps put
//! there, compared as the filesystem compares them (`name_key`).

mod agents;
mod lectures;
mod own;
mod subject;
mod walk;

pub(super) use own::FINDER_FILE;
pub(super) use walk::holds_no_file;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::db::Rows;
use super::journal::{Kind, Op, Plan, Step};
use super::rules::RuleList;
use crate::library::layout::{self, FolderMoveKind, COURSES_DIR};

/// True when the data dir still holds the old layout anywhere: a subject
/// with an old folder in it, or the data-dir recordings folder.
pub fn has_legacy(data_dir: &Path) -> bool {
    if data_dir.join(layout::legacy_recordings_root()).exists() {
        return true;
    }
    let moves = layout::legacy_folder_moves();
    subject_dirs(data_dir).iter().any(|name| {
        moves.iter().any(|m| {
            m.kind != FolderMoveKind::NoteAssets
                && std::fs::symlink_metadata(data_dir.join(COURSES_DIR).join(name).join(&m.old))
                    .is_ok_and(|meta| !meta.file_type().is_symlink())
        })
    })
}

/// Refuse a library the move cannot do by renaming: a recordings folder
/// that is a symlink (to another volume, most likely) would have its
/// recordings renamed across the link, or skipped. It is refused, so the
/// student moves it back first.
pub fn refuse(data_dir: &Path) -> Result<(), String> {
    let root = layout::legacy_recordings_root();
    let path = data_dir.join(root);
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            let target = std::fs::read_link(&path)
                .map(|t| t.display().to_string())
                .unwrap_or_else(|_| "somewhere else".into());
            Err(format!(
                "the recordings folder {} is a link to {target}. Oculus moves the library \
                 only by renaming inside the data folder, so put that folder back at {} \
                 (not a link) and open Oculus again",
                path.display(),
                path.display()
            ))
        }
        _ => Ok(()),
    }
}

/// Every step of the move, from the folders on disk and the database's rows.
pub fn build(data_dir: &Path, rows: &Rows) -> Plan {
    let mut b = Builder::new(data_dir, rows);
    walk::debris(&mut b);
    for name in subject_dirs(data_dir) {
        subject::plan(&mut b, &name);
    }
    loose_files(&mut b);
    lectures::plan(&mut b);
    stale_rows(&mut b);
    b.finish()
}

/// A `files` row's claim on a new path: its id, its old path, and whether
/// its file is on disk.
type Claim = (i64, String, bool);

/// Two `files` rows of one subject must not end at one path: a row whose
/// file is gone can map onto a file that moved there from another folder
/// (`uploads/a.pdf` and a stale `documents/a.pdf` both flatten to `a.pdf`),
/// and the UPDATE would break the table's uniqueness. A row whose file is
/// on disk keeps the path; the stale one takes a free name beside it.
fn stale_rows(b: &mut Builder) {
    let mut ends: HashMap<(i64, String), Vec<Claim>> = HashMap::new();
    for (id, subject, old) in &b.rows.files {
        let new = b.rules.apply(old);
        let on_disk = b.exists(old);
        ends.entry((*subject, new))
            .or_default()
            .push((*id, old.clone(), on_disk));
    }
    let mut clashes: Vec<((i64, String), Vec<Claim>)> = ends
        .into_iter()
        .filter(|(_, rows)| rows.len() > 1)
        .collect();
    clashes.sort();
    for ((_, new), mut rows) in clashes {
        // The row on disk (or else the oldest) keeps the path.
        rows.sort_by_key(|(id, _, on_disk)| (!on_disk, *id));
        for (_, old, _) in rows.into_iter().skip(1) {
            let chosen = b.free_name(parent(&new), base(&new), false);
            let to = join(parent(&new), &chosen);
            b.rules.add(&old, &to);
            b.plan.left.push(format!(
                "{old}: its file is gone, and its row now says {to} to keep {new} for the file there"
            ));
        }
    }
}

/// The subject folders, by name, in order.
fn subject_dirs(data_dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = list(&data_dir.join(COURSES_DIR))
        .into_iter()
        .filter(|(name, kind)| *kind == Entry::Dir && !name.starts_with('.'))
        .map(|(name, _)| name)
        .collect();
    names.sort();
    names
}

/// Files at `courses/` itself belong to no subject: left where they are.
fn loose_files(b: &mut Builder) {
    for (name, kind) in list(&b.data_dir.join(COURSES_DIR)) {
        if kind == Entry::File && !name.starts_with('.') {
            b.plan
                .left
                .push(format!("{COURSES_DIR}/{name}: in no subject"));
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Entry {
    Dir,
    File,
    Symlink,
}

/// A folder's entries with what each is (a symlink is never followed);
/// empty when it cannot be read.
pub(super) fn list(dir: &Path) -> Vec<(String, Entry)> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, Entry)> = read
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let kind = e.file_type().ok()?;
            let kind = if kind.is_symlink() {
                Entry::Symlink
            } else if kind.is_dir() {
                Entry::Dir
            } else {
                Entry::File
            };
            Some((name, kind))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// `a/b` + `c`.
pub(super) fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else if name.is_empty() {
        dir.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

pub(super) fn parent(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(dir, _)| dir)
}

pub(super) fn base(rel: &str) -> &str {
    rel.rsplit_once('/').map_or(rel, |(_, name)| name)
}

/// How the volumes Oculus runs on compare names: one Unicode form (an NFC
/// `café` and an NFD one are the same file on APFS) and no case.
pub(super) fn name_key(name: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    name.nfc().collect::<String>().to_lowercase()
}

/// `name (n)`, before the extension for a file: `notes (2).md`, `Week 3 (2)`.
pub(super) fn suffixed(name: &str, n: usize, is_dir: bool) -> String {
    if n < 2 {
        return name.to_string();
    }
    match name.rfind('.').filter(|&i| i > 0 && !is_dir) {
        Some(i) => format!("{} ({n}){}", &name[..i], &name[i..]),
        None => format!("{name} ({n})"),
    }
}

pub(super) struct Builder<'a> {
    pub data_dir: &'a Path,
    pub rows: &'a Rows,
    pub plan: Plan,
    pub rules: RuleList,
    /// Data-dir-relative folders the debris sweep deletes, so nothing plans
    /// to move them.
    pub debris: HashSet<String>,
    /// Names each folder (data-dir-relative, `name_key`) will hold that are
    /// not on disk yet, or that the plan treats as held.
    placed: HashMap<String, HashSet<String>>,
    /// Names on disk per folder (`name_key`), read once.
    on_disk: HashMap<String, HashSet<String>>,
    /// Folders moved aside by an earlier step (`name_key`).
    vacated: Vec<String>,
}

impl<'a> Builder<'a> {
    fn new(data_dir: &'a Path, rows: &'a Rows) -> Self {
        Self {
            data_dir,
            rows,
            plan: Plan::default(),
            rules: RuleList::default(),
            debris: HashSet::new(),
            placed: HashMap::new(),
            on_disk: HashMap::new(),
            vacated: Vec::new(),
        }
    }

    fn finish(mut self) -> Plan {
        self.plan.rules = self.rules.rules;
        self.plan
    }

    pub fn exists(&self, rel: &str) -> bool {
        std::fs::symlink_metadata(self.data_dir.join(rel)).is_ok()
    }

    pub fn is_dir(&self, rel: &str) -> bool {
        std::fs::symlink_metadata(self.data_dir.join(rel)).is_ok_and(|m| m.is_dir())
    }

    /// True when `dir/name` is on disk now or some step puts something there.
    pub fn taken(&mut self, dir: &str, name: &str) -> bool {
        let (dir, name) = (name_key(dir), name_key(name));
        if self
            .placed
            .get(&dir)
            .is_some_and(|names| names.contains(&name))
        {
            return true;
        }
        if self.vacated_under(&dir) || self.vacated_under(&format!("{dir}/{name}")) {
            return false;
        }
        let data_dir = self.data_dir;
        self.on_disk
            .entry(dir)
            .or_insert_with_key(|dir| {
                list(&data_dir.join(dir))
                    .into_iter()
                    .map(|(n, _)| name_key(&n))
                    .collect()
            })
            .contains(&name)
    }

    /// True when `dir` (a `name_key`) is, or is inside, a folder an earlier
    /// step moved aside: what is on disk there now is not there any more.
    fn vacated_under(&self, dir: &str) -> bool {
        self.vacated
            .iter()
            .any(|v| dir == v || dir.starts_with(&format!("{v}/")))
    }

    /// Hold `name` in `dir`, so no later step picks it.
    pub fn hold(&mut self, dir: &str, name: &str) {
        self.placed
            .entry(name_key(dir))
            .or_default()
            .insert(name_key(name));
    }

    /// Say that `rel` is moved away before anything else is put at its
    /// path, so its contents on disk now count for nothing there.
    pub fn vacate(&mut self, rel: &str) {
        self.vacated.push(name_key(rel));
    }

    /// `name`, or `name (2)`, `name (3)`… — the first one `dir` does not
    /// hold; held from now on.
    pub fn free_name(&mut self, dir: &str, name: &str, is_dir: bool) -> String {
        let chosen = (1..)
            .map(|n| suffixed(name, n, is_dir))
            .find(|candidate| !self.taken(dir, candidate))
            .expect("an unbounded range has a free name");
        self.hold(dir, &chosen);
        chosen
    }

    pub fn step(&mut self, kind: Kind, op: Op) {
        self.plan.steps.push(Step {
            kind,
            op,
            done: false,
        });
    }

    /// One rename, and the rule that says where `from` went.
    pub fn rename(&mut self, kind: Kind, from: &str, to: &str) {
        // The folders the rename creates on the way are held too.
        let mut held = to;
        while !held.is_empty() {
            self.hold(parent(held), base(held));
            held = parent(held);
        }
        self.rules.add(from, to);
        self.step(
            kind,
            Op::Rename {
                from: from.to_string(),
                to: to.to_string(),
            },
        );
    }

    /// A folder to `to`, whole when nothing is there yet; into it entry by
    /// entry when something is (a name clash takes `name (2)`), and the
    /// emptied folder removed after.
    pub fn move_dir(&mut self, kind: Kind, from: &str, to: &str) {
        let dir_free = !self.taken(parent(to), base(to));
        if dir_free {
            self.rename(kind, from, to);
            return;
        }
        self.rules.add(from, to);
        for (name, entry) in list(&self.data_dir.join(from)) {
            let child = join(from, &name);
            if self.debris.contains(&child) {
                continue;
            }
            let chosen = self.free_name(to, &name, entry == Entry::Dir);
            let dest = join(to, &chosen);
            self.step(
                kind,
                Op::Rename {
                    from: child.clone(),
                    to: dest.clone(),
                },
            );
            if chosen != name {
                self.rules.add(&child, &dest);
            }
        }
        self.step(
            Kind::Cleanup,
            Op::RemoveDirIfEmpty {
                path: from.to_string(),
            },
        );
    }
}
