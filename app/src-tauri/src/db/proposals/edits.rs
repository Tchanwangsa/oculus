use super::{inspect, journal::Journal, state};
use serde_json::Value;
use sqlx::SqliteConnection;

fn keys(entity: &str) -> Result<&'static [&'static str], String> {
    Ok(match entity {
        "cheat" => &[
            "title",
            "body",
            "subject_id",
            "tags",
            "position",
            "topic_ids",
        ],
        "question" => &[
            "subject_id",
            "kind",
            "prompt",
            "answer",
            "options",
            "tolerance",
            "rubric",
            "level",
            "topic",
            "style_ref",
            "status",
            "topic_ids",
        ],
        "quiz" => &["title", "description", "subject_id", "topic_ids"],
        "artifact" => &["title", "program", "state", "subject_id", "topic_ids"],
        "topic" => &["name", "description", "position"],
        _ => return Err("unknown editable revision entity".into()),
    })
}

/// Revision uses ordinary guarded field edits, including topic assignments.
pub(super) async fn run(
    conn: &mut SqliteConnection,
    value: &Value,
    journal: &mut Journal,
) -> Result<(), String> {
    for edit in state::rows(value, "changes")? {
        if !state::included(edit) {
            continue;
        }
        state::object(edit, &["entity", "id", "from", "to"])?;
        let entity = state::text(edit, "entity")?;
        let allowed = keys(&entity)?;
        let id = state::id(&edit["id"])?;
        let from = state::object(&edit["from"], allowed)?;
        let to = state::object(&edit["to"], allowed)?;
        if from.keys().ne(to.keys()) {
            return Err("from and to must contain the same fields".into());
        }
        let live = inspect::current(conn, &entity, &id.to_string()).await?;
        if live.is_null() {
            return Err(format!("{entity} {id} no longer exists"));
        }
        for (key, expected) in from {
            if live.get(key) != Some(expected) {
                return Err(format!("{entity} {id} changed since this proposal ({key})"));
            }
        }
        if to.is_empty() || to == from {
            continue;
        }
        journal.watch(conn, &entity, &id.to_string()).await?;
        if entity == "topic" {
            crate::db::revision::topics::update_on(conn, id, &edit["to"]).await?;
        } else {
            crate::db::revision::update_on(
                conn,
                inspect::revision_entity(&entity).ok_or("unknown revision entity")?,
                id,
                &edit["to"],
            )
            .await?;
        }
    }
    Ok(())
}
