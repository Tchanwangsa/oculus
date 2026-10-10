//! The one event stream every bridge produces.
//!
//! Each bridge folds its CLI's dialect into this enum; the manager persists
//! it, the frontend renders it and the CLI prints it. A new provider is one
//! translator to here, not a timeline change.

mod classify;

pub use classify::{cap_output, classify, MAX_TOOL_OUTPUT};

use serde::{Deserialize, Serialize};

/// Which CLI a thread is bound to. Stored on the thread row as its string
/// form, so the names are part of the schema.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Claude,
    Codex,
    Opencode,
    Antigravity,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Claude => "claude",
            Provider::Codex => "codex",
            Provider::Opencode => "opencode",
            Provider::Antigravity => "antigravity",
        }
    }

    pub fn parse(s: &str) -> Option<Provider> {
        match s {
            "claude" => Some(Provider::Claude),
            "codex" => Some(Provider::Codex),
            "opencode" => Some(Provider::Opencode),
            "antigravity" => Some(Provider::Antigravity),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Provider::Claude => "Claude Code",
            Provider::Codex => "Codex",
            // Lowercase is the project's own branding.
            Provider::Opencode => "opencode",
            // The product name; the binary `agy` lives in `discover::binary_name`.
            Provider::Antigravity => "Antigravity",
        }
    }
}

/// What a tool call *is*, independent of the provider's name for it, which
/// rides along in [`HarnessEvent::ToolStarted::name`].
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Read,
    Edit,
    Write,
    Bash,
    /// Grep, Glob, file search.
    Search,
    /// A shell command that runs the `oculus` binary.
    OculusCli,
    /// A subagent / delegation.
    Task,
    /// WebFetch, WebSearch.
    Web,
    /// Plan / todo bookkeeping, collapsed by default.
    Plan,
    /// Compaction, and anything else the timeline folds away.
    Other,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RateWindow {
    pub label: String,
    /// 0–100.
    pub used_percent: f64,
    /// Unix seconds.
    pub resets_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HarnessEvent {
    /// Once per process start; a resumed thread announces the same id again.
    SessionStarted {
        provider_session_id: String,
        model: Option<String>,
        cwd: String,
    },
    /// The user's message, echoed once sent. `at` is the lecture playhead's
    /// second for a message from the player dock; omitted when absent.
    UserMessage {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<i64>,
    },
    /// The provider began working on the last user message.
    TurnStarted,
    /// Live assistant text, superseded by the [`Self::AssistantMessage`] that follows.
    AssistantDelta {
        text: String,
    },
    /// Live reasoning text.
    ThinkingDelta {
        text: String,
    },
    /// A finished block of assistant text: persisted, where deltas are display only.
    AssistantMessage {
        text: String,
    },
    /// A finished block of reasoning text. Persisted, collapsed by default.
    Thinking {
        text: String,
    },
    ToolStarted {
        /// Provider's id for the call; [`Self::ToolFinished`] closes on it.
        id: String,
        kind: ToolKind,
        /// Raw provider tool name (`Bash`, `commandExecution`, `mcp__x__y`).
        name: String,
        /// One line for the row: the command, the path, the query.
        title: String,
        input: serde_json::Value,
    },
    /// Streamed tool output (command stdout so far).
    ToolOutputDelta {
        id: String,
        text: String,
    },
    ToolFinished {
        id: String,
        ok: bool,
        /// Output or error text, capped by the bridge.
        output: String,
        /// A title known only once the call is over (Codex's web search sends
        /// its query with the results); replaces `ToolStarted`'s. `None` keeps it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    Usage {
        input_tokens: u64,
        output_tokens: u64,
        /// Tokens the last request occupied — what "context used" means.
        context_tokens: Option<u64>,
        context_window: Option<u64>,
        cost_usd: Option<f64>,
    },
    RateLimits {
        windows: Vec<RateWindow>,
    },
    /// The answer to the naming turn ([`super::Harness::name_thread`]).
    ThreadTitled {
        title: String,
    },
    /// A message typed mid-turn, waiting in `Queue`; no row is written for it.
    Queued {
        id: String,
        text: String,
    },
    /// Left the queue: cancelled, cleared, or sent (its `UserMessage` follows).
    Unqueued {
        id: String,
    },
    /// The provider's handle for the turn just sent (Claude's message uuid,
    /// Codex's turn id), kept on the user row: a rewind names it, no CLI re-issues it.
    TurnAnchor {
        anchor: String,
    },
    /// Rows from `from_item_id` on are gone: an edited question rewound the thread.
    Rewound {
        from_item_id: i64,
        /// Whether the provider's session rewound too (false: no anchor, or session gone).
        context: bool,
    },
    /// Antigravity refused a step its rules don't allow; the turn is already
    /// over. Carries what to allow before the next message; persisted as a row.
    PermissionNeeded {
        /// The provider's tool (`run_command`, `write_to_file`).
        tool: String,
        /// In the provider's word: `command`, `write_file`, `read_file`, `read_url`.
        action: String,
        /// What it was refused on: the command line, or the path.
        target: Option<String>,
        /// A rule that would allow it (`command(python3)`), when readable off the refusal.
        rule: Option<String>,
    },
    /// The provider stopped working on the user's message.
    TurnFinished {
        /// `completed`, `interrupted`, `failed`.
        status: String,
    },
    Error {
        message: String,
        /// Set when the provider has no usable credentials; drawn as a sign-in card.
        auth: Option<Provider>,
    },
    /// The process is gone; the thread stays and the next message resumes it.
    Exited {
        code: Option<i32>,
    },
}

impl HarnessEvent {
    /// An error with no provider behind it, so no sign-in to offer.
    pub fn error(message: impl Into<String>) -> Self {
        HarnessEvent::Error {
            message: message.into(),
            auth: None,
        }
    }

    /// A provider's error, with `auth` set when the (deliberately narrow)
    /// [`signin::is_auth_failure`](super::signin::is_auth_failure) reads it as a credentials failure.
    pub fn error_for(provider: Provider, message: impl Into<String>) -> Self {
        let message = message.into();
        let auth = super::signin::is_auth_failure(provider, &message).then_some(provider);
        HarnessEvent::Error { message, auth }
    }
}
