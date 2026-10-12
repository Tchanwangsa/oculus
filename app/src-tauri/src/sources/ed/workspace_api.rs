//! Ed's `/api` calls around a challenge workspace: attach it, ticket a
//! download, an upload or a run, mark it, and read a submission back. Each
//! ticket is a credential for the workspace server (`sahara`), so it is held
//! in a [`Ticket`], whose `Debug` never shows it.
//!
//! `mark` makes a real submission that counts in Ed's records: only the
//! workspace's Mark button may call it, never a sync, a reconnect or an agent.

use super::Ed;

use serde_json::{json, Value};

/// A one-shot credential for the workspace server.
#[derive(Clone)]
pub struct Ticket(String);

impl Ticket {
    /// The ticket itself, for the one URL that carries it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Ticket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Ticket(…)")
    }
}

/// `POST /challenges/<ch>/connect`: the student's workspace, attached.
#[derive(Debug, Clone)]
pub struct Connected {
    pub workspace_id: String,
    /// Opens `wss://<host>/connect`.
    pub ticket: Ticket,
}

impl Ed {
    /// Attach the student's own workspace for a challenge, what opening the
    /// slide in Ed does, and get the session socket's ticket.
    pub fn connect(&self, challenge_id: i64) -> Result<Connected, String> {
        let reply = self.post(&format!("/challenges/{challenge_id}/connect"), &json!({}))?;
        let workspace_id = reply["workspace_id"]
            .as_str()
            .map(str::to_string)
            .or_else(|| reply["workspace_id"].as_i64().map(|n| n.to_string()))
            .ok_or("Ed returned no workspace for the challenge")?;
        let ticket = ticket_in(&reply).ok_or("Ed returned no session ticket for the workspace")?;
        Ok(Connected {
            workspace_id,
            ticket,
        })
    }

    /// A ticket for `GET <sahara>/download/<ticket>`: `"."` is the whole tree
    /// as a zip (`""` is refused and `"/"` errors), a file path the raw file.
    /// The workspace must be live.
    pub fn download_ticket(&self, workspace_id: &str, path: &str) -> Result<Ticket, String> {
        let reply = self.post(
            &format!("/workspaces/{workspace_id}/download"),
            &json!({ "path": path }),
        )?;
        ticket_in(&reply).ok_or_else(|| "Ed returned no download ticket for the workspace".into())
    }

    /// A ticket for `POST <sahara>/upload/<ticket>`. The path must be `"."`
    /// (`/home` gets a ticket the upload then fails on); each file part's
    /// name is its path under it. The workspace must be live.
    pub fn upload_ticket(&self, workspace_id: &str) -> Result<Ticket, String> {
        let reply = self.post(
            &format!("/workspaces/{workspace_id}/upload"),
            &json!({ "wid": workspace_id, "path": "." }),
        )?;
        ticket_in(&reply).ok_or_else(|| "Ed returned no upload ticket for the workspace".into())
    }

    /// A ticket for `wss://<sahara>/run`, running `command` on the workspace
    /// at commit `hash` in a `cols`×`rows` terminal. `""` runs Ed's check.
    pub fn run_ticket(
        &self,
        challenge_id: i64,
        hash: &str,
        command: &str,
        (cols, rows): (u16, u16),
    ) -> Result<Ticket, String> {
        let reply = self.post(
            &format!("/challenges/{challenge_id}/run"),
            &json!({
                "pty_size": { "cols": cols, "rows": rows },
                "hash": hash,
                "command": command,
                "dump_files": true,
            }),
        )?;
        ticket_in(&reply).ok_or_else(|| "Ed returned no run ticket for the challenge".into())
    }

    /// Submit the workspace at commit `hash` for marking. A real submission:
    /// see the module header. Blocks until Ed has marked it.
    pub fn mark(&self, challenge_id: i64, hash: &str) -> Result<Value, String> {
        let reply = self.post(
            &format!("/challenges/{challenge_id}/mark"),
            &json!({ "hash": hash }),
        )?;
        submission_in(reply)
    }

    /// One submission, by the id `mark` returned. Ed lists none for a
    /// student, so the ids Oculus made are the history.
    pub fn submission(&self, submission_id: i64) -> Result<Value, String> {
        submission_in(self.get(&format!("/challenges/submissions/{submission_id}"))?)
    }
}

fn ticket_in(reply: &Value) -> Option<Ticket> {
    reply["ticket"]
        .as_str()
        .filter(|t| !t.is_empty())
        .map(|t| Ticket(t.to_string()))
}

/// The answer's `submission`, or the bare object.
fn submission_in(mut reply: Value) -> Result<Value, String> {
    match reply["submission"].take() {
        Value::Object(s) => Ok(Value::Object(s)),
        _ if reply.is_object() && reply.get("id").is_some() => Ok(reply),
        _ => Err("Ed returned no submission".into()),
    }
}
