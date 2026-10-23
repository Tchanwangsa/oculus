use super::{inspect, state};
use serde_json::{json, Value};
use sqlx::SqliteConnection;
use std::path::{Path, PathBuf};

/// A newly reserved document. Only a failed apply removes it; an existing file
/// is never opened for writing or cleanup.
pub(super) struct NewNote {
    pub path: PathBuf,
    pub relative: String,
    pub body: String,
    pub row: Value,
}
impl NewNote {
    pub fn cleanup(&self) {
        if std::fs::read(&self.path).ok().as_deref() == Some(self.body.as_bytes()) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
pub(super) async fn create(
    conn: &mut SqliteConnection,
    root: &Path,
    v: &Value,
) -> Result<NewNote, String> {
    let code = state::text(v, "subject")?;
    let (subject, code) = inspect::subject(conn, Some(&code))
        .await?
        .ok_or("a note needs a subject")?;
    let title = state::text(v, "title")?;
    let body = v
        .get("body")
        .and_then(Value::as_str)
        .ok_or("a note needs its body")?
        .to_owned();
    let file = crate::library::files::documents::create_document_body_in(
        root,
        &code,
        &title,
        body.as_bytes(),
    )?;
    let mut note = NewNote {
        path: root.join(&file.relative_path),
        relative: file.relative_path,
        body,
        row: Value::Null,
    };
    let written=sqlx::query("INSERT INTO files (subject_id,filename,relative_path,file_type,size_bytes,category,first_seen_at) VALUES (?1,?2,?3,'md',?4,'document',datetime('now'))")
        .bind(subject).bind(file.filename).bind(&note.relative).bind(note.body.len() as i64).execute(&mut *conn).await;
    let result = match written {
        Ok(r) => r,
        Err(e) => {
            note.cleanup();
            return Err(e.to_string());
        }
    };
    let row = sqlx::query("SELECT * FROM files WHERE id=?1")
        .bind(result.last_insert_rowid())
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string());
    match row.and_then(|r| inspect::raw(&r)) {
        Ok(row) => note.row = row,
        Err(e) => {
            note.cleanup();
            return Err(e);
        }
    }
    Ok(note)
}
pub(super) fn value(note: &NewNote) -> Value {
    json!({"path":note.relative,"body":note.body,"row":comparable_row(note.row.clone()),"links":[],"pages":[],"versions":[]})
}

/// Opening the note records access without changing anything Undo would
/// discard. Content, identity, path and other metadata still guard the write.
fn comparable_row(mut row: Value) -> Value {
    if let Some(object) = row.as_object_mut() {
        object.remove("last_accessed_at");
    }
    row
}

pub(super) async fn snapshot(
    conn: &mut SqliteConnection,
    root: &Path,
    path: &str,
) -> Result<Value, String> {
    if !crate::library::paths::is_document_rel(path) {
        return Err("invalid note path".into());
    }
    let rows = sqlx::query("SELECT * FROM files WHERE relative_path=?1")
        .bind(path)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    if rows.len() != 1 {
        return Err("the note's library row changed since this proposal".into());
    }
    let body = std::fs::read_to_string(root.join(path))
        .map_err(|e| format!("the note changed since this proposal: {e}"))?;
    let row = comparable_row(inspect::raw(&rows[0])?);
    let id = row["id"].as_i64().ok_or("invalid note row")?;
    let links = inspect::raw_rows(
        conn,
        "SELECT task_id FROM task_files WHERE file_id=?1 ORDER BY task_id",
        id,
    )
    .await?;
    let pages = inspect::raw_rows(
        conn,
        "SELECT id FROM pages WHERE file_id=?1 ORDER BY id",
        id,
    )
    .await?;
    let versions = inspect::raw_rows(
        conn,
        "SELECT id FROM document_versions WHERE file_id=?1 ORDER BY id",
        id,
    )
    .await?;
    Ok(json!({"path":path,"body":body,"row":row,"links":links,"pages":pages,"versions":versions}))
}

pub(super) struct RemovedNote {
    original: PathBuf,
    staged: PathBuf,
}
impl RemovedNote {
    pub fn rollback(self) -> Result<(), String> {
        // Linking with create semantics refuses to overwrite a note recreated
        // while the database transaction was finishing.
        std::fs::hard_link(&self.staged, &self.original)
            .map_err(|e| format!("restore note from {}: {e}", self.staged.display()))?;
        std::fs::remove_file(self.staged).map_err(|e| e.to_string())
    }
    pub fn finish(self) {
        let _ = std::fs::remove_file(self.staged);
    }
}
pub(super) async fn remove(
    conn: &mut SqliteConnection,
    root: &Path,
    path: &str,
    after: &Value,
) -> Result<RemovedNote, String> {
    if snapshot(conn, root, path).await? != *after {
        return Err("the note changed since this proposal; Undo would discard your edits".into());
    }
    let original = root.join(path);
    let staged = crate::library::files::documents::note_temp_path(&original)?;
    std::fs::rename(&original, &staged).map_err(|e| e.to_string())?;
    let removed = RemovedNote { original, staged };
    if std::fs::read(&removed.staged).ok().as_deref() != after["body"].as_str().map(str::as_bytes) {
        removed.rollback()?;
        return Err("the note changed while Undo was starting".into());
    }
    let id = after["row"]["id"]
        .as_i64()
        .ok_or("invalid note journal id")?;
    let result = async {
        for (table, column) in [
            ("pages", "file_id"),
            ("document_versions", "file_id"),
            ("files", "id"),
        ] {
            sqlx::query(&format!("DELETE FROM {table} WHERE {column}=?1"))
                .bind(id)
                .execute(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok::<_, String>(())
    }
    .await;
    if let Err(e) = result {
        removed.rollback()?;
        return Err(e);
    }
    Ok(removed)
}
