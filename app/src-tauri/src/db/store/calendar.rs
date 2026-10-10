use sqlx::SqlitePool;

/// Replace a subject's calendar rows with what Canvas just returned.
///
/// Delete-then-insert, not upsert, so a moved or cancelled class disappears.
/// The fetch is always the course's complete set.
pub async fn replace_calendar_events(
    pool: &SqlitePool,
    subject_id: i64,
    events: &[crate::sources::calendar::CalendarEvent],
) -> Result<(), String> {
    sqlx::query("DELETE FROM calendar_events WHERE subject_id = ?1")
        .bind(subject_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    for e in events {
        sqlx::query(
            r#"INSERT INTO calendar_events
                 (id, subject_id, kind, title, start_at, end_at, all_day,
                  location, url, description, synced_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))
               ON CONFLICT(id) DO UPDATE SET
                 subject_id  = excluded.subject_id,
                 kind        = excluded.kind,
                 title       = excluded.title,
                 start_at    = excluded.start_at,
                 end_at      = excluded.end_at,
                 all_day     = excluded.all_day,
                 location    = excluded.location,
                 url         = excluded.url,
                 description = excluded.description,
                 synced_at   = datetime('now')"#,
        )
        .bind(&e.id)
        .bind(subject_id)
        .bind(&e.kind)
        .bind(&e.title)
        .bind(&e.start_at)
        .bind(&e.end_at)
        .bind(e.all_day as i64)
        .bind(&e.location)
        .bind(&e.url)
        .bind(&e.description)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
