//! The cheat sheet's tag registry (`revision_tags`, migration 52). Entries keep
//! tag names in `revision_notes.tags`; triggers register a new name, give an
//! entry the registry's spelling and follow renames and deletes. Colours are the
//! task palette's keys (`db::projects::TAG_COLORS`).
use serde_json::Value;
use sqlx::SqlitePool;

use crate::db::projects::normalise_tag_name;

/// Every tag in registry order, with how many entries carry it.
pub async fn list(pool: &SqlitePool) -> Result<Vec<Value>, String> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT json_object('id',g.id,'name',g.name,'color',g.color,'position',g.position,
                'uses',(SELECT COUNT(*) FROM revision_notes n, json_each(n.tags) j
                         WHERE j.value = g.name COLLATE NOCASE))
           FROM revision_tags g ORDER BY g.position, g.id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    rows.into_iter()
        .map(|v| serde_json::from_str(&v).map_err(|e| e.to_string()))
        .collect()
}

/// Refuses a tag that is empty or longer than a task tag may be, so a sentence
/// or a library path never becomes a registry row.
pub fn check(names: &[&str]) -> Result<(), String> {
    for name in names {
        let name = normalise_tag_name(name)?;
        if name.contains('/') {
            return Err(format!(
                "tag \"{name}\" looks like a path; the entry's source holds where it came from"
            ));
        }
    }
    Ok(())
}
