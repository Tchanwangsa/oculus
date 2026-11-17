//! One subject's folder: the synced folders into `Course/`, videos out of
//! `Files/` into `Media/`, the student's folders flattened to the root, the
//! agent folder sorted, then each parse record filed under
//! `.oculus/derived/` and each markdown file listed for re-pointing.

use super::walk::{find, record_home};
use super::{join, list, Builder, Entry};
use crate::library::layout::{self, FolderMoveKind, SyncedKind, COURSES_DIR};
use crate::library::migrate::journal::{Kind, Moved, Op};

/// Caption files a video may have beside it: `<stem>.vtt` and
/// `<name>.vtt` (`Week 1.mp4.vtt`, what transcription writes).
fn captions(name: &str) -> Vec<String> {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    vec![format!("{stem}.vtt"), format!("{name}.vtt")]
}

pub(super) fn plan(b: &mut Builder, name: &str) {
    let root = format!("{COURSES_DIR}/{name}");
    let moves = layout::legacy_folder_moves();
    let olds: Vec<String> = moves
        .iter()
        .filter(|m| m.kind != FolderMoveKind::NoteAssets)
        .map(|m| join(&root, &m.old))
        .collect();
    let found = find(b, &olds);
    // The old folders' names stay held at the root: a student's folder that
    // flattens there must not take one while it is still being emptied.
    for old in &olds {
        let held = super::base(old).to_string();
        b.hold(&root, &held);
    }

    for m in &moves {
        let (old, new) = (join(&root, &m.old), join(&root, &m.new));
        match m.kind {
            FolderMoveKind::Synced if b.is_dir(&old) => {
                b.move_dir(Kind::Synced, &old, &new);
            }
            FolderMoveKind::SyncedFile if b.exists(&old) => {
                let dir = super::parent(&new).to_string();
                let chosen = b.free_name(&dir, super::base(&new), false);
                b.rename(Kind::Synced, &old, &join(&dir, &chosen));
            }
            _ => {}
        }
    }
    media(b, &root);
    super::own::plan(b, &root, &moves);
    super::agents::plan(b, &root, name, &moves);

    for record in &found.records {
        let at = b.rules.apply(record);
        let Some(home) = record_home(&at) else {
            continue;
        };
        if home == at || b.exists(&home) {
            continue;
        }
        b.step(
            Kind::Derived,
            Op::Rename {
                from: at,
                to: home.clone(),
            },
        );
        b.rules.add(record, &home);
    }
    for old in found.markdown {
        let new = b.rules.apply(&old);
        if new != old {
            b.plan.notes.push(Moved { old, new });
        }
    }
    left_at_root(b, &root, &moves);
}

/// Videos the database knows in the old `Files/` (module videos, page-embed
/// downloads) go to `Media/`, with their captions.
fn media(b: &mut Builder, root: &str) {
    let files = join(root, &layout::legacy_files_dir());
    let media = join(root, &layout::synced_dir(SyncedKind::Media));
    let videos: Vec<String> = b
        .rows
        .videos
        .iter()
        .filter(|v| super::parent(v) == files && b.exists(v))
        .cloned()
        .collect();
    for old in videos {
        let name = super::base(&old).to_string();
        let chosen = b.free_name(&media, &name, false);
        b.rename_moved(Kind::Media, &old, &join(&media, &chosen));
        for (caption, new_caption) in captions(&name).into_iter().zip(captions(&chosen)) {
            let old_caption = join(&files, &caption);
            if b.exists(&old_caption) {
                let new_caption = join(&media, &new_caption);
                b.hold(&media, super::base(&new_caption));
                b.rename_moved(Kind::Media, &old_caption, &new_caption);
            }
        }
    }
}

/// What sits at a subject's root that no rule moves (a folder of unknown
/// purpose, a stray file): it stays, and is the student's now.
fn left_at_root(b: &mut Builder, root: &str, moves: &[layout::FolderMove]) {
    for (name, kind) in list(&b.data_dir.join(root)) {
        let rel = join(root, &name);
        let known = moves.iter().any(|m| join(root, &m.old) == rel)
            || name.starts_with('.')
            || kind == Entry::Symlink
            || layout::reserved_name(&name)
            || b.debris.contains(&rel);
        if !known && b.rules.apply(&rel) == rel {
            b.plan
                .left
                .push(format!("{rel}: kept at the subject's root"));
        }
    }
}

impl Builder<'_> {
    /// A rename of something an earlier step already moved: `old` is its
    /// path before the move, which the rule keeps; the step starts from where
    /// the earlier move put it.
    pub(super) fn rename_moved(&mut self, kind: Kind, old: &str, to: &str) {
        let from = self.rules.apply(old);
        self.hold(super::parent(to), super::base(to));
        self.rules.add(old, to);
        self.step(
            kind,
            Op::Rename {
                from,
                to: to.to_string(),
            },
        );
    }
}
