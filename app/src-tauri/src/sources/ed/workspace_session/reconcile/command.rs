//! The pane's command for a conflict (`resolve`).

use tauri::AppHandle;

use super::resolve::Keep;
use crate::runtime::blocking;
use crate::sources::ed::workspace_session::commands::session;

/// Keep the local file (`mine`, uploaded while online) or take Ed's copy
/// (`ed`); either way the `<name>.ed-server.<ext>` copy is deleted.
#[tauri::command]
pub async fn ed_workspace_resolve(
    app: AppHandle,
    challenge_id: i64,
    path: String,
    keep: Keep,
) -> Result<(), String> {
    let session = session(&app, challenge_id)?;
    let place = session.place();
    blocking::run(move || session.resolve(place, &path, keep)).await
}
