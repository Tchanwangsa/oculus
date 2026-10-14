//! A submission's code. Ed copies it into a temporary workspace for anyone
//! who connects with the submission's ticket; `init.wid` names that copy, and
//! its tree downloads like any workspace's. Nothing is ever uploaded to or
//! removed from it. The files are kept in memory per submission, since a
//! submission never changes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;

use crate::sources::ed::workspace_api::Ticket;
use crate::sources::ed::{open_live_workspace, sahara, unpack_workspace, Ed, WorkspaceFile};

type Files = Arc<Vec<WorkspaceFile>>;

fn cache() -> &'static Mutex<HashMap<i64, Files>> {
    static CACHE: OnceLock<Mutex<HashMap<i64, Files>>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

/// One file of a submission, for the read-only Code view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodeFile {
    pub path: String,
    /// UTF-8, lossy.
    pub text: String,
}

pub fn as_text(files: &[WorkspaceFile]) -> Vec<CodeFile> {
    files
        .iter()
        .map(|f| CodeFile {
            path: f.path.clone(),
            text: String::from_utf8_lossy(&f.bytes).into_owned(),
        })
        .collect()
}

/// Submission `submission_id`'s files, from the cache or Ed.
pub fn files(ed: &Ed, submission_id: i64) -> Result<Files, String> {
    if let Some(files) = cached(submission_id) {
        return Ok(files);
    }
    fetch(
        ed,
        &ed.workspace_host()?,
        submission_id,
        |host, ticket| {
            let live = open_live_workspace(host, ticket.expose())?;
            let wid = live
                .wid()
                .ok_or("Ed opened no copy of the submission's code")?
                .to_string();
            Ok((live, wid))
        },
        |host, ticket| sahara::download(&sahara::origin(host), ticket),
    )
}

fn cached(submission_id: i64) -> Option<Files> {
    let cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    cache.get(&submission_id).cloned()
}

/// [`files`] from the workspace server on `host`, with the socket and the
/// download handed in: `open` holds the temporary workspace live while its
/// guard lives and names it.
pub fn fetch<G>(
    ed: &Ed,
    host: &str,
    submission_id: i64,
    open: impl FnOnce(&str, &Ticket) -> Result<(G, String), String>,
    download: impl FnOnce(&str, &Ticket) -> Result<Vec<u8>, String>,
) -> Result<Files, String> {
    let ticket = ed.submission_ticket(submission_id)?;
    let (live, wid) = open(host, &ticket)?;
    let zip = ed
        .download_ticket(&wid, ".")
        .and_then(|ticket| download(host, &ticket));
    drop(live);
    let (files, skipped) = unpack_workspace(&zip?)?;
    for skip in skipped {
        elog!("[oculus] ed submission {submission_id}: {skip}");
    }
    let files: Files = Arc::new(files);
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(submission_id, files.clone());
    Ok(files)
}
