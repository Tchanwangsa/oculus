//! Lecture recordings: each `lectures/<media id>/` into its subject's
//! `Course/Recordings/<date>-<slug>/`, named as a new download's would be
//! (`layout::new_recording_rel`). A download cut short (`raw*.mp4`) is
//! dropped, not moved; a folder left with nothing in it is removed and its
//! lecture keeps no folder.

use std::collections::HashSet;

use super::own::FINDER_FILE;
use super::{join, list, Builder, Entry};
use crate::library::layout;
use crate::library::migrate::journal::{Kind, LectureMove, Moved, Op};
use crate::sources::echo360::is_partial_download;

pub(super) fn plan(b: &mut Builder) {
    let root = layout::legacy_recordings_root();
    if !b.is_dir(root) {
        return;
    }
    let mut taken: HashSet<String> = b.rows.taken_folders.clone();
    for (name, kind) in list(&b.data_dir.join(root)) {
        let dir = join(root, &name);
        if b.debris.contains(&dir) {
            continue;
        }
        if kind != Entry::Dir {
            if name == FINDER_FILE {
                b.step(Kind::Cleanup, Op::RemoveFile { path: dir });
            } else {
                b.plan.left.push(format!("{dir}: not a recording folder"));
            }
            continue;
        }
        let mut kept = 0;
        let mut markdown = Vec::new();
        for (entry, entry_kind) in list(&b.data_dir.join(&dir)) {
            let path = join(&dir, &entry);
            if b.debris.contains(&path) {
                continue;
            }
            if entry_kind == Entry::File && (is_partial_download(&entry) || entry == FINDER_FILE) {
                let kind = if entry == FINDER_FILE {
                    Kind::Cleanup
                } else {
                    Kind::Partial
                };
                b.step(kind, Op::RemoveFile { path });
                continue;
            }
            if entry_kind == Entry::File && entry.to_ascii_lowercase().ends_with(".md") {
                markdown.push(path);
            }
            kept += 1;
        }
        if kept == 0 {
            b.step(Kind::Cleanup, Op::RemoveDirIfEmpty { path: dir });
            continue;
        }
        let Some(row) = b.rows.lectures.iter().find(|l| l.id == name) else {
            b.plan.left.push(format!(
                "{dir}: no lecture on record, so no subject to file it in"
            ));
            continue;
        };
        let Some(code) = row.code.clone() else {
            b.plan
                .left
                .push(format!("{dir}: its lecture has no subject"));
            continue;
        };
        let target = match &row.folder_rel {
            Some(folder) => folder.clone(),
            None => {
                let data_dir = b.data_dir;
                layout::new_recording_rel(&code, &row.date, &row.title, |rel| {
                    taken.contains(&rel.to_lowercase()) || data_dir.join(rel).exists()
                })
            }
        };
        taken.insert(target.to_lowercase());
        b.move_dir(Kind::Recording, &dir, &target);
        b.plan.lectures.push(LectureMove {
            id: row.id.clone(),
            folder_rel: target,
        });
        for old in markdown {
            let new = b.rules.apply(&old);
            b.plan.notes.push(Moved { old, new });
        }
    }
    b.step(
        Kind::Cleanup,
        Op::RemoveDirIfEmpty {
            path: root.to_string(),
        },
    );
}
