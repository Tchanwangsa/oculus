//! A subject's old agent folder: what the app keeps there (the link to the
//! subject's memories, the student's `INSTRUCTIONS.md`, a CLI's dot-folders)
//! goes to `.oculus/agents/`; what agents wrote there for the student (notes,
//! worked answers) goes to the subject's `Agent notes/`, where it stays in
//! sight. The memories link is remade at its new depth after the move.

use super::own::FINDER_FILE;
use super::{join, list, Builder, Entry};
use crate::library::layout::{self, FolderMove, FolderMoveKind};
use crate::library::migrate::journal::{Kind, Op};

pub(super) fn plan(b: &mut Builder, root: &str, name: &str, moves: &[FolderMove]) {
    let Some(m) = moves.iter().find(|m| m.kind == FolderMoveKind::Agents) else {
        return;
    };
    let dir = join(root, &m.old);
    if !b.is_dir(&dir) {
        return;
    }
    let home = join(root, &m.new);
    b.rules.add(&dir, &home);
    let mut notes: Option<String> = None;
    for (entry, kind) in list(&b.data_dir.join(&dir)) {
        let path = join(&dir, &entry);
        if b.debris.contains(&path) {
            continue;
        }
        if entry == FINDER_FILE {
            b.step(Kind::Cleanup, Op::RemoveFile { path });
        } else if entry == crate::agents::MEMORIES_DIR {
            b.plan.relink.push(name.to_string());
            if kind == Entry::Symlink {
                b.step(Kind::Agents, Op::RemoveFile { path });
            } else {
                b.move_dir(Kind::Agents, &path, &join(&home, &entry));
            }
        } else if entry == layout::SUBJECT_INSTRUCTIONS || entry.starts_with('.') {
            let chosen = b.free_name(&home, &entry, kind == Entry::Dir);
            b.rename(Kind::Agents, &path, &join(&home, &chosen));
        } else {
            let folder = notes
                .get_or_insert_with(|| {
                    let chosen = b.free_name(root, layout::AGENT_NOTES, true);
                    join(root, &chosen)
                })
                .clone();
            let chosen = b.free_name(&folder, &entry, kind == Entry::Dir);
            b.rename(Kind::AgentNotes, &path, &join(&folder, &chosen));
        }
    }
    b.step(Kind::Cleanup, Op::RemoveDirIfEmpty { path: dir });
}
