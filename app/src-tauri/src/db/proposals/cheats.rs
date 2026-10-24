use super::{inspect, journal::Journal, state, Proposal};
use serde_json::Value;
use sqlx::SqliteConnection;

pub(super) async fn create(
    conn: &mut SqliteConnection,
    proposal: &Proposal,
    value: &Value,
    journal: &mut Journal,
) -> Result<(), String> {
    for note in state::rows(value, "notes")? {
        if !state::included(note) {
            continue;
        }
        state::object(
            note,
            &[
                "title",
                "body",
                "subject",
                "tags",
                "source",
                "topics",
                "topic_ids",
            ],
        )?;
        let title = state::text(note, "title")?;
        let body = state::text(note, "body")?;
        let named = inspect::subject(conn, state::nullable_text(note, "subject")?.as_deref())
            .await?
            .map(|s| s.0);
        // An entry without a subject files under the thread's.
        let subject = match named {
            Some(id) => id,
            None => sqlx::query_scalar::<_, Option<i64>>(
                "SELECT subject_id FROM harness_threads WHERE id=?1",
            )
            .bind(proposal.thread_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| e.to_string())?
            .flatten()
            .ok_or("Each cheat sheet entry needs a subject")?,
        };
        let tags = state::strings(note, "tags")?;
        crate::db::revision::tags::check(&tags.iter().map(String::as_str).collect::<Vec<_>>())?;
        let source = state::nullable_text(note, "source")?
            .unwrap_or_else(|| format!("{}:{}", proposal.thread_id, proposal.item_id));
        let id=sqlx::query("INSERT INTO revision_notes (subject_id,title,body,tags,position,source_kind,source_ref) SELECT ?1,?2,?3,?4,COALESCE(MAX(position),-1)+1,'chat',?5 FROM revision_notes")
            .bind(subject).bind(title).bind(body).bind(serde_json::to_string(&tags).map_err(|e|e.to_string())?).bind(source).execute(&mut *conn).await.map_err(|e|e.to_string())?.last_insert_rowid();
        journal.created("cheat", id);
        let mut ids: Vec<i64> = match note.get("topic_ids").filter(|v| !v.is_null()) {
            Some(v) => {
                serde_json::from_value(v.clone()).map_err(|_| "topic_ids needs numeric ids")?
            }
            None => Vec::new(),
        };
        for name in state::strings(note, "topics")? {
            let id = topic_named(conn, subject, &name, journal).await?;
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        if !ids.is_empty() {
            crate::db::revision::update_on(
                conn,
                crate::db::revision::Entity::Note,
                id,
                &serde_json::json!({"topic_ids":ids}),
            )
            .await?;
        }
    }
    Ok(())
}

/// The subject's topic called `name`, created with the entries when it is new
/// so the card is the only write and Undo removes the topic too.
async fn topic_named(
    conn: &mut SqliteConnection,
    subject: i64,
    name: &str,
    journal: &mut Journal,
) -> Result<i64, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Topic names must not be empty".into());
    }
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM revision_topics WHERE subject_id=?1 AND name=?2 COLLATE NOCASE",
    )
    .bind(subject)
    .bind(name)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = sqlx::query("INSERT INTO revision_topics (subject_id,name) VALUES (?1,?2)")
        .bind(subject)
        .bind(name)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?
        .last_insert_rowid();
    journal.created("topic", id);
    Ok(id)
}
