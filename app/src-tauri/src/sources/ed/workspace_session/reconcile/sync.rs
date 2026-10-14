//! The lesson sync's reconcile of a workspace that has a record. It holds
//! the workspace live itself (`open_live_workspace`), since Ed only serves
//! a live one, and leaves a challenge open in the app to its session.

use std::path::Path;

use super::compare::{self, Head, Outcome, Remote};
use super::registry;
use crate::sources::ed::live::LiveWorkspace;
use crate::sources::ed::workspace_session::frames;
use crate::sources::ed::workspace_state;
use crate::sources::ed::{open_live_workspace, sahara, Ed};

pub enum Synced {
    Done(Outcome),
    /// Not reconciled, and why.
    LeftAlone(String),
}

struct LiveRemote<'a> {
    ed: &'a Ed,
    origin: String,
    workspace_id: &'a str,
    live: LiveWorkspace,
}

impl Remote for LiveRemote<'_> {
    fn tree(&mut self) -> Result<Vec<u8>, String> {
        let ticket = self.ed.download_ticket(self.workspace_id, ".")?;
        sahara::download(&self.origin, &ticket)
    }

    fn upload(&mut self, path: &str, bytes: &[u8]) -> Result<String, String> {
        let ticket = self.ed.upload_ticket(self.workspace_id)?;
        sahara::upload(&self.origin, &ticket, path, bytes)
    }

    fn remove(&mut self, path: &str) -> Result<(), String> {
        self.live.send(frames::remove(path, false))
    }
}

/// Reconcile challenge `challenge_id`'s folder `slide_dir` (data-dir-relative)
/// with Ed and save the record. A record naming another folder (the lesson
/// was renamed) is left alone: this folder holds none of its work.
pub fn sync(
    ed: &Ed,
    data_dir: &Path,
    challenge_id: i64,
    slide_dir: &str,
) -> Result<Synced, String> {
    let Some(_claim) = registry::begin_sync(challenge_id) else {
        return Ok(Synced::LeftAlone(
            "open in Oculus, so its session keeps it in step".into(),
        ));
    };
    let mut record = workspace_state::load(data_dir, challenge_id).unwrap_or_default();
    if !record.slide_dir.is_empty() && record.slide_dir != slide_dir {
        return Ok(Synced::LeftAlone(format!(
            "its local work is in {}, not this slide's folder",
            record.slide_dir
        )));
    }
    let host = ed.workspace_host()?;
    let connected = ed.connect(challenge_id)?;
    let live = open_live_workspace(&host, connected.ticket.expose())?;
    let head = live.head().map(str::to_string);
    let mut remote = LiveRemote {
        ed,
        origin: sahara::origin(&host),
        workspace_id: &connected.workspace_id,
        live,
    };
    let head = Head {
        hash: head.as_deref(),
        changed: false,
    };
    let outcome = compare::run(&data_dir.join(slide_dir), &record, head, &mut remote)?;
    record.workspace_id = connected.workspace_id.clone();
    record.slide_dir = slide_dir.to_string();
    record.files = outcome.files.clone();
    record.base_commit = outcome.base_commit.clone();
    workspace_state::save(data_dir, challenge_id, &record)?;
    Ok(Synced::Done(outcome))
}
