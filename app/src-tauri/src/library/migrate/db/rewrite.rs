//! Every stored path, rewritten in place in one transaction: rows are
//! UPDATEd, never deleted and re-inserted, since `files` cascades to its
//! pages, versions and task links. The `library_layout` setting is written
//! in the same transaction, so a resumed run knows whether this happened.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use crate::library::layout;
use crate::library::migrate::journal::{Plan, LAYOUT};
use crate::library::migrate::links::Linker;
use crate::library::migrate::rules::Rules;

/// The settings row that says the database is in the new layout.
const LAYOUT_SETTING: &str = "library_layout";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DbCounts {
    pub files: usize,
    pub run_files: usize,
    pub lectures: usize,
    pub other: usize,
    pub path_moves: usize,
    /// The database was already rewritten by an earlier run.
    pub already: bool,
}

pub(in crate::library::migrate) type Tx<'c> = Transaction<'c, Sqlite>;

fn err(what: &str) -> impl Fn(sqlx::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

pub async fn rewrite(
    pool: &SqlitePool,
    data_dir: &Path,
    plan: &Plan,
    linker: &Linker<'_>,
) -> Result<DbCounts, String> {
    let rules = Rules::new(&plan.rules);
    let mut tx = pool.begin().await.map_err(err("begin"))?;
    let mut counts = DbCounts::default();
    if crate::db::store::setting(&mut *tx, LAYOUT_SETTING)
        .await?
        .as_deref()
        == Some(LAYOUT)
    {
        counts.already = true;
        return Ok(counts);
    }
    let moved_notes = files(&mut tx, &rules, &mut counts).await?;
    counts.run_files = column(&mut tx, &rules, "sync_run_files", "relative_path").await?;
    for (table, col) in [
        ("echo_embeds", "file_rel"),
        ("echo_embeds", "poster_rel"),
        ("inbox_item_entries", "relative_path"),
        ("revision_notes", "source_ref"),
    ] {
        counts.other += column(&mut tx, &rules, table, col).await?;
    }
    for (table, key, cols) in [
        ("chat_proposals", "rowid", ["state", "initial_state"]),
        ("agent_changes", "id", ["before", "after"]),
    ] {
        for col in cols {
            counts.other += json_column(&mut tx, &rules, table, key, col).await?;
        }
    }
    counts.lectures = lectures(&mut tx, data_dir, plan).await?;
    counts.other += versions(&mut tx, &moved_notes, linker).await?;
    for rule in &plan.rules {
        sqlx::query("INSERT OR REPLACE INTO path_moves (old_prefix, new_prefix) VALUES (?1, ?2)")
            .bind(&rule.old)
            .bind(&rule.new)
            .execute(&mut *tx)
            .await
            .map_err(err("path_moves"))?;
        counts.path_moves += 1;
    }
    crate::db::store::set_setting(&mut *tx, LAYOUT_SETTING, LAYOUT).await?;
    tx.commit().await.map_err(err("commit"))?;
    Ok(counts)
}

/// `files.relative_path`, and the `category` its new place gives it; the
/// `filename` too when a clash renamed the file. Returns the moved
/// markdown rows (id → old, new) whose saved versions hold links.
async fn files(
    tx: &mut Tx<'_>,
    rules: &Rules,
    counts: &mut DbCounts,
) -> Result<HashMap<i64, (String, String)>, String> {
    let rows = sqlx::query("SELECT id, relative_path, filename FROM files")
        .fetch_all(&mut **tx)
        .await
        .map_err(err("read files"))?;
    let mut notes = HashMap::new();
    for row in rows {
        let (id, old, filename): (i64, String, String) =
            (row.get("id"), row.get("relative_path"), row.get("filename"));
        let Some(new) = rules.map(&old).filter(|n| *n != old) else {
            continue;
        };
        let category = layout::split_course(&new).map(|(_, inner)| layout::category_of(inner));
        let old_base = old.rsplit('/').next().unwrap_or_default();
        let new_base = new.rsplit('/').next().unwrap_or_default();
        let filename = if filename == old_base {
            new_base.to_string()
        } else {
            filename
        };
        let updated = sqlx::query(
            "UPDATE files SET relative_path = ?1, category = COALESCE(?2, category), filename = ?3
              WHERE id = ?4",
        )
        .bind(&new)
        .bind(category)
        .bind(&filename)
        .bind(id)
        .execute(&mut **tx)
        .await;
        // The plan gives clashing rows distinct paths (`plan::stale_rows`);
        // should one still clash, it keeps its old path rather than fail the
        // whole rewrite on every launch from now on.
        match updated {
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
                elog!(
                    "[oculus] library move: files row {id} stays at {old}: {new} is another row's"
                );
                continue;
            }
            Err(e) => return Err(format!("update files row {id} ({old} → {new}): {e}")),
            Ok(_) => {}
        }
        counts.files += 1;
        if new.to_ascii_lowercase().ends_with(".md") {
            notes.insert(id, (old, new));
        }
    }
    Ok(notes)
}

