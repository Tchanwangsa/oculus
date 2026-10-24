use super::{get, inspect, journal, notes, state, Proposal};
use serde_json::Value;
use sqlx::SqlitePool;
use std::path::Path;

pub async fn apply(pool: &SqlitePool, id: &str, value: &Value) -> Result<Proposal, String> {
    apply_in(pool, &crate::library::paths::data_dir(), id, value).await
}
pub async fn apply_in(
    pool: &SqlitePool,
    root: &Path,
    id: &str,
    value: &Value,
) -> Result<Proposal, String> {
    let (item, block) = state::key(id)?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let changed=sqlx::query("UPDATE chat_proposals SET status='created',updated_at=datetime('now') WHERE item_id=?1 AND block=?2 AND status='pending' AND EXISTS (SELECT 1 FROM harness_items i JOIN harness_threads t ON t.id=i.thread_id WHERE i.id=chat_proposals.item_id AND i.thread_id=chat_proposals.thread_id)")
        .bind(item).bind(block).execute(&mut *tx).await.map_err(|e|e.to_string())?.rows_affected();
    if changed != 1 {
        return Err("this proposal is already decided or its reply was removed".into());
    }
    let row = state::on(&mut tx, id).await?;
    state::validate_shape(&row.kind, value)?;
    let mut created_note = None;
    let written = async {
        if row.kind == "NewNote" {
            let note = notes::create(&mut tx, root, value).await?;
            let snapshot = notes::value(&note);
            let path = note.relative.clone();
            created_note = Some(note);
            journal::record(
                &mut tx,
                row.thread_id,
                id,
                ("note", &path, "create"),
                &Value::Null,
                &snapshot,
            )
            .await?;
        } else {
            let mut journal = journal::Journal::default();
            if row.kind == "NewCheatNotes" {
                super::cheats::create(&mut tx, &row, value, &mut journal).await?;
            } else {
                super::mutations::run(&mut tx, &row.kind, value, &mut journal).await?;
            }
            if journal.finish(&mut tx, row.thread_id, id).await? == 0 {
                return Err("select at least one item that changes something".into());
            }
        }
        sqlx::query("UPDATE chat_proposals SET state=?1 WHERE item_id=?2 AND block=?3")
            .bind(value.to_string())
            .bind(item)
            .bind(block)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        Ok::<_, String>(())
    }
    .await;
    if let Err(e) = written {
        if let Some(note) = created_note {
            note.cleanup();
        }
        return Err(e);
    }
    if let Err(e) = tx.commit().await {
        if let Some(note) = created_note {
            note.cleanup();
        }
        return Err(e.to_string());
    }
    get(pool, id).await
}

pub async fn undo(pool: &SqlitePool, id: &str) -> Result<Proposal, String> {
    undo_in(pool, &crate::library::paths::data_dir(), id).await
}
pub async fn undo_in(pool: &SqlitePool, root: &Path, id: &str) -> Result<Proposal, String> {
    let (item, block) = state::key(id)?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let kind: String = sqlx::query_scalar(
        "SELECT kind FROM chat_proposals WHERE item_id=?1 AND block=?2 AND status='created'",
    )
    .bind(item)
    .bind(block)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| e.to_string())?
    .ok_or("only a created proposal can be undone")?;
    // Undoing cheat entries returns the card to its proposal, ready to Add again.
    let cheats = kind == "NewCheatNotes";
    sqlx::query("UPDATE chat_proposals SET status=?3,updated_at=datetime('now') WHERE item_id=?1 AND block=?2")
        .bind(item).bind(block).bind(if cheats { "pending" } else { "undone" })
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    let mut changes = journal::changes(&mut tx, id).await?;
    if changes.is_empty() {
        return Err("this proposal has no reversible changes".into());
    }
    // Check every row before restoring any: tag triggers and cascades can alter
    // the next row, but cannot make an unrelated live edit safe to overwrite.
    let mut gone = vec![];
    for change in &changes {
        let entity = change["entity"].as_str().ok_or("invalid journal entity")?;
        let entity_id = change["id"].as_str().ok_or("invalid journal id")?;
        let live = if entity == "note" {
            notes::snapshot(&mut tx, root, entity_id).await?
        } else {
            inspect::snapshot(&mut tx, entity, entity_id).await?
        };
        // An added cheat entry is removed as it is now, edits and all; one the
        // student already deleted has nothing left to undo.
        if cheats && entity == "cheat" && change["before"].is_null() {
            if live.is_null() {
                gone.push(entity_id.to_owned());
            }
            continue;
        }
        if live != change["after"] {
            return Err(format!(
                "{entity} {entity_id} changed since this proposal; Undo would discard your edits"
            ));
        }
    }
    changes.retain(|c| !(c["entity"] == "cheat" && gone.iter().any(|id| c["id"] == id.as_str())));
    let mut removed = vec![];
    let restored=async {
        // Remove creations in dependency order, then restore registry/project
        // rows before tasks and their links. The reverse journal order is kept
        // within each dependency class.
        for entity in ["note","cheat","question","quiz","artifact","topic","task","project","event","tag"] {
            for c in changes.iter().filter(|c|c["entity"]==entity&&c["before"].is_null()) {
                let id=c["id"].as_str().ok_or("invalid journal id")?;
                if entity=="note"{removed.push(notes::remove(&mut tx,root,id,&c["after"]).await?);}
                else {journal::restore(&mut tx,entity,id,&Value::Null).await?;}
            }
        }
        for entity in ["tag","project","event","task","topic","cheat","question","quiz","artifact"] {
            let mut restoring:Vec<_>=changes.iter().filter(|c|c["entity"]==entity&&!c["before"].is_null()).collect();
            if entity=="task"{restoring.sort_by_key(|c|(c["before"]["row"]["parent_id"].as_i64().is_some(),c["before"]["row"]["id"].as_i64()));}
            for c in restoring {journal::restore(&mut tx,entity,c["id"].as_str().ok_or("invalid journal id")?,&c["before"]).await?;}
        }
        sqlx::query("UPDATE agent_changes SET reverted_at=datetime('now') WHERE proposal_id=?1 AND reverted_at IS NULL").bind(id).execute(&mut *tx).await.map_err(|e|e.to_string())?;Ok::<_,String>(())
    }.await;
    if let Err(e) = restored {
        for n in removed {
            n.rollback()?;
        }
        return Err(e);
    }
    if let Err(e) = tx.commit().await {
        for n in removed {
            n.rollback()?;
        }
        return Err(e.to_string());
    }
    for n in removed {
        n.finish();
    }
    get(pool, id).await
}
