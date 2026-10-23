//! App commands for durable proposal cards. The database owns validation and
//! transactions; the harness bus only broadcasts the committed result.

use crate::db::proposals::{self, Proposal};
use serde_json::{Map, Value};
use tauri::AppHandle;

async fn publish(app: &AppHandle, row: Proposal) -> Result<Proposal, String> {
    crate::harness::app::publish_proposal(app, row.clone()).await?;
    Ok(row)
}
#[tauri::command]
pub async fn proposal_open(
    app: AppHandle,
    thread_id: i64,
    item_id: i64,
    block: i64,
    kind: String,
    state: Value,
    amends: Option<String>,
) -> Result<Proposal, String> {
    let pool = crate::db::store::open_pool().await?;
    let row = proposals::open(
        &pool,
        thread_id,
        item_id,
        block,
        &kind,
        &state,
        amends.as_deref(),
    )
    .await?;
    // An amendment changes two cards; publish both committed rows.
    if let Some(previous) = amends {
        if let Ok(old) = proposals::get(&pool, &previous).await {
            if old.thread_id == thread_id {
                publish(&app, old).await?;
            }
        }
    }
    publish(&app, row).await
}
#[tauri::command]
pub async fn proposal_edit(app: AppHandle, id: String, state: Value) -> Result<Proposal, String> {
    let pool = crate::db::store::open_pool().await?;
    publish(&app, proposals::edit(&pool, &id, &state).await?).await
}
#[tauri::command]
pub async fn proposal_apply(app: AppHandle, id: String, state: Value) -> Result<Proposal, String> {
    let pool = crate::db::store::open_pool().await?;
    publish(&app, proposals::apply(&pool, &id, &state).await?).await
}
#[tauri::command]
pub async fn proposal_discard(app: AppHandle, id: String) -> Result<Proposal, String> {
    let pool = crate::db::store::open_pool().await?;
    publish(&app, proposals::discard(&pool, &id).await?).await
}
#[tauri::command]
pub async fn proposal_undo(app: AppHandle, id: String) -> Result<Proposal, String> {
    let pool = crate::db::store::open_pool().await?;
    publish(&app, proposals::undo(&pool, &id).await?).await
}
#[tauri::command]
pub async fn proposal_inspect(
    entity: String,
    ids: Vec<Value>,
) -> Result<Map<String, Value>, String> {
    let pool = crate::db::store::open_pool().await?;
    proposals::inspect(&pool, &entity, &ids).await
}
