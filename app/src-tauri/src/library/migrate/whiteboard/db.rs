//! The database's side of the rename, in one transaction: each drawing's
//! `files` row (path, name, `file_type`, `category`), the other columns and
//! JSON that hold library paths, the saved versions of notes that link to a
//! drawing, and `path_moves`. Every write maps an old path to its new one or
//! sets a value outright, so a run resumed after the commit changes nothing.

use sqlx::{Row, SqlitePool};

use super::journal::Plan;
use super::{LEGACY_CATEGORY, LEGACY_EXTENSION};
use crate::library::files::whiteboards::DrawingKind;
use crate::library::layout;
use crate::library::migrate::db::{column, json_column, Tx};
use crate::library::migrate::links::Linker;
use crate::library::migrate::plan::name_key;
use crate::library::migrate::rules::{Rule, Rules};
use crate::library::migrate::{record_moves, OwnRule};

/// The rows the plan reads.
#[derive(Default)]
pub struct Rows {
    /// Rows that may be drawings: `(id, path, category)`.
    pub drawings: Vec<(i64, String, Option<String>)>,
    /// Paths of rows already named as whiteboards, which a rename must not
    /// take.
    pub taken: Vec<String>,
}

pub async fn rows(pool: &SqlitePool) -> Result<Rows, String> {
    let drawings = sqlx::query_as(
        "SELECT id, relative_path, category FROM files
          WHERE category = ?1 OR relative_path LIKE '%' || ?2 ORDER BY id",
    )
    .bind(LEGACY_CATEGORY)
    .bind(LEGACY_EXTENSION)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("read files: {e}"))?;
    let taken = sqlx::query_scalar("SELECT relative_path FROM files WHERE relative_path LIKE ?1")
        .bind(format!("%{}", DrawingKind::Whiteboard.extension()))
        .fetch_all(pool)
        .await
        .map_err(|e| format!("read files: {e}"))?;
    Ok(Rows { drawings, taken })
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DbCounts {
    pub files: usize,
    pub other: usize,
    pub versions: usize,
    pub path_moves: usize,
}

fn err(what: &str) -> impl Fn(sqlx::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

pub async fn rewrite(
    pool: &SqlitePool,
    plan: &Plan,
    linker: &Linker<'_>,
) -> Result<DbCounts, String> {
    let mut tx = pool.begin().await.map_err(err("begin"))?;
    let (files, spellings) = files(&mut tx, &plan.rules).await?;
    let all: Vec<Rule> = plan.rules.iter().cloned().chain(spellings).collect();
    let rules = &Rules::new(&all);
    let mut counts = DbCounts {
        files,
        ..DbCounts::default()
    };
    recategorize(&mut tx).await?;
    for (table, col) in [
        ("inbox_item_entries", "relative_path"),
        ("revision_notes", "source_ref"),
    ] {
        counts.other += column(&mut tx, rules, table, col).await?;
    }
    for (table, key, cols) in [
        ("chat_proposals", "rowid", ["state", "initial_state"]),
        ("agent_changes", "id", ["before", "after"]),
    ] {
        for col in cols {
            counts.other += json_column(&mut tx, rules, table, key, col).await?;
        }
    }
    counts.versions = versions(&mut tx, linker).await?;
    counts.path_moves = path_moves(&mut tx, &all).await?;
    tx.commit().await.map_err(err("commit"))?;
    Ok(counts)
}

/// Each renamed drawing's row: its path, its name when it was the file's,
/// and the kind its new name gives it. A row is found as the volume finds
/// the file (`drawing_row`); one spelled other than the disk comes back as a
/// rule of its own, so what holds its spelling follows too.
async fn files(tx: &mut Tx<'_>, rules: &[Rule]) -> Result<(usize, Vec<Rule>), String> {
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, relative_path, filename FROM files
          WHERE category = ?1 OR relative_path LIKE '%' || ?2",
    )
    .bind(LEGACY_CATEGORY)
    .bind(LEGACY_EXTENSION)
    .fetch_all(&mut **tx)
    .await
    .map_err(err("read files"))?;
    let mut changed = 0;
    let mut spellings = Vec::new();
    for rule in rules {
        let Some((id, path, filename)) = drawing_row(&rows, &rule.old) else {
            continue;
        };
        if *path != rule.old {
            spellings.push(Rule {
                old: path.clone(),
                new: rule.new.clone(),
            });
        }
        let (id, filename) = (*id, filename.clone());
        let old_base = path.rsplit('/').next().unwrap_or_default();
        let new_base = rule.new.rsplit('/').next().unwrap_or_default();
        let filename = if filename == old_base {
            new_base.to_string()
        } else {
            filename
        };
        let updated = sqlx::query(
            "UPDATE files SET relative_path = ?1, filename = ?2, file_type = ?3, category = ?4
              WHERE id = ?5",
        )
        .bind(&rule.new)
        .bind(&filename)
        .bind(DrawingKind::Whiteboard.file_type())
        .bind(layout::category_of(layout::within_course(&rule.new)))
        .bind(id)
        .execute(&mut **tx)
        .await;
        // The plan picks names no row holds; should one still clash, that
        // row keeps its old path rather than fail every launch from now on.
        match updated {
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
                elog!(
                    "[oculus] whiteboard rename: files row {id} stays at {}: {} is another row's",
                    rule.old,
                    rule.new
                );
                continue;
            }
            Err(e) => return Err(format!("update files row {id} ({}): {e}", rule.old)),
            Ok(_) => changed += 1,
        }
    }
    Ok((changed, spellings))
}

