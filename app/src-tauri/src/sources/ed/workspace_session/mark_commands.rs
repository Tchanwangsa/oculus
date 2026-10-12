//! Mark and the feedback panel's reads (`mark`). `ed_workspace_mark` is the
//! only caller of Ed's mark: see `mark`'s header.

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use super::commands::{session, EdWorkspaces};
use super::mark::history;
use super::mark::standing::{self, Standing};
use super::mark::submission::Submission;
use super::slide;
use crate::runtime::blocking;
use crate::sources::ed::Ed;

/// Submit the workspace. A real submission: the pane calls it only from
/// the Mark button, after its confirm dialogs.
#[tauri::command]
pub async fn ed_workspace_mark(app: AppHandle, challenge_id: i64) -> Result<Submission, String> {
    let session = session(&app, challenge_id)?;
    let place = session.place();
    blocking::run(move || session.mark(place)).await
}

/// The submissions this install made for the challenge, newest first.
#[tauri::command]
pub async fn ed_workspace_submissions(
    app: AppHandle,
    challenge_id: i64,
) -> Result<Vec<Submission>, String> {
    let ed = ed_for(&app, challenge_id);
    blocking::run(move || {
        history::submissions(&ed, &crate::library::paths::data_dir(), challenge_id)
    })
    .await
}

/// Score, status, attempts left and due date, read live from Ed.
#[tauri::command]
pub async fn ed_challenge_standing(app: AppHandle, challenge_id: i64) -> Result<Standing, String> {
    let open = session(&app, challenge_id).ok();
    let ed = ed_for(&app, challenge_id);
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

/// The open session's Ed client, else a fresh one: neither read needs the
/// workspace open.
fn ed_for(app: &AppHandle, challenge_id: i64) -> Arc<Ed> {
    app.state::<EdWorkspaces>()
        .sessions()
        .get(&challenge_id)
        .map(|s| s.core.ed.clone())
        .unwrap_or_else(|| Arc::new(Ed::open(&crate::library::paths::data_dir())))
}
