//! `move_entries` and `rename_entry`: one path for both, since a rename is a
//! move into the same folder under another name.
//!
//! Disk first (journaled, inside the job fence), then the rows in one
//! transaction; if the rows fail the disk goes back. What cannot be undone
//! and needs nothing from the rows runs last: relinking a renamed parse,
//! dropping artifacts a new name has no slot for, carrying a note's
//! pictures, and pointing a drawing's page-picture manifest at its new path.

use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

use super::busy::fenced;
use super::companions::{companions, derived_dir, images_prefix};
use super::error::{shown, DriveError, DriveResult};
use super::guard::{
    files_under, is_subject_root, own_entry, parent_rel, subject_folder, target_folder, Entry,
};
use super::journal::{merge_move, Journal};
use super::names::check_name;
use super::pictures;
use super::rows::{self, MovedRow, RowMove};
use crate::library::layout::DerivedKind;
use crate::library::paths::{doc_pdf_rel, ArtifactSlot};

/// One entry and where it goes.
struct Plan {
    entry: Entry,
    to: String,
}

/// What the last step does for one moved file.
enum After {
    /// Relink the parse record and text now at these paths.
    Relink {
        record: PathBuf,
        markdown: PathBuf,
        old: String,
        new: String,
        pdf_name: String,
    },
    Remove(PathBuf),
    Pictures {
        from: String,
        to: String,
    },
}

/// Every moved path, `(from, to)`, in the order asked.
pub type Moves = Vec<(String, String)>;

/// Move `rels` into `target_dir`; returns their new paths in order. A name
/// already in the target is an `exists` error: nothing is overwritten.
pub async fn move_entries_at(
    data_dir: &Path,
    pool: &SqlitePool,
    rels: &[String],
    target_dir: &str,
) -> DriveResult<Moves> {
    let target = target_folder(data_dir, target_dir)?;
    let mut plans = Vec::new();
    for rel in rels {
        let entry = own_entry(data_dir, rel)?;
        if entry.dir && (target.rel == entry.rel || target.rel.starts_with(&format!("{rel}/"))) {
            return Err(DriveError::io(format!(
                "{} can't go inside itself",
                shown(rel)
            )));
        }
        let to = target.child_rel(shown(rel));
        if is_subject_root(&target.rel) && crate::library::layout::reserved_name(shown(rel)) {
            return Err(DriveError::invalid_name(format!(
                "{} is a name Oculus keeps for itself",
                shown(rel)
            )));
        }
        plans.push(Plan { entry, to });
    }
    run(data_dir, pool, plans, target.subject_folder()).await
}

/// Rename the entry at `rel` in place; returns its new path.
pub async fn rename_entry_at(
    data_dir: &Path,
    pool: &SqlitePool,
    rel: &str,
    new_name: &str,
) -> DriveResult<Moves> {
    let entry = own_entry(data_dir, rel)?;
    let parent = parent_rel(rel);
    check_name(new_name, is_subject_root(parent))?;
    let to = format!("{parent}/{new_name}");
    let folder = subject_folder(rel).unwrap_or_default().to_string();
    run(data_dir, pool, vec![Plan { entry, to }], &folder).await
}

async fn run(
    data_dir: &Path,
    pool: &SqlitePool,
    plans: Vec<Plan>,
    target_subject: &str,
) -> DriveResult<Moves> {
    check_free(&plans)?;
    let crossing = plans
        .iter()
        .any(|p| subject_folder(&p.entry.rel) != Some(target_subject));
    let subject = if crossing {
        Some(
            rows::subject_id(pool, target_subject)
                .await?
                .ok_or_else(|| DriveError::not_found(target_subject))?,
        )
    } else {
        None
    };
    let moves: Moves = plans
        .iter()
        .map(|p| (p.entry.rel.clone(), p.to.clone()))
        .collect();
    let files: Vec<String> = plans.iter().flat_map(|p| files_under(&p.entry)).collect();
    let dir = data_dir.to_path_buf();
    let ((journal, row_moves, after), held) = blocking(move || {
        fenced(&dir, &files, || {
            let mut journal = Journal::default();
            match on_disk(&dir, &plans, &mut journal) {
                Ok((row_moves, after)) => Ok((journal, row_moves, after)),
                Err(e) => {
                    journal.undo();
                    Err(e)
                }
            }
        })
    })
    .await?;
    let moved = match rows::apply_moves(pool, &row_moves, subject).await {
        Ok(moved) => moved,
        Err(e) => {
            blocking(move || {
                journal.undo();
                Ok(())
            })
            .await?;
            return Err(e);
        }
    };
    drop(held);
    let dir = data_dir.to_path_buf();
    blocking(move || {
        finish(&dir, after, &moved);
        Ok(())
    })
    .await?;
    Ok(moves)
}

pub(super) async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> DriveResult<T> + Send + 'static,
) -> DriveResult<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| DriveError::io(e.to_string()))?
}

