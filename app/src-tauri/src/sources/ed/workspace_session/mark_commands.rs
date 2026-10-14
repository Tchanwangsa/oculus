//! Mark and the challenge's standing (`mark`). `ed_workspace_mark` is the
//! only caller of Ed's mark: see `mark`'s header.

use tauri::AppHandle;

use super::commands::session;
use super::mark::standing::{self, Standing};
use super::mark::submission::Submission;
use super::slide;
use super::submission_commands::ed_and_user;
use crate::runtime::blocking;

/// Submit the workspace. A real submission: the pane calls it only from
/// the Mark button, after its confirm dialogs.
#[tauri::command]
pub async fn ed_workspace_mark(app: AppHandle, challenge_id: i64) -> Result<Submission, String> {
    let session = session(&app, challenge_id)?;
    let place = session.place();
    blocking::run(move || session.mark(place)).await
}

/// Score, status, attempts left and due date, read live from Ed.
#[tauri::command]
pub async fn ed_challenge_standing(app: AppHandle, challenge_id: i64) -> Result<Standing, String> {
    let open = session(&app, challenge_id).ok();
    let (ed, _) = ed_and_user(&app, challenge_id);
    blocking::run(move || {
        let slide_due = match open {
            Some(session) => session.slide.due_at.clone(),
            None => slide::find(&crate::library::paths::data_dir(), challenge_id)
                .ok()
                .and_then(|s| s.due_at),
        };
        standing::standing(&ed, challenge_id, slide_due)
    })
    .await
}
