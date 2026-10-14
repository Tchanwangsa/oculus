//! The Feedback tab's commands: the student's submissions, a submission's
//! code, and Restore. The reads need no open workspace; Restore does.

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use super::commands::{session, EdWorkspaces};
use super::mark::code::{self, CodeFile};
use super::mark::history;
use super::mark::submission::Submission;
use crate::runtime::blocking;
use crate::sources::ed::Ed;

/// The student's submissions for the challenge, newest first, as Ed lists
/// them.
#[tauri::command]
pub async fn ed_workspace_submissions(
    app: AppHandle,
    challenge_id: i64,
) -> Result<Vec<Submission>, String> {
    let (ed, user_id) = ed_and_user(&app, challenge_id);
    blocking::run(move || {
        let user_id = match user_id {
            Some(id) => id,
            None => ed.user_id()?,
        };
        history::submissions(&ed, user_id, challenge_id)
    })
    .await
}

/// A submission's files, as text, for the read-only Code view.
#[tauri::command]
pub async fn ed_submission_files(
    app: AppHandle,
    challenge_id: i64,
    submission_id: i64,
) -> Result<Vec<CodeFile>, String> {
    let (ed, _) = ed_and_user(&app, challenge_id);
    blocking::run(move || code::files(&ed, submission_id).map(|files| code::as_text(&files))).await
}

/// Replace the workspace with a submission's code. Destructive: the pane
/// calls it only after its confirm dialog.
#[tauri::command]
pub async fn ed_workspace_restore(
    app: AppHandle,
    challenge_id: i64,
    submission_id: i64,
) -> Result<(), String> {
    let session = session(&app, challenge_id)?;
    let place = session.place();
    blocking::run(move || session.restore(place, submission_id)).await
}

/// The open session's Ed client and the user id its connect named, else a
/// fresh client: the reads do not need the workspace open.
pub(super) fn ed_and_user(app: &AppHandle, challenge_id: i64) -> (Arc<Ed>, Option<i64>) {
    match app.state::<EdWorkspaces>().sessions().get(&challenge_id) {
        Some(s) => (s.core.ed.clone(), s.core.shared().user_id),
        None => (Arc::new(Ed::open(&crate::library::paths::data_dir())), None),
    }
}