/// The row for the file at `rel`: by exact spelling, else the one row with
/// its `name_key` (one Unicode form, no case).
fn drawing_row<'r>(
    rows: &'r [(i64, String, String)],
    rel: &str,
) -> Option<&'r (i64, String, String)> {
    if let Some(row) = rows.iter().find(|(_, path, _)| path == rel) {
        return Some(row);
    }
    let key = name_key(rel);
    let mut folded = rows.iter().filter(|(_, path, _)| name_key(path) == key);
    let row = folded.next()?;
    folded.next().is_none().then_some(row)
}

/// A row still saying `canvas` is a `.canvas` from another app, now an
/// upload like any other file: its category is what its path gives it.
async fn recategorize(tx: &mut Tx<'_>) -> Result<(), String> {
    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, relative_path FROM files WHERE category = ?1")
            .bind(LEGACY_CATEGORY)
            .fetch_all(&mut **tx)
            .await
            .map_err(err("read files"))?;
    for (id, rel) in rows {
        sqlx::query("UPDATE files SET category = ?1 WHERE id = ?2")
            .bind(layout::category_of(layout::within_course(&rel)))
            .bind(id)
            .execute(&mut **tx)
            .await
            .map_err(err("update files"))?;
    }
    Ok(())
}

/// Saved versions of notes that link to a drawing, re-pointed as the notes
/// on disk are (`contents`).
async fn versions(tx: &mut Tx<'_>, linker: &Linker<'_>) -> Result<usize, String> {
    let rows = sqlx::query(
        "SELECT v.id, v.text, f.relative_path FROM document_versions v
           JOIN files f ON f.id = v.file_id
          WHERE instr(v.text, ?1) > 0",
    )
    .bind(LEGACY_EXTENSION)
    .fetch_all(&mut **tx)
    .await
    .map_err(err("read document_versions"))?;
    let mut changed = 0;
    for row in rows {
        let (id, text, rel): (i64, String, String) =
            (row.get("id"), row.get("text"), row.get("relative_path"));
        let (relinked, n) = linker.relink(&text, &rel, &rel);
        if n == 0 {
            continue;
        }
        sqlx::query("UPDATE document_versions SET text = ?1 WHERE id = ?2")
            .bind(relinked)
            .bind(id)
            .execute(&mut **tx)
            .await
            .map_err(err("update document_versions"))?;
        changed += 1;
    }
    Ok(changed)
}

/// A rule per renamed drawing, and one for every older path that led to it
/// (the Drive move's `canvases/a.canvas` → `a.canvas`), so an old path
/// resolves to the drawing in one step (`record_moves`).
async fn path_moves(tx: &mut Tx<'_>, rules: &[Rule]) -> Result<usize, String> {
    let renames: Vec<(Rule, OwnRule)> = rules
        .iter()
        .map(|rule| (rule.clone(), OwnRule::Replace))
        .collect();
    record_moves(tx, &renames).await
}
