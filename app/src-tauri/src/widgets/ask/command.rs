use serde_json::Value;
use tauri::State;

use crate::harness::app::HarnessState;
use crate::harness::jobs::{self, Job};
use crate::harness::opencode;
use crate::runtime::blocking::run as blocking;

use super::prompt::{check_sizes, request, INSTRUCTIONS};
use super::reply::answer;
use super::schema::Schema;

/// What a stopped request rejects with.
const STOPPED: &str = "The request was stopped.";

/// One answer for a widget's `ask`: `prompt` and `schema` as the widget sent
/// them, `request_id` the host's own id for [`widget_ask_cancel`]. A schema
/// outside the subset, an oversized request and a job with no model are
/// refused before any turn.
#[tauri::command]
pub async fn widget_ask(
    state: State<'_, HarnessState>,
    request_id: u64,
    prompt: String,
    schema: Value,
) -> Result<Value, String> {
    check_sizes(&prompt, &schema)?;
    let parsed = Schema::parse(&schema)?;
    let text = request(&prompt, &schema);
    let pool = crate::db::store::open_pool().await?;
    let chain = jobs::chain(&pool, Job::WidgetAsk).await;
    let harness = state.harness.clone();
    blocking(move || {
        let scope = harness.stop_scope(request_id)?;
        let answered = jobs::try_each(Job::WidgetAsk, &chain, |sel| {
            answer(
                |turn| scope.one_turn(sel, INSTRUCTIONS, opencode::WIDGET_AGENT, turn),
                &text,
                &parsed,
            )
        })?;
        answered.ok_or_else(|| STOPPED.to_string())
    })
    .await
}

/// Stop `request_id`'s turn, if it is still running; its call rejects with
/// "The request was stopped." and no fallback model starts.
#[tauri::command]
pub async fn widget_ask_cancel(
    state: State<'_, HarnessState>,
    request_id: u64,
) -> Result<(), String> {
    let h = state.harness.clone();
    blocking(move || {
        h.stop_one_off(request_id);
        Ok(())
    })
    .await
}