/// No two entries go to one name, and nothing is at any destination except,
/// on a case-insensitive volume, the entry itself (a case-only rename).
fn check_free(plans: &[Plan]) -> DriveResult<()> {
    for (i, plan) in plans.iter().enumerate() {
        if plans[..i]
            .iter()
            .any(|p| p.to.eq_ignore_ascii_case(&plan.to))
        {
            return Err(DriveError::exists(&plan.to));
        }
    }
    Ok(())
}

fn taken(at: &Path, by: &Path) -> bool {
    std::fs::symlink_metadata(at).is_ok() && !crate::library::files::documents::same_entry(by, at)
}

/// Move every planned entry; returns the rows' moves and the last step's
/// work. Every clash is checked before anything moves.
fn on_disk(
    data_dir: &Path,
    plans: &[Plan],
    journal: &mut Journal,
) -> DriveResult<(Vec<RowMove>, Vec<After>)> {
    let plans: Vec<&Plan> = plans.iter().filter(|p| p.to != p.entry.rel).collect();
    for plan in &plans {
        let to = data_dir.join(&plan.to);
        if taken(&to, &plan.entry.path) {
            return Err(DriveError::exists(&plan.to));
        }
        if plan.entry.dir {
            continue;
        }
        for c in companions(data_dir, &plan.entry.rel, &plan.to) {
            if let Some(dest) = &c.to {
                if !c.hidden && taken(dest, &c.from) {
                    let rel = dest.strip_prefix(data_dir).unwrap_or(dest);
                    return Err(DriveError::exists(&rel.to_string_lossy()));
                }
            }
        }
    }
    let mut row_moves = Vec::new();
    let mut after = Vec::new();
    for plan in plans {
        let to = data_dir.join(&plan.to);
        if plan.entry.dir {
            journal.rename(&plan.entry.path, &to)?;
            if let (Some(from), Some(dest)) = (
                derived_dir(data_dir, &plan.entry.rel),
                derived_dir(data_dir, &plan.to),
            ) {
                if from.is_dir() {
                    merge_move(journal, &from, &dest)?;
                }
            }
            row_moves.push(RowMove {
                from: plan.entry.rel.clone(),
                to: plan.to.clone(),
                dir: true,
                relink: None,
                drop_parse: false,
            });
            continue;
        }
        row_moves.push(move_file(data_dir, plan, journal, &mut after)?);
    }
    Ok((row_moves, after))
}

fn move_file(
    data_dir: &Path,
    plan: &Plan,
    journal: &mut Journal,
    after: &mut Vec<After>,
) -> DriveResult<RowMove> {
    let (from_rel, to_rel) = (plan.entry.rel.as_str(), plan.to.as_str());
    let note = pictures::is_note(data_dir, from_rel);
    let parts = companions(data_dir, from_rel, to_rel);
    let pdf = |rel: &str| doc_pdf_rel(rel).map(|p| data_dir.join(p));
    let parsed = parts
        .iter()
        .any(|c| c.slot == ArtifactSlot::Derived(DerivedKind::Pages));
    let relink = match (pdf(from_rel), pdf(to_rel)) {
        (Some(old), Some(new)) if parsed => {
            let (old_prefix, new_prefix) = (images_prefix(&old), images_prefix(&new));
            (old_prefix != new_prefix).then(|| {
                after.push(After::Relink {
                    record: crate::library::layout::derived(&new, DerivedKind::Pages),
                    markdown: crate::library::layout::derived(&new, DerivedKind::Markdown),
                    old: old_prefix.clone(),
                    new: new_prefix.clone(),
                    pdf_name: new
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                });
                (old_prefix, new_prefix)
            })
        }
        _ => None,
    };
    let drop_parse = parts.iter().any(|c| c.to.is_none());
    journal.rename(&plan.entry.path, &data_dir.join(to_rel))?;
    for c in parts {
        match c.to {
            Some(dest) => {
                if c.hidden {
                    merge_move(journal, &c.from, &dest)?;
                } else {
                    journal.rename(&c.from, &dest)?;
                }
            }
            None => after.push(After::Remove(c.from)),
        }
    }
    if note {
        after.push(After::Pictures {
            from: from_rel.to_string(),
            to: to_rel.to_string(),
        });
    }
    Ok(RowMove {
        from: from_rel.to_string(),
        to: to_rel.to_string(),
        dir: false,
        relink,
        drop_parse,
    })
}

/// The last step: none of it can be undone, so a failure is logged and the
/// move stands.
fn finish(data_dir: &Path, after: Vec<After>, moved: &[MovedRow]) {
    for step in after {
        let done = match &step {
            After::Relink {
                record,
                markdown,
                old,
                new,
                pdf_name,
            } => super::relink::relink_record(record, old, new, pdf_name)
                .and_then(|()| super::relink::relink_markdown(markdown, old, new)),
            After::Remove(path) => if path.is_dir() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            }
            .map_err(|e| e.to_string()),
            After::Pictures { from, to } => pictures::carry(data_dir, from, to, false),
        };
        if let Err(e) = done {
            elog!("[oculus] drive: after a move: {e}");
        }
    }
    for row in moved {
        if let Err(e) =
            crate::library::files::canvases::snapshots::retarget(data_dir, row.id, &row.rel)
        {
            elog!("[oculus] drive: page pictures of {}: {e}", row.rel);
        }
    }
}
