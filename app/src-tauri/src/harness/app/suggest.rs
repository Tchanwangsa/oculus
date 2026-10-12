use tauri::State;

use crate::harness::jobs;
use crate::runtime::blocking::run as blocking;

use super::HarnessState;

/// One inline completion for the document editor: the text to insert at
/// the caret between `before` and `after` in the note at `path`
/// (library-relative, for the prompt's title and subject). Empty for none,
/// and for a call a newer `request_id` or a cancel superseded.
#[tauri::command]
pub async fn document_suggest(
    state: State<'_, HarnessState>,
    request_id: u64,
    path: String,
    before: String,
    after: String,
) -> Result<String, String> {
    let pool = crate::db::store::open_pool().await?;
    let chain = jobs::chain(&pool, jobs::Job::DocumentSuggestions).await;
    let h = state.harness.clone();
    blocking(move || h.suggest(request_id, &chain, &path, &before, &after)).await
}

/// Stop the suggestion in flight, if any; its call answers empty.
#[tauri::command]
pub async fn document_suggest_cancel(state: State<'_, HarnessState>) -> Result<(), String> {
    let h = state.harness.clone();
    blocking(move || {
        h.cancel_suggestion();
        Ok(())
    })
    .await
}
