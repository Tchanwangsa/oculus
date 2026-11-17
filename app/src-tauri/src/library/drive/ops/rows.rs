//! The `files` rows that follow the disk. Rows are keyed by id and other
//! tables hang off the id (`pages`, `document_versions`, `task_files`), so a
//! move or rename updates a row in place, never deletes and re-inserts it.
//! Each command's rows change in one transaction.

use sqlx::{Row, SqliteConnection, SqlitePool};

use super::error::{DriveError, DriveResult};
use crate::library::layout;
use crate::library::paths::{fold_code, safe_dir};

fn db(e: sqlx::Error) -> DriveError {
    DriveError::io(format!("the database refused it — {e}"))
}

/// The subject whose folder is `folder` (`courses/<folder>`).
pub async fn subject_id(pool: &SqlitePool, folder: &str) -> DriveResult<Option<i64>> {
    let wanted = fold_code(folder);
    let rows = sqlx::query("SELECT id, code FROM subjects")
        .fetch_all(pool)
        .await
        .map_err(db)?;
    Ok(rows.iter().find_map(|r| {
        let code: String = r.get("code");
        (fold_code(&safe_dir(&code)) == wanted).then(|| r.get("id"))
    }))
}

/// The subjects whose folders are `folders`, skipping any without a row.
pub async fn subject_ids(pool: &SqlitePool, folders: &[String]) -> Vec<i64> {
    let mut ids = Vec::new();
    for folder in folders {
        if let Ok(Some(id)) = subject_id(pool, folder).await {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids
}

/// One entry moved on disk, for its rows to follow.
pub struct RowMove {
    pub from: String,
    pub to: String,
    pub dir: bool,
    /// A parsed file's picture links, `(old prefix, new prefix)`, when its
    /// stem changed: its pages' markdown is relinked too.
    pub relink: Option<(String, String)>,
    /// The new name parses as nothing (or differently): its pages go and its
    /// parse and embed state is cleared.
    pub drop_parse: bool,
}

/// A row after a move: what the post-move work needs.
pub struct MovedRow {
    pub id: i64,
    pub rel: String,
}

/// Rows at or below `rel`: `(id, relative_path)`.
async fn rows_at(
    conn: &mut SqliteConnection,
    rel: &str,
    dir: bool,
) -> DriveResult<Vec<(i64, String)>> {
    let rows = if dir {
        sqlx::query(
            "SELECT id, relative_path FROM files
             WHERE substr(relative_path, 1, length(?1) + 1) = ?1 || '/'",
        )
        .bind(rel)
        .fetch_all(&mut *conn)
        .await
    } else {
        sqlx::query("SELECT id, relative_path FROM files WHERE relative_path = ?1")
            .bind(rel)
            .fetch_all(&mut *conn)
            .await
    }
    .map_err(db)?;
    Ok(rows
        .iter()
        .map(|r| (r.get("id"), r.get("relative_path")))
        .collect())
}

/// A row and everything hung off its id. Children are deleted explicitly:
/// `ON DELETE CASCADE` fires only on a connection with `foreign_keys` on.
async fn delete_ids(conn: &mut SqliteConnection, ids: &[i64]) -> DriveResult<()> {
    for id in ids {
        for sql in [
            "DELETE FROM pages WHERE file_id = ?1",
            "DELETE FROM document_versions WHERE file_id = ?1",
            "DELETE FROM task_files WHERE file_id = ?1",
            "DELETE FROM files WHERE id = ?1",
        ] {
            sqlx::query(sql)
                .bind(id)
                .execute(&mut *conn)
                .await
                .map_err(db)?;
        }
    }
    Ok(())
}

/// A row left at `rel` by a file that is gone (the disk was checked free):
/// removed, or the moved row could not take its path.
async fn clear_stale(conn: &mut SqliteConnection, rel: &str, keep: i64) -> DriveResult<()> {
    let stale: Vec<i64> = rows_at(conn, rel, false)
        .await?
        .into_iter()
        .map(|(id, _)| id)
        .filter(|id| *id != keep)
        .collect();
    delete_ids(conn, &stale).await
}

fn file_type(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

fn name_of(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

/// Follow `moves` in the rows: path, name, kind (`category_of`, so `.md`
/// to `.txt` stops being a note) and, with `subject`, the subject. Returns
/// every row that moved.
pub async fn apply_moves(
    pool: &SqlitePool,
    moves: &[RowMove],
    subject: Option<i64>,
) -> DriveResult<Vec<MovedRow>> {
    let mut tx = pool.begin().await.map_err(db)?;
    let mut moved = Vec::new();
    for m in moves {
        for (id, rel) in rows_at(&mut tx, &m.from, m.dir).await? {
            let to = format!("{}{}", m.to, &rel[m.from.len()..]);
            clear_stale(&mut tx, &to, id).await?;
            let renamed = name_of(&rel) != name_of(&to);
            sqlx::query(
                "UPDATE files SET subject_id = COALESCE(?1, subject_id), relative_path = ?2,
                    filename = ?3, file_type = CASE WHEN ?4 THEN ?5 ELSE file_type END,
                    category = ?6, modified_at = datetime('now')
                 WHERE id = ?7",
            )
            .bind(subject)
            .bind(&to)
            .bind(name_of(&to))
            .bind(renamed)
            .bind(file_type(name_of(&to)))
            .bind(layout::category_of(layout::within_course(&to)))
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
            if let Some((old, new)) = &m.relink {
                sqlx::query(
                    "UPDATE pages SET markdown = replace(markdown, ?1, ?2) WHERE file_id = ?3",
                )
                .bind(format!("]({old}/"))
                .bind(format!("]({new}/"))
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(db)?;
            }
            if m.drop_parse {
                sqlx::query("DELETE FROM pages WHERE file_id = ?1")
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(db)?;
                sqlx::query(
                    "UPDATE files SET parse_status = NULL, parsed_at = NULL,
                        embed_status = NULL, embedded_at = NULL WHERE id = ?1",
                )
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(db)?;
            }
            moved.push(MovedRow { id, rel: to });
        }
    }
    tx.commit().await.map_err(db)?;
    Ok(moved)
}

/// Delete the rows at or below each `(rel, dir)`, with their children;
/// returns their ids.
pub async fn delete_rows(pool: &SqlitePool, entries: &[(String, bool)]) -> DriveResult<Vec<i64>> {
    let mut tx = pool.begin().await.map_err(db)?;
    let mut ids = Vec::new();
    for (rel, dir) in entries {
        ids.extend(
            rows_at(&mut tx, rel, *dir)
                .await?
                .into_iter()
                .map(|(id, _)| id),
        );
    }
    delete_ids(&mut tx, &ids).await?;
    tx.commit().await.map_err(db)?;
    Ok(ids)
}

/// A copied file, `(relative_path, size)`, for its own row.
pub struct NewRow {
    pub rel: String,
    pub size: u64,
}

/// Insert a row for each copy, in `subject`; a stale row on a copy's path
/// goes first.
pub async fn insert_copies(pool: &SqlitePool, subject: i64, rows: &[NewRow]) -> DriveResult<()> {
    let mut tx = pool.begin().await.map_err(db)?;
    for row in rows {
        clear_stale(&mut tx, &row.rel, -1).await?;
        let name = name_of(&row.rel);
        sqlx::query(
            "INSERT INTO files (subject_id, filename, relative_path, file_type, size_bytes,
                category, first_seen_at, modified_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'), datetime('now'))",
        )
        .bind(subject)
        .bind(name)
        .bind(&row.rel)
        .bind(file_type(name))
        .bind(row.size as i64)
        .bind(layout::category_of(layout::within_course(&row.rel)))
        .execute(&mut *tx)
        .await
        .map_err(db)?;
    }
    tx.commit().await.map_err(db)
}
