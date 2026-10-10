use sqlx::{Row, SqlitePool};

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
    let subject_id = match lecture_id {
        Some(id) => {
            sqlx::query_scalar::<_, Option<i64>>("SELECT subject_id FROM lectures WHERE id = ?1")
                .bind(id)
                .fetch_optional(pool)
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
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(res.last_insert_rowid())
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
    pub provider: Provider,
    pub provider_session_id: Option<String>,
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
        "SELECT t.id, t.provider, t.provider_session_id, t.model, s.code AS subject_code,
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
        provider_session_id: r.get("provider_session_id"),
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

pub async fn delete_thread(pool: &SqlitePool, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM harness_threads WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
