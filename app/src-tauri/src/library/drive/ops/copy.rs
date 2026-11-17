//! `copy_to_own`: synced (or own) files and folders copied into one of the
//! student's folders. Each copy is a new file with its own row, written
//! before the command returns so the frontend can start its parse at once;
//! parse artifacts are not copied (the copy is parsed as itself), an Office
//! file's converted PDF is (it is the viewable copy). A note's pictures are
//! copied with it. A taken name steps aside to ` (2)`.

use std::collections::HashSet;
use std::path::Path;

use sqlx::SqlitePool;

use super::error::{shown, DriveError, DriveResult};
use super::guard::{readable_entry, target_folder, Entry, Target};
use super::journal::Journal;
use super::moves::blocking;
use super::names::free_name_where;
use super::pictures;
use super::rows::{self, NewRow};
use crate::library::layout;
use crate::library::paths::{artifact_slots, ArtifactSlot};

/// A folder copy stops this many folders down.
const MAX_DEPTH: usize = 32;

/// Copy `rels` into `target_dir`; returns the copies' paths in order.
pub async fn copy_to_own_at(
    data_dir: &Path,
    pool: &SqlitePool,
    rels: &[String],
    target_dir: &str,
) -> DriveResult<Vec<String>> {
    let target = target_folder(data_dir, target_dir)?;
    let subject = rows::subject_id(pool, target.subject_folder())
        .await?
        .ok_or_else(|| DriveError::not_found(target.subject_folder()))?;
    let mut entries = Vec::new();
    for rel in rels {
        let entry = readable_entry(data_dir, rel)?;
        if entry.dir && (target.rel == entry.rel || target.rel.starts_with(&format!("{rel}/"))) {
            return Err(DriveError::io(format!(
                "{} can't be copied into itself",
                shown(rel)
            )));
        }
        entries.push(entry);
    }
    let dir = data_dir.to_path_buf();
    let (journal, copies, new_rows, notes) =
        blocking(move || on_disk(&dir, &target, &entries)).await?;
    if let Err(e) = rows::insert_copies(pool, subject, &new_rows).await {
        blocking(move || {
            journal.undo();
            Ok(())
        })
        .await?;
        return Err(e);
    }
    let dir = data_dir.to_path_buf();
    blocking(move || {
        journal.commit();
        for (from, to) in notes {
            if let Err(e) = pictures::carry(&dir, &from, &to, true) {
                elog!("[oculus] drive: pictures of {to}: {e}");
            }
        }
        Ok(())
    })
    .await?;
    Ok(copies)
}

type Copied = (Journal, Vec<String>, Vec<NewRow>, Vec<(String, String)>);

fn on_disk(data_dir: &Path, target: &Target, entries: &[Entry]) -> DriveResult<Copied> {
    let mut journal = Journal::default();
    let mut copies = Vec::new();
    let mut new_rows = Vec::new();
    let mut notes = Vec::new();
    for entry in entries {
        let name = copy_name(data_dir, target, shown(&entry.rel), entry.dir);
        let name = if target.is_subject_root() && layout::reserved_name(&name) {
            copy_name(data_dir, target, &format!("{name} copy"), entry.dir)
        } else {
            name
        };
        let to_rel = target.child_rel(&name);
        let to = data_dir.join(&to_rel);
        let copied = if entry.dir {
            std::fs::create_dir(&to)
                .map_err(DriveError::from)
                .and_then(|()| {
                    journal.made(&to);
                    copy_folder(data_dir, &entry.rel, &to_rel, 0, &mut new_rows)
                })
        } else {
            copy_file(data_dir, &entry.rel, &to_rel, &mut journal, &mut new_rows).map(|()| {
                if pictures::is_note(data_dir, &to_rel) {
                    notes.push((entry.rel.clone(), to_rel.clone()));
                }
            })
        };
        if let Err(e) = copied {
            journal.undo();
            return Err(e);
        }
        copies.push(to_rel);
    }
    Ok((journal, copies, new_rows, notes))
}

/// A name in `target` free for the copy and, for a file, for its converted
/// PDF (`x.docx` → `x.docx.pdf`), so a copy never lands on anything.
fn copy_name(data_dir: &Path, target: &Target, name: &str, folder: bool) -> String {
    free_name_where(name, folder, |candidate| {
        let rel = target.child_rel(candidate);
        let taken = |path: &Path| std::fs::symlink_metadata(path).is_ok();
        !taken(&data_dir.join(&rel))
            && (folder
                || crate::library::paths::artifact_at(data_dir, &rel, ArtifactSlot::Converted)
                    .is_none_or(|pdf| !taken(&pdf)))
    })
}

