//! Deciding every rename before the first: the two records folders, then each
//! drawing among the student's own files. A drawing takes `<stem>.whiteboard`,
//! or `<stem> (2).whiteboard`… when something has that name already, compared
//! as the volume compares names (`name_key`), so nothing is ever overwritten.

use std::collections::{HashMap, HashSet};
use std::io::BufReader;
use std::path::Path;

use serde::Deserialize;

use super::db::Rows;
use super::journal::Plan;
use super::{LEGACY_CATEGORY, LEGACY_DRAFTS_DIR, LEGACY_EXTENSION, LEGACY_PAGES_DIR};
use crate::library::files::whiteboards::{self, DrawingKind};
use crate::library::layout;
use crate::library::migrate::journal::{Kind, Op, Step};
use crate::library::migrate::plan::{base, join, list, name_key, parent, suffixed};
use crate::library::migrate::rules::Rule;
use crate::library::paths::PAGES_DIR;

pub fn build(data_dir: &Path, rows: &Rows) -> Plan {
    let mut plan = Plan::default();
    records(data_dir, &mut plan, LEGACY_PAGES_DIR, PAGES_DIR);
    records(
        data_dir,
        &mut plan,
        LEGACY_DRAFTS_DIR,
        whiteboards::drafts::DIR,
    );
    let mut names = Names::new(data_dir, rows);
    for code in layout::subjects_on_disk(data_dir) {
        for (rel, _) in layout::own_files(data_dir, &code) {
            if !rel.ends_with(LEGACY_EXTENSION) {
                continue;
            }
            if !is_drawing(&data_dir.join(&rel)) {
                plan.left
                    .push(format!("{rel}: not an Oculus drawing, so kept as it is"));
                continue;
            }
            let to = names.renamed(&rel);
            add(&mut plan, &rel, &to);
        }
    }
    stale_rows(data_dir, &mut plan, &mut names, rows);
    plan.recategorize = rows
        .drawings
        .iter()
        .any(|(_, _, category)| category.as_deref() == Some(LEGACY_CATEGORY));
    plan
}

fn add(plan: &mut Plan, from: &str, to: &str) {
    plan.rules.push(Rule {
        old: from.to_string(),
        new: to.to_string(),
    });
    plan.steps.push(Step {
        kind: Kind::Whiteboard,
        op: Op::Rename {
            from: from.to_string(),
            to: to.to_string(),
        },
        done: false,
    });
}

/// A records folder keyed by file id, to its new name: whole when nothing is
/// there yet, else entry by entry, an id already there keeping its own.
fn records(data_dir: &Path, plan: &mut Plan, old: &str, new: &str) {
    let step = |plan: &mut Plan, op: Op| {
        plan.steps.push(Step {
            kind: Kind::DrawingRecords,
            op,
            done: false,
        })
    };
    match std::fs::symlink_metadata(data_dir.join(old)) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => {
            plan.left.push(format!(
                "{old}: not a folder, so its drawings' records stay"
            ));
            return;
        }
        Err(_) => return,
    }
    match std::fs::symlink_metadata(data_dir.join(new)) {
        Err(_) => {
            step(
                plan,
                Op::Rename {
                    from: old.to_string(),
                    to: new.to_string(),
                },
            );
            return;
        }
        Ok(meta) if !meta.is_dir() => {
            plan.left
                .push(format!("{old}: kept, since {new} is not a folder"));
            return;
        }
        Ok(_) => {}
    }
    for (name, _) in list(&data_dir.join(old)) {
        let to = join(new, &name);
        if std::fs::symlink_metadata(data_dir.join(&to)).is_ok() {
            plan.left
                .push(format!("{old}/{name}: kept, since {to} is there"));
            continue;
        }
        step(
            plan,
            Op::Rename {
                from: join(old, &name),
                to,
            },
        );
    }
    step(
        plan,
        Op::RemoveDirIfEmpty {
            path: old.to_string(),
        },
    );
}

/// True for a file holding an Oculus drawing (its JSON `type` says so); a
/// `.canvas` from another app (Obsidian's, say) stays as it is. Only the
/// type is kept as the JSON is read.
fn is_drawing(path: &Path) -> bool {
    #[derive(Deserialize)]
    struct Head {
        #[serde(rename = "type")]
        kind: Option<String>,
    }
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    serde_json::from_reader::<_, Head>(BufReader::new(file))
        .ok()
        .and_then(|head| head.kind)
        .is_some_and(|kind| whiteboards::authority::model::is_drawing_type(&kind))
}

/// A row for a drawing that is not on disk any more keeps a rule, so its row
/// and old citations follow the rename as the others do.
fn stale_rows(data_dir: &Path, plan: &mut Plan, names: &mut Names, rows: &Rows) {
    let renamed: HashSet<String> = plan.rules.iter().map(|r| r.old.clone()).collect();
    for (_, rel, category) in &rows.drawings {
        if category.as_deref() != Some(LEGACY_CATEGORY)
            || !rel.ends_with(LEGACY_EXTENSION)
            || renamed.contains(rel)
            || std::fs::symlink_metadata(data_dir.join(rel)).is_ok()
        {
            continue;
        }
        let to = names.renamed(rel);
        plan.rules.push(Rule {
            old: rel.clone(),
            new: to,
        });
    }
}

/// The names each folder holds, on disk, in the database, or by an earlier
/// step, as the volume compares them.
struct Names<'a> {
    data_dir: &'a Path,
    rows: HashSet<String>,
    folders: HashMap<String, HashSet<String>>,
}

impl<'a> Names<'a> {
    fn new(data_dir: &'a Path, rows: &Rows) -> Self {
        Self {
            data_dir,
            rows: rows.taken.iter().map(|p| name_key(p)).collect(),
            folders: HashMap::new(),
        }
    }

    /// `rel` (a `.canvas`) as a whiteboard: the first free name, held from
    /// now on.
    fn renamed(&mut self, rel: &str) -> String {
        let dir = parent(rel).to_string();
        let stem = &base(rel)[..base(rel).len() - LEGACY_EXTENSION.len()];
        let wanted = format!("{stem}{}", DrawingKind::Whiteboard.extension());
        let data_dir = self.data_dir;
        let held = self.folders.entry(name_key(&dir)).or_insert_with(|| {
            list(&data_dir.join(&dir))
                .into_iter()
                .map(|(n, _)| name_key(&n))
                .collect()
        });
        let chosen = (1..)
            .map(|n| suffixed(&wanted, n, false))
            .find(|name| {
                !held.contains(&name_key(name)) && !self.rows.contains(&name_key(&join(&dir, name)))
            })
            .expect("an unbounded range has a free name");
        held.insert(name_key(&chosen));
        join(&dir, &chosen)
    }
}
