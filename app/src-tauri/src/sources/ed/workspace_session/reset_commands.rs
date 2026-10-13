//! Reset to scaffold's command (`reset`).

use tauri::AppHandle;

use super::commands::session;
use crate::runtime::blocking;

/// Put the challenge back to Ed's original files and make the library copy
/// match. Destructive: the pane calls it only after its confirm dialog. Its
/// place in the save queue is taken before any waiting.
#[tauri::command]
pub async fn ed_workspace_reset(app: AppHandle, challenge_id: i64) -> Result<(), String> {
    let session = session(&app, challenge_id)?;
    let place = session.place();
    blocking::run(move || session.reset(place)).await
}
