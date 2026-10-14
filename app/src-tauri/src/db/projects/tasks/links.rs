//! Task → library file links (`task_files`, migration 47), addressed by the
//! file's `relative_path` — what `oculus files` prints.

use sqlx::{Row, SqliteConnection};

/// The `files` row a `relative_path` names. The column is unique only together
/// with `subject_id`, so a path two subjects share is refused and lists them
/// rather than guessing; one that matches nothing says so.
async fn file_id_on(conn: &mut SqliteConnection, path: &str) -> Result<i64, String> {
    let rows = sqlx::query(
        "SELECT f.id, s.code FROM files f JOIN subjects s ON s.id = f.subject_id
          WHERE f.relative_path = ?1 ORDER BY s.code ASC",
    )
    .bind(path)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;
    match rows.as_slice() {
        [] => Err(format!(
            "no library file has the path \"{path}\" (oculus files lists them)"
        )),
        [one] => Ok(one.get("id")),
        many => {
            let subjects: Vec<String> = many.iter().map(|r| r.get("code")).collect();
            Err(format!(
                "the path \"{path}\" is in {} subjects ({}) and cannot name one file",
                many.len(),
                subjects.join(", ")
            ))
        }
    }
}

/// Every path to its file id, in order and without repeats. One bad path
/// refuses them all.
pub(in crate::db::projects) async fn resolve_files_on(
    conn: &mut SqliteConnection,
    paths: &[String],
) -> Result<Vec<i64>, String> {
    let mut ids: Vec<i64> = Vec::with_capacity(paths.len());
    for path in paths {
        let id = file_id_on(conn, path.trim()).await?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// Link files to a task; one already linked is left alone. Returns whether
/// anything changed, which decides whether `updated_at` moves.
pub(in crate::db::projects) async fn attach_files_on(
    conn: &mut SqliteConnection,
    task_id: i64,
    file_ids: &[i64],
) -> Result<bool, String> {
    let mut changed = false;
    for file_id in file_ids {
        let added =
            sqlx::query("INSERT OR IGNORE INTO task_files (task_id, file_id) VALUES (?1, ?2)")
                .bind(task_id)
                .bind(file_id)
                .execute(&mut *conn)
                .await
                .map_err(|e| e.to_string())?
                .rows_affected();
        changed |= added > 0;
    }
    Ok(changed)
}

/// Unlink files from a task; one that was not linked is a no-op.
pub(in crate::db::projects) async fn detach_files_on(
    conn: &mut SqliteConnection,
    task_id: i64,
    file_ids: &[i64],
) -> Result<bool, String> {
    let mut changed = false;
    for file_id in file_ids {
        let removed = sqlx::query("DELETE FROM task_files WHERE task_id = ?1 AND file_id = ?2")
            .bind(task_id)
            .bind(file_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?
            .rows_affected();
        changed |= removed > 0;
    }
    Ok(changed)
}
