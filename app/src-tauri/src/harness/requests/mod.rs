//! What an agent asks the student mid-turn. A bridge turns its CLI's
//! callback into a [`NewRequest`] and a [`Route`] back to that callback; the
//! consumer saves the request (`store.rs`) in the same step that registers
//! the route (`registry.rs`), so a request is either fully stored before its
//! session is released or never stored for that session. The student's
//! answer is the `Respond` command (`core/decide/respond.rs`); the reply is
//! written by an effect, and what the bridge can prove of it is saved as the
//! request's `delivery`. docs/harness.md, "Requests".

pub(in crate::harness) mod policy;
pub(in crate::harness) mod registry;
pub(in crate::harness) mod route;
pub(in crate::harness) mod store;

pub(in crate::harness) use registry::Registry;
#[cfg(test)]
pub(in crate::harness) use route::Route;
pub(in crate::harness) use route::{LiveRoute, Reply};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How a chat's agent asks for what its sandbox blocks. Stored on the
/// thread (`harness_threads.permission_mode`) and read when a run starts.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PermissionMode {
    /// Each request is shown to the student, who answers it.
    #[default]
    Ask,
    /// Declined without a prompt, as one-off jobs always are. Shown to the
    /// student as "Don't ask".
    Quiet,
    /// The CLI's own reviewer decides and the student is never asked. A
    /// bridge with no reviewer (`Capabilities::auto_review`) asks instead.
    Auto,
}

impl PermissionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            PermissionMode::Ask => "ask",
            PermissionMode::Quiet => "quiet",
            PermissionMode::Auto => "auto",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ask" => Some(PermissionMode::Ask),
            "quiet" => Some(PermissionMode::Quiet),
            "auto" => Some(PermissionMode::Auto),
            _ => None,
        }
    }
}

/// What a request is about. A command runs outside the sandbox once
/// approved; the others name what they would let in.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Command,
    FileChange,
    Permissions,
    Network,
    Access,
    Tool,
    Rule,
}

impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Command => "command",
            Action::FileChange => "file_change",
            Action::Permissions => "permissions",
            Action::Network => "network",
            Action::Access => "access",
            Action::Tool => "tool",
            Action::Rule => "rule",
        }
    }
}

/// What an option does once chosen, whatever the provider calls it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Allow this one call.
    Accept,
    /// Allow it, and what the option's `scope` says, without asking again.
    AcceptForSession,
    Decline,
}

/// One answer a request offers: only what its provider really takes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RequestOption {
    /// What `Respond` names.
    pub id: String,
    pub label: String,
    pub decision: Decision,
    /// What the answer covers, in a sentence, where it reaches past this
    /// one call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// A request as a bridge raises it, before it is saved.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NewRequest {
    pub action: Action,
    /// What the student is shown: the command and its folder, the files
    /// and their diff, the access asked for, the reason, and the agent
    /// that asked when it is not the chat's own.
    pub payload: Value,
    pub options: Vec<RequestOption>,
    /// The provider's callback id as JSON, exactly as sent (a number or a
    /// string).
    pub native_ref: String,
    /// The paths the request names, checked against the protected ones
    /// before the student sees it (`policy.rs`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
}

/// Where a request stands.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    /// The student answered it.
    Resolved,
    /// Its session went away under it: the process ended, the provider
    /// withdrew it, or the app was closed.
    Expired,
    /// The student ended it: Stop, rewind or delete, or the turn it waited
    /// in finished.
    Cancelled,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::Resolved => "resolved",
            Status::Expired => "expired",
            Status::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Status::Pending),
            "resolved" => Some(Status::Resolved),
            "expired" => Some(Status::Expired),
            "cancelled" => Some(Status::Cancelled),
            _ => None,
        }
    }
}

/// What is known of the answer's way to the provider.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    /// Not answered yet.
    None,
    /// Saved and being written, or written and not yet confirmed.
    Sending,
    /// The provider has it: Codex retired its callback, Claude's pipe took
    /// the line, opencode answered 200.
    Acknowledged,
    /// Nothing reached the provider; the student may answer again.
    Failed,
    /// The write broke partway: it may have arrived, so it is never resent.
    Uncertain,
}

impl Delivery {
    pub fn as_str(self) -> &'static str {
        match self {
            Delivery::None => "none",
            Delivery::Sending => "sending",
            Delivery::Acknowledged => "acknowledged",
            Delivery::Failed => "failed",
            Delivery::Uncertain => "uncertain",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "none" => Some(Delivery::None),
            "sending" => Some(Delivery::Sending),
            "acknowledged" => Some(Delivery::Acknowledged),
            "failed" => Some(Delivery::Failed),
            "uncertain" => Some(Delivery::Uncertain),
            _ => None,
        }
    }
}

/// A saved request, as the webview reads it (`HarnessRequest` in
/// `app/src/lib/harness/types.ts`), from `harness_load` and the
/// `request_opened` and `request_updated` events.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RequestView {
    pub id: i64,
    pub thread_id: i64,
    pub run_id: Option<i64>,
    /// The timeline row that shows it.
    pub item_id: Option<i64>,
    pub provider: crate::harness::Provider,
    /// `approval`; `question` comes with stage 3.
    pub kind: String,
    pub action: Action,
    pub payload: Value,
    pub options: Vec<RequestOption>,
    pub status: Status,
    /// `live`, `message` or `not_resumable`.
    pub responder: String,
    /// Why it can no longer be answered, in the student's words.
    pub reason: Option<String>,
    /// The option chosen.
    pub response: Option<String>,
    pub delivery: Delivery,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

/// Why a request ended without an answer, as its row says it.
pub(in crate::harness) mod reasons {
    pub const APP_CLOSED: &str = "The app was closed.";
    pub const PROCESS_ENDED: &str = "The agent's process ended.";
    pub const WITHDRAWN: &str = "The agent withdrew this request.";
    pub const STOPPED: &str = "The turn was stopped.";
    pub const TURN_ENDED: &str = "The turn ended before it was answered.";
    pub const REPLACED: &str = "The agent's session was restarted.";
}
