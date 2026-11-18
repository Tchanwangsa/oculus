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
use crate::library::migrate::rules::{Rule, Rules};

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
    rules: &Rules,
    linker: &Linker<'_>,
) -> Result<DbCounts, String> {
    let mut tx = pool.begin().await.map_err(err("begin"))?;
    let mut counts = DbCounts {
        files: files(&mut tx, &plan.rules).await?,
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
    counts.path_moves = path_moves(&mut tx, &plan.rules).await?;
    tx.commit().await.map_err(err("commit"))?;
    Ok(counts)
}

/// Each renamed drawing's row: its path, its name when it was the file's,
/// and the kind its new name gives it.
async fn files(tx: &mut Tx<'_>, rules: &[Rule]) -> Result<usize, String> {
    let mut changed = 0;
    for rule in rules {
        let row = sqlx::query("SELECT id, filename FROM files WHERE relative_path = ?1")
            .bind(&rule.old)
            .fetch_optional(&mut **tx)
            .await
            .map_err(err("read files"))?;
        let Some(row) = row else { continue };
        let (id, filename): (i64, String) = (row.get("id"), row.get("filename"));
        let old_base = rule.old.rsplit('/').next().unwrap_or_default();
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
    Ok(changed)
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

/// A rule per renamed drawing, and one for every older rule that led to it
/// (the Drive move's `canvases/a.canvas` → `a.canvas`), so an old path
/// resolves to the drawing in one step.
async fn path_moves(tx: &mut Tx<'_>, rules: &[Rule]) -> Result<usize, String> {
    let older: Vec<(String, String)> =
        sqlx::query_as("SELECT old_prefix, new_prefix FROM path_moves")
            .fetch_all(&mut **tx)
            .await
            .map_err(err("read path_moves"))?;
    let mut out: Vec<(String, String)> = older
        .iter()
        .flat_map(|(old, new)| rules.iter().filter_map(move |rule| through(old, new, rule)))
        .collect();
    out.extend(rules.iter().map(|r| (r.old.clone(), r.new.clone())));
    for (old, new) in &out {
        sqlx::query("INSERT OR REPLACE INTO path_moves (old_prefix, new_prefix) VALUES (?1, ?2)")
            .bind(old)
            .bind(new)
            .execute(&mut **tx)
            .await
            .map_err(err("path_moves"))?;
    }
    Ok(out.len())
}

/// The old path an older rule `old` → `new` sends to `rule.old`, mapped
/// straight to `rule.new`.
fn through(old: &str, new: &str, rule: &Rule) -> Option<(String, String)> {
    if rule.old == new {
        return Some((old.to_string(), rule.new.clone()));
    }
    let rest = rule.old.strip_prefix(new)?;
    rest.starts_with('/')
        .then(|| (format!("{old}{rest}"), rule.new.clone()))
}
