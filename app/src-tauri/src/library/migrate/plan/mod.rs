//! Deciding the whole move before anything moves: every rename, in the order
//! it must run, the rules old paths map by, and the markdown to re-point.
//! Each step's `from` is where the thing is when that step runs, so a step
//! that picks a file out of a folder renamed earlier names its new place.
//! Names are chosen against what is on disk now plus what earlier steps put
//! there, ignoring case as the filesystem does.

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

/// Every step of the move, from the folders on disk and the database's rows.
pub fn build(data_dir: &Path, rows: &Rows) -> Plan {
    let mut b = Builder::new(data_dir, rows);
    walk::debris(&mut b);
    for name in subject_dirs(data_dir) {
        subject::plan(&mut b, &name);
    }
    loose_files(&mut b);
    lectures::plan(&mut b);
    b.finish()
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
    /// Names each folder (data-dir-relative, lowercased) will hold that are
    /// not on disk yet, or that the plan treats as held.
    placed: HashMap<String, HashSet<String>>,
    /// Lowercased names on disk per folder, read once.
    on_disk: HashMap<String, HashSet<String>>,
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
        let lower = name.to_lowercase();
        if self
            .placed
            .get(dir)
            .is_some_and(|names| names.contains(&lower))
        {
            return true;
        }
        let data_dir = self.data_dir;
        self.on_disk
            .entry(dir.to_string())
            .or_insert_with(|| {
                list(&data_dir.join(dir))
                    .into_iter()
                    .map(|(n, _)| n.to_lowercase())
                    .collect()
            })
            .contains(&lower)
    }

    /// Hold `name` in `dir`, so no later step picks it.
    pub fn hold(&mut self, dir: &str, name: &str) {
        self.placed
            .entry(dir.to_string())
            .or_default()
            .insert(name.to_lowercase());
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