/// `std::fs::copy`, refusing to replace anything at `to`.
fn copy_new(from: &Path, to: &Path) -> DriveResult<u64> {
    if std::fs::symlink_metadata(to).is_ok() {
        return Err(DriveError::exists(&to.to_string_lossy()));
    }
    Ok(std::fs::copy(from, to)?)
}

/// One file, and its converted PDF when it has one.
fn copy_file(
    data_dir: &Path,
    from_rel: &str,
    to_rel: &str,
    journal: &mut Journal,
    new_rows: &mut Vec<NewRow>,
) -> DriveResult<()> {
    let to = data_dir.join(to_rel);
    let size = copy_new(&data_dir.join(from_rel), &to)?;
    journal.made(&to);
    new_rows.push(NewRow {
        rel: to_rel.to_string(),
        size,
    });
    for (slot, from) in artifact_slots(data_dir, from_rel) {
        if slot != ArtifactSlot::Converted || !from.is_file() {
            continue;
        }
        if let Some(dest) = crate::library::paths::artifact_at(data_dir, to_rel, slot) {
            copy_new(&from, &dest)?;
            journal.made(&dest);
        }
    }
    Ok(())
}

/// Everything Drive lists in a folder, at any depth, plus its notes'
/// pictures folders; no symlink, dot-name or parse artifact. The folder
/// itself is already made and journaled, so its contents need no entries.
fn copy_folder(
    data_dir: &Path,
    from_rel: &str,
    to_rel: &str,
    depth: usize,
    new_rows: &mut Vec<NewRow>,
) -> DriveResult<()> {
    if depth >= MAX_DEPTH {
        return Ok(());
    }
    let mut children: Vec<(String, std::fs::FileType)> =
        std::fs::read_dir(data_dir.join(from_rel))?
            .flatten()
            .filter_map(|e| Some((e.file_name().into_string().ok()?, e.file_type().ok()?)))
            .filter(|(_, kind)| !kind.is_symlink())
            .collect();
    children.sort_by(|a, b| a.0.cmp(&b.0));
    let mut artifacts: HashSet<String> = HashSet::new();
    for (name, kind) in &children {
        if kind.is_file() {
            for (slot, path) in artifact_slots(data_dir, &format!("{from_rel}/{name}")) {
                if slot != ArtifactSlot::Converted {
                    if let Some(n) = path.file_name() {
                        artifacts.insert(n.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    for (name, kind) in children {
        let pictures = name == layout::note_assets_dir();
        if (name.starts_with('.') && !pictures) || artifacts.contains(&name) {
            continue;
        }
        let (from, to) = (format!("{from_rel}/{name}"), format!("{to_rel}/{name}"));
        if kind.is_dir() {
            std::fs::create_dir(data_dir.join(&to))?;
            if pictures {
                copy_flat(&data_dir.join(&from), &data_dir.join(&to))?;
            } else {
                copy_folder(data_dir, &from, &to, depth + 1, new_rows)?;
            }
        } else if kind.is_file() {
            let size = std::fs::copy(data_dir.join(&from), data_dir.join(&to))?;
            let converted = from_rel_is_converted(data_dir, &from);
            if !converted {
                new_rows.push(NewRow { rel: to, size });
            }
        }
    }
    Ok(())
}

/// True when the file at `rel` is a sibling Office file's converted PDF:
/// copied, but never a row of its own.
fn from_rel_is_converted(data_dir: &Path, rel: &str) -> bool {
    rel.strip_suffix(".pdf").is_some_and(|original| {
        crate::library::paths::doc_pdf_rel(original).as_deref() == Some(rel)
            && data_dir.join(original).is_file()
    })
}

/// A pictures folder's files.
fn copy_flat(from: &Path, to: &Path) -> DriveResult<()> {
    for entry in std::fs::read_dir(from)?.flatten() {
        if entry.file_type().is_ok_and(|t| t.is_file()) {
            std::fs::copy(entry.path(), to.join(entry.file_name()))?;
        }
    }
    Ok(())
}