/// A path column, one UPDATE per distinct value that moved.
pub(in crate::library::migrate) async fn column(
    tx: &mut Tx<'_>,
    rules: &Rules,
    table: &str,
    col: &str,
) -> Result<usize, String> {
    let values: Vec<String> = sqlx::query_scalar(&format!(
        "SELECT DISTINCT {col} FROM {table} WHERE {col} IS NOT NULL"
    ))
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| format!("read {table}.{col}: {e}"))?;
    let mut changed = 0;
    for old in values {
        let Some(new) = rules.map(&old) else { continue };
        let done = sqlx::query(&format!("UPDATE {table} SET {col} = ?1 WHERE {col} = ?2"))
            .bind(&new)
            .bind(&old)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("update {table}.{col}: {e}"))?;
        changed += done.rows_affected() as usize;
    }
    Ok(changed)
}

/// Every string inside a JSON value that a rule covers, rewritten.
pub(in crate::library::migrate) fn remap_json(value: &mut Value, rules: &Rules) -> bool {
    match value {
        Value::String(s) => match rules.map(s) {
            Some(new) => {
                *s = new;
                true
            }
            None => false,
        },
        Value::Array(items) => {
            // Every element is visited: `any` would stop at the first change.
            let mut changed = false;
            for v in items {
                changed |= remap_json(v, rules);
            }
            changed
        }
        Value::Object(map) => {
            let mut changed = false;
            for v in map.values_mut() {
                changed |= remap_json(v, rules);
            }
            changed
        }
        _ => false,
    }
}

/// A JSON column (a proposal's state, an undo snapshot) with its paths.
pub(in crate::library::migrate) async fn json_column(
    tx: &mut Tx<'_>,
    rules: &Rules,
    table: &str,
    key: &str,
    col: &str,
) -> Result<usize, String> {
    let rows = sqlx::query(&format!(
        "SELECT {key} AS k, {col} AS v FROM {table} WHERE {col} IS NOT NULL"
    ))
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| format!("read {table}.{col}: {e}"))?;
    let mut changed = 0;
    for row in rows {
        let (k, raw): (i64, String) = (row.get("k"), row.get("v"));
        let Ok(mut value) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        if !remap_json(&mut value, rules) {
            continue;
        }
        sqlx::query(&format!("UPDATE {table} SET {col} = ?1 WHERE {key} = ?2"))
            .bind(value.to_string())
            .bind(k)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("update {table}.{col}: {e}"))?;
        changed += 1;
    }
    Ok(changed)
}

/// A lecture's folder, and its media paths moved into it. The stored paths
/// are absolute into wherever the library was when they were written, so
/// they are recognised by their `…/<recordings root>/<id>/<file>` tail and
/// joined to this data dir, never followed.
async fn lectures(tx: &mut Tx<'_>, data_dir: &Path, plan: &Plan) -> Result<usize, String> {
    let mut changed = 0;
    for lecture in &plan.lectures {
        let row = sqlx::query(
            "SELECT video_path, video2_path, transcript_path FROM lectures WHERE id = ?1",
        )
        .bind(&lecture.id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(err("read lectures"))?;
        let Some(row) = row else { continue };
        let moved = |col: &str| -> Option<String> {
            let stored: Option<String> = row.get(col);
            stored
                .map(|s| lecture_path(&s, &lecture.id, data_dir, &lecture.folder_rel).unwrap_or(s))
        };
        sqlx::query(
            "UPDATE lectures SET folder_rel = ?1, video_path = ?2, video2_path = ?3,
                    transcript_path = ?4
              WHERE id = ?5",
        )
        .bind(&lecture.folder_rel)
        .bind(moved("video_path"))
        .bind(moved("video2_path"))
        .bind(moved("transcript_path"))
        .bind(&lecture.id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("update lecture {}: {e}", lecture.id))?;
        changed += 1;
    }
    Ok(changed)
}

/// `stored`'s file inside the recording folder, as an absolute path in this
/// data dir; `None` when `stored` is not under `<recordings root>/<id>/`.
pub(in crate::library::migrate) fn lecture_path(
    stored: &str,
    id: &str,
    data_dir: &Path,
    folder_rel: &str,
) -> Option<String> {
    let slashed = stored.replace('\\', "/");
    let tail = format!("{}/{id}/", layout::legacy_recordings_root());
    let at = if slashed.starts_with(&tail) {
        0
    } else {
        slashed.find(&format!("/{tail}"))? + 1
    };
    let file = &slashed[at + tail.len()..];
    let mut path = data_dir.join(folder_rel);
    for part in file.split('/').filter(|p| !p.is_empty()) {
        path.push(part);
    }
    Some(path.to_string_lossy().into_owned())
}

/// A moved note's saved versions, their links re-pointed as the file's were.
async fn versions(
    tx: &mut Tx<'_>,
    notes: &HashMap<i64, (String, String)>,
    linker: &Linker<'_>,
) -> Result<usize, String> {
    let mut changed = 0;
    for (file_id, (old, new)) in notes {
        let rows = sqlx::query("SELECT id, text FROM document_versions WHERE file_id = ?1")
            .bind(file_id)
            .fetch_all(&mut **tx)
            .await
            .map_err(err("read document_versions"))?;
        for row in rows {
            let (id, text): (i64, String) = (row.get("id"), row.get("text"));
            let (relinked, n) = linker.relink(&text, old, new);
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
    }
    Ok(changed)
}
