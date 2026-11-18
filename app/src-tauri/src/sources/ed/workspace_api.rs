//! Ed's `/api` calls a workspace download needs: attach the student's
//! workspace and ticket a download of it. Each ticket is a credential for the
//! workspace server (`sahara`), so it is held in a [`Ticket`], whose `Debug`
//! never shows it.

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
}

fn ticket_in(reply: &Value) -> Option<Ticket> {
    reply["ticket"]
        .as_str()
        .filter(|t| !t.is_empty())
        .map(|t| Ticket(t.to_string()))
}
