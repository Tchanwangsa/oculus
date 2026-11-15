use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::harness::event::Provider;

/// Start a thread. `lecture_id` is the recording a dock conversation is about.
/// A lecture thread's subject is read off the `lectures` row, never trusted
/// from the payload: a stale one would point at the wrong course folder.
pub async fn create_thread(
    pool: &SqlitePool,
    provider: Provider,
    model: Option<&str>,
    subject_id: Option<i64>,
    lecture_id: Option<&str>,
    first_message: &str,
) -> Result<i64, String> {
    let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;
    create(
        &mut conn,
        provider,
        model,
        subject_id,
        lecture_id,
        first_message,
    )
    .await
}

/// [`create_thread`] on the caller's connection, so a send that creates its
/// thread writes both in one transaction (`core/commit.rs`).
pub async fn create(
    conn: &mut SqliteConnection,
    provider: Provider,
    model: Option<&str>,
    subject_id: Option<i64>,
    lecture_id: Option<&str>,
    first_message: &str,
) -> Result<i64, String> {
    let subject_id = match lecture_id {
        Some(id) => {
            sqlx::query_scalar::<_, Option<i64>>("SELECT subject_id FROM lectures WHERE id = ?1")
                .bind(id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("no lecture {id}"))?
        }
        None => subject_id,
    };
    let title = title_from(first_message);
    let res = sqlx::query(
        "INSERT INTO harness_threads (provider, model, subject_id, lecture_id, title, status)
         VALUES (?1, ?2, ?3, ?4, ?5, 'idle')",
    )
    .bind(provider.as_str())
    .bind(model)
    .bind(subject_id)
    .bind(lecture_id)
    .bind(title)
    .execute(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;
    Ok(res.last_insert_rowid())
}

/// Count one more committed change to the thread's rows or runs, and return
/// the count. Every envelope carries it, and a reload reads it with the rows
/// (`core/load.rs`), so the webview can tell what a snapshot already holds.
pub async fn raise_revision(conn: &mut SqliteConnection, thread_id: i64) -> Result<i64, String> {
    sqlx::query_scalar(
        "UPDATE harness_threads SET revision = revision + 1 WHERE id = ?1 RETURNING revision",
    )
    .bind(thread_id)
    .fetch_optional(&mut *conn)
    .await
    .map(Option::unwrap_or_default)
    .map_err(|e| e.to_string())
}

/// The thread's revision as stored; 0 for a thread that is gone.
pub async fn revision(pool: &SqlitePool, thread_id: i64) -> Result<i64, String> {
    sqlx::query_scalar("SELECT revision FROM harness_threads WHERE id = ?1")
        .bind(thread_id)
        .fetch_optional(pool)
        .await
        .map(Option::unwrap_or_default)
        .map_err(|e| e.to_string())
}

/// The first line of the first message, clipped: the name until the naming
/// turn replaces it ([`claim_naming`]).
fn title_from(text: &str) -> String {
    let line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    let mut t: String = line.chars().take(72).collect();
    if line.chars().count() > 72 {
        t.push('…');
    }
    if t.is_empty() {
        "New thread".into()
    } else {
        t
    }
}

pub struct ThreadRow {
    pub id: i64,
    /// What the next send will use; a run records what it was sent with.
    pub provider: Provider,
    pub model: Option<String>,
    /// The subject's Canvas code, i.e. its `courses/` folder. Joined, not stored,
    /// so a rename cannot strand the thread; None is the general thread.
    pub subject_code: Option<String>,
    /// The dock conversation's recording, joined likewise. Deleting the
    /// recording clears the column, not the conversation.
    pub lecture: Option<LectureRef>,
}

/// What a lecture thread's instructions name: the recording and its transcript.
pub struct LectureRef {
    pub id: String,
    pub title: String,
    /// `YYYY-MM-DD`. The title is the timetable's (`… TU L105`), so the date is
    /// what says which lecture this is, and which week's slides go with it.
    pub date: String,
    /// Whether `transcript.vtt` is on disk: clearing transcripts leaves the
    /// column's path behind, and the agent must not be sent to a missing file.
    pub has_transcript: bool,
}

pub async fn thread(pool: &SqlitePool, id: i64) -> Result<ThreadRow, String> {
    let r = sqlx::query(
        "SELECT t.id, t.provider, t.model, s.code AS subject_code,
                l.id AS lecture_id, l.title AS lecture_title, l.date AS lecture_date,
                l.transcript_path
         FROM harness_threads t
         LEFT JOIN subjects s ON s.id = t.subject_id
         LEFT JOIN lectures l ON l.id = t.lecture_id
         WHERE t.id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("no thread {id}"))?;
    let provider: String = r.get("provider");
    let lecture = r
        .get::<Option<String>, _>("lecture_id")
        .map(|lid| LectureRef {
            id: lid,
            title: r.get("lecture_title"),
            date: r
                .get::<String, _>("lecture_date")
                .chars()
                .take(10)
                .collect(),
            has_transcript: r
                .get::<Option<String>, _>("transcript_path")
                .is_some_and(|p| std::path::Path::new(&p).exists()),
        });
    Ok(ThreadRow {
        id: r.get("id"),
        provider: Provider::parse(&provider)
            .ok_or_else(|| format!("unknown provider {provider}"))?,
        model: r.get("model"),
        subject_code: r.get("subject_code"),
        lecture,
    })
}

pub async fn set_model(pool: &SqlitePool, id: i64, model: Option<&str>) -> Result<(), String> {
    sqlx::query("UPDATE harness_threads SET model = ?2 WHERE id = ?1")
        .bind(id)
        .bind(model)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Stamps the moment the student had the thread on screen, to the
/// millisecond so it follows a run's `ended_at` in the same second. Never
/// `updated_at`, which orders the thread list. None when the thread is gone.
pub async fn mark_viewed(pool: &SqlitePool, id: i64) -> Result<Option<String>, String> {
    sqlx::query_scalar(
        "UPDATE harness_threads SET viewed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1 RETURNING viewed_at",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())
}

/// The thread is on screen in the front tab of a visible window: a run that
/// ended before now is read. Returns the stored `viewed_at`.
#[tauri::command]
pub async fn harness_thread_viewed(thread_id: i64) -> Result<Option<String>, String> {
    let pool = crate::db::store::open_pool().await?;
    mark_viewed(&pool, thread_id).await
}

/// Explicit dependent deletes, children first: a connection may lack `foreign_keys`.
pub async fn delete_thread(pool: &SqlitePool, id: i64) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    delete(&mut tx, id).await?;
    tx.commit().await.map_err(|e| e.to_string())
}

/// [`delete_thread`] inside the caller's transaction.
pub async fn delete(conn: &mut SqliteConnection, id: i64) -> Result<(), String> {
    for sql in [
        "DELETE FROM chat_proposals WHERE thread_id = ?1",
        "DELETE FROM harness_items WHERE thread_id = ?1",
        "DELETE FROM harness_command_receipts WHERE thread_id = ?1",
        "DELETE FROM harness_handoffs WHERE thread_id = ?1",
        "DELETE FROM harness_delivery_gaps WHERE thread_id = ?1",
        "DELETE FROM harness_stream_drafts WHERE run_id IN (SELECT id FROM harness_runs WHERE thread_id = ?1)",
        "DELETE FROM harness_attempts WHERE run_id IN (SELECT id FROM harness_runs WHERE thread_id = ?1)",
        "DELETE FROM harness_runs WHERE thread_id = ?1",
        "DELETE FROM harness_bindings WHERE thread_id = ?1",
        "DELETE FROM harness_threads WHERE id = ?1",
    ] {
        sqlx::query(sql)
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn viewing_stamps_viewed_at_and_leaves_updated_at() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for migration in crate::db::migrations::all() {
            sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
        }
        let id = create_thread(&pool, Provider::Claude, None, None, None, "hi")
            .await
            .unwrap();
        sqlx::query("UPDATE harness_threads SET updated_at = '2026-01-01 00:00:00'")
            .execute(&pool)
            .await
            .unwrap();
        let viewed = mark_viewed(&pool, id).await.unwrap().unwrap();
        // ISO with milliseconds: `2026-11-15T03:04:05.678Z`.
        assert_eq!(viewed.len(), 24, "{viewed}");
        let (stored, updated): (Option<String>, String) =
            sqlx::query_as("SELECT viewed_at, updated_at FROM harness_threads WHERE id = ?1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stored.as_deref(), Some(viewed.as_str()));
        assert_eq!(updated, "2026-01-01 00:00:00");
        assert_eq!(mark_viewed(&pool, id + 1).await.unwrap(), None);
    }

    #[test]
    fn titles_are_the_first_line_clipped() {
        assert_eq!(title_from("\n\n  Hello world  \nmore"), "Hello world");
        assert_eq!(title_from(""), "New thread");
        let long = "x".repeat(100);
        let t = title_from(&long);
        assert!(t.ends_with('…'));
        assert_eq!(t.chars().count(), 73);
    }
}
