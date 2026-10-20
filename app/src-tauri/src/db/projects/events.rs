//! Calendar ids a project can pin, matching the non-task layers in loadCalendar.

use serde::Serialize;
use sqlx::{Row, SqliteConnection, SqlitePool};

const EVENTS: &str = r#"
WITH events AS (
    SELECT e.id, e.subject_id, s.code AS subject_code, e.kind, e.title, e.start_at
      FROM calendar_events e JOIN subjects s ON s.id = e.subject_id
     WHERE julianday(e.start_at) IS NOT NULL
    UNION ALL
    SELECT 'local_' || e.id, e.subject_id, s.code, e.kind, e.title, e.start_at
      FROM local_events e LEFT JOIN subjects s ON s.id = e.subject_id
     WHERE julianday(e.start_at) IS NOT NULL
    UNION ALL
    SELECT 'lecture_' || l.id, l.subject_id, s.code, 'lecture', l.title, l.date
      FROM lectures l JOIN subjects s ON s.id = l.subject_id
     WHERE julianday(l.date) IS NOT NULL
       AND NOT EXISTS (
           SELECT 1 FROM calendar_events e WHERE e.subject_id = l.subject_id
               AND e.kind = 'class' AND julianday(e.start_at) IS NOT NULL)
)
"#;

#[derive(Debug, Serialize)]
pub struct ProjectEvent {
    pub id: String,
    pub subject_id: Option<i64>,
    pub subject_code: Option<String>,
    pub kind: String,
    pub title: String,
    pub start_at: String,
}

pub async fn project_events(
    pool: &SqlitePool,
    subject_id: Option<i64>,
) -> Result<Vec<ProjectEvent>, String> {
    let rows = sqlx::query(&format!(
        "{EVENTS} SELECT * FROM events WHERE (?1 IS NULL OR subject_id = ?1)
         ORDER BY julianday(start_at), id"
    ))
    .bind(subject_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows
        .iter()
        .map(|r| ProjectEvent {
            id: r.get("id"),
            subject_id: r.get("subject_id"),
            subject_code: r.get("subject_code"),
            kind: r.get("kind"),
            title: r.get("title"),
            start_at: r.get("start_at"),
        })
        .collect())
}

pub(super) async fn require_event(conn: &mut SqliteConnection, id: &str) -> Result<(), String> {
    let exists: bool = sqlx::query_scalar(&format!(
        "{EVENTS} SELECT EXISTS(SELECT 1 FROM events WHERE id = ?1)"
    ))
    .bind(id)
    .fetch_one(conn)
    .await
    .map_err(|e| e.to_string())?;
    if !exists {
        return Err(format!(
            "no linkable calendar event has id \"{id}\" (oculus project events lists them)"
        ));
    }
    Ok(())
}
