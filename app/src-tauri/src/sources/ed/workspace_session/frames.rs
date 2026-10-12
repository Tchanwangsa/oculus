//! The session socket's frames: JSON text `{type, data}` both ways. Only the
//! kinds a workspace uses are read; the rest (the live-editing `file_ot*`,
//! cursors, chat, clients joining) come back as `None` and are ignored.
//!
//! `init` also carries `auth`, a credential: it is never read into a field,
//! so it cannot reach a log through `Debug`.

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

/// A frame from the workspace server.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    Init(Init),
    Ping,
    NewCommit(NewCommit),
    Fsop(Fsop),
    ListReply(ListReply),
    /// Ed's own sentence for something that went wrong.
    DisplayError(String),
    ProgramInit(ProgramInit),
    ProgramData(ProgramData),
    ProgramUpdate(ProgramUpdate),
    ProgramClose(ProgramClose),
}

/// The session's first frame.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct Init {
    pub read_only: bool,
    /// Recent commits, newest first: the head is what the server holds now.
    pub last_commits: Vec<CommitRef>,
    /// Terminals and other programs that outlive a session, by id.
    pub program_state: Value,
}

impl Init {
    /// The commit the workspace is at, when the server named one.
    pub fn head(&self) -> Option<&str> {
        self.last_commits.first().map(|c| c.hash.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CommitRef {
    pub hash: String,
}

/// A commit landed: every upload makes one. `context` is set only for an
/// explicit `commit {id}`, and echoes that id.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct NewCommit {
    pub commit: CommitRef,
    #[serde(default)]
    pub context: Option<Value>,
}

/// A file-system change, ours echoed or another client's.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Fsop {
    /// `new_file`, `new_folder`, `remove`, `symlink`, `move` or `load_blob`.
    #[serde(rename = "type")]
    pub op: String,
    #[serde(default)]
    pub param1: Option<String>,
    #[serde(default)]
    pub param2: Option<String>,
    #[serde(default)]
    pub param3: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ListReply {
    #[serde(default)]
    pub listing: Vec<ListEntry>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ListEntry {
    pub name: String,
    /// `file` or `folder`.
    #[serde(rename = "type", default)]
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ProgramInit {
    pub id: String,
    #[serde(default)]
    pub state: Value,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub persist: bool,
}

/// Terminal output, base64 of the raw bytes (ANSI included).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ProgramData {
    pub id: String,
    pub data: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ProgramUpdate {
    pub id: String,
    #[serde(default)]
    pub state: Value,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ProgramClose {
    pub id: String,
}

#[derive(Deserialize)]
struct Raw {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    data: Value,
}

/// One text frame, `None` for a kind not listed in [`Incoming`] or a
/// listed kind of a shape this does not know.
pub fn parse(text: &str) -> Option<Incoming> {
    let Raw { kind, data } = serde_json::from_str(text).ok()?;
    Some(match kind.as_str() {
        "init" => Incoming::Init(of(data)?),
        "ping" => Incoming::Ping,
        "new_commit" => Incoming::NewCommit(of(data)?),
        "fsop" => Incoming::Fsop(of(data)?),
        "list_reply" => Incoming::ListReply(of(data)?),
        "display_error" => Incoming::DisplayError(sentence(data)),
        "program_init" => Incoming::ProgramInit(of(data)?),
        "program_data" => Incoming::ProgramData(of(data)?),
        "program_update" => Incoming::ProgramUpdate(of(data)?),
        "program_close" => Incoming::ProgramClose(match data {
            Value::String(id) => ProgramClose { id },
            data => of(data)?,
        }),
        _ => return None,
    })
}

/// `init`'s `data`, as `live::wait_for_init` hands it over.
pub fn init(data: Value) -> Init {
    of(data).unwrap_or_default()
}

fn of<T: DeserializeOwned>(data: Value) -> Option<T> {
    serde_json::from_value(data).ok()
}

fn sentence(data: Value) -> String {
    match data {
        Value::String(s) => s,
        Value::Object(mut o) => match o.remove("message") {
            Some(Value::String(s)) => s,
            _ => Value::Object(o).to_string(),
        },
        other => other.to_string(),
    }
}

/// Keeps the session open: the web app sends it every 270 s.
pub fn keep_alive() -> String {
    json!({ "type": "keep_alive" }).to_string()
}

/// Remove a file or a folder, workspace-relative like an upload's path.
pub fn remove(path: &str) -> String {
    json!({
        "type": "fsop",
        "data": { "type": "remove", "param1": path, "param2": "", "param3": "" },
    })
    .to_string()
}
