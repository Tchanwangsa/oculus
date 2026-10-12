//! The workspace pane's commands, over the open sessions Tauri manages.
//! Every path is workspace-relative and goes through `paths::resolve`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, EventTarget, Manager};

use super::events::{Saved, Sink, WorkspaceInfo, EVENT};
use super::session::Session;
use super::slide;
use crate::runtime::blocking;
use crate::shell::browser::MAIN;
use crate::sources::ed::Ed;

/// One session per open challenge.
#[derive(Default)]
pub struct EdWorkspaces(Mutex<HashMap<i64, Arc<Session>>>);

impl EdWorkspaces {
    pub(super) fn sessions(&self) -> std::sync::MutexGuard<'_, HashMap<i64, Arc<Session>>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub(super) fn session(app: &AppHandle, challenge_id: i64) -> Result<Arc<Session>, String> {
    app.state::<EdWorkspaces>()
        .sessions()
        .get(&challenge_id)
        .cloned()
        .ok_or_else(|| format!("Ed challenge {challenge_id}'s workspace is not open"))
}

/// Open the challenge's workspace, or answer for the one already open. It
/// returns at once, `connecting`; the status events say the rest.
#[tauri::command]
pub async fn ed_workspace_open(app: AppHandle, challenge_id: i64) -> Result<WorkspaceInfo, String> {
    blocking::run(move || {
        let workspaces = app.state::<EdWorkspaces>();
        let mut sessions = workspaces.sessions();
        if let Some(open) = sessions.get(&challenge_id) {
            return Ok(open.info());
        }
        let data_dir = crate::library::paths::data_dir();
        let slide = slide::find(&data_dir, challenge_id)?;
        let ed = Arc::new(Ed::open(&data_dir));
        let emitter = app.clone();
        let sink: Sink = Arc::new(move |event| {
            let _ = emitter.emit_to(EventTarget::webview(MAIN), EVENT, event);
        });
        let session = Session::open(challenge_id, data_dir, slide, ed, sink)?;
        let info = session.info();
        sessions.insert(challenge_id, Arc::new(session));
        Ok(info)
    })
    .await
}

#[tauri::command]
pub async fn ed_workspace_close(app: AppHandle, challenge_id: i64) -> Result<(), String> {
    if let Some(session) = app.state::<EdWorkspaces>().sessions().remove(&challenge_id) {
        session.close();
    }
    Ok(())
}

#[tauri::command]
pub async fn ed_workspace_read(
    app: AppHandle,
    challenge_id: i64,
    path: String,
) -> Result<String, String> {
    let session = session(&app, challenge_id)?;
    blocking::run(move || session.read(&path)).await
}

/// Write the library file, then upload it while online. Saves keep the
/// order they were asked in: the place is taken before any waiting.
#[tauri::command]
pub async fn ed_workspace_save(
    app: AppHandle,
    challenge_id: i64,
    path: String,
    text: String,
) -> Result<Saved, String> {
    let session = session(&app, challenge_id)?;
    let place = session.place();
    blocking::run(move || session.save(place, &path, text.as_bytes())).await
}

#[tauri::command]
pub async fn ed_workspace_delete(
    app: AppHandle,
    challenge_id: i64,
    path: String,
) -> Result<(), String> {
    let session = session(&app, challenge_id)?;
    let place = session.place();
    blocking::run(move || session.delete(place, &path)).await
}
