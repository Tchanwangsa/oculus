//! Ed's workspace server over HTTP: a file up, the tree down. The ticket in
//! the URL is the only credential (`workspace_api`), so these requests go
//! straight to the server, and no error names the URL.

use std::io::Read;
use std::time::Duration;

use super::workspace_api::Ticket;
use crate::runtime::multipart::{self, FilePart};

const CONNECT_WAIT: Duration = Duration::from_secs(30);
/// The bound on each read of an answer, not on the whole request.
const READ_WAIT: Duration = Duration::from_secs(60);

/// `https://<host>`, the origin both calls are made against.
pub fn origin(host: &str) -> String {
    format!("https://{host}")
}

/// Upload one file at `path` (workspace-relative, `/`-separated; a nested
/// path makes its folders). Every upload is a commit: the answer is its hash.
pub fn upload(origin: &str, ticket: &Ticket, path: &str, bytes: &[u8]) -> Result<String, String> {
    let boundary = multipart::boundary();
    let body = multipart::body(
        &boundary,
        &[],
        &FilePart {
            field: "file",
            filename: path,
            content_type: "application/octet-stream",
        },
        bytes,
    );
    let reply = agent()
        .post(&format!("{origin}/upload/{}", ticket.expose()))
        .set("Content-Type", &multipart::content_type(&boundary))
        .send_bytes(&body);
    let text = answer(reply)?;
    let reply: serde_json::Value = serde_json::from_slice(&text)
        .map_err(|_| "the workspace server's answer to the upload is not JSON".to_string())?;
    reply["hash"]
        .as_str()
        .filter(|h| !h.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "the workspace server took the upload but named no commit".into())
}

/// Download what a download ticket names: the whole tree as a zip for `"."`.
pub fn download(origin: &str, ticket: &Ticket) -> Result<Vec<u8>, String> {
    answer(
        agent()
            .get(&format!("{origin}/download/{}", ticket.expose()))
            .call(),
    )
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(CONNECT_WAIT)
        .timeout_read(READ_WAIT)
        .build()
}

/// The body of a 2xx answer; otherwise the status or the transport's kind,
/// never `ureq`'s own message, which quotes the URL.
fn answer(reply: Result<ureq::Response, ureq::Error>) -> Result<Vec<u8>, String> {
    let response = match reply {
        Ok(r) if (200..300).contains(&r.status()) => r,
        Ok(r) => return Err(format!("the workspace server answered HTTP {}", r.status())),
        Err(ureq::Error::Status(code, _)) => {
            return Err(format!("the workspace server answered HTTP {code}"))
        }
        Err(ureq::Error::Transport(t)) => {
            return Err(format!(
                "could not reach the workspace server ({})",
                t.kind()
            ))
        }
    };
    let mut body = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|e| format!("the workspace server's answer was cut off ({})", e.kind()))?;
    Ok(body)
}
