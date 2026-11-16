//! The one event stream every bridge produces.
//!
//! Each bridge folds its CLI's dialect into this enum; the manager persists
//! it, the frontend renders it and the CLI prints it. A new provider is one
//! translator to here, not a timeline change.

pub mod answer;
mod classify;
mod turn;

pub use classify::{cap_output, classify, MAX_TOOL_OUTPUT};
pub use turn::{unbound, Bound, TurnId, Turns};

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

/// Where a message or reasoning block came from: the provider's own id for it,
/// scoped to the attempt that produced it (docs/harness.md, "Each send is a run").
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Source {
    pub id: String,
    #[serde(default)]
    pub phase: Phase,
    /// Its closing snapshot never came: the text is what streamed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
}

impl Source {
    pub fn of(id: impl Into<String>) -> Option<Self> {
        Some(Source {
            id: id.into(),
            phase: Phase::Unknown,
            partial: false,
        })
    }

    pub fn partial(id: impl Into<String>) -> Option<Self> {
        Some(Source {
            id: id.into(),
            phase: Phase::Unknown,
            partial: true,
        })
    }
}

/// The provider's own label for a message. Only Codex sends one; `Unknown`
/// is a real value, not commentary.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Unknown,
    Commentary,
    FinalAnswer,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Unknown => "unknown",
            Phase::Commentary => "commentary",
            Phase::FinalAnswer => "final_answer",
        }
    }
}

/// A decision the CLI made on one tool call without asking anyone: Auto's
/// reviewer, or (on Claude) a deny rule, which holds in every mode.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Review {
    /// `in_progress`, `approved`, `denied`, `timed_out` or `aborted`.
    pub status: String,
    /// `low`, `medium`, `high`, as the reviewer rated it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    /// Who decided, as the CLI names it: `agent` for Codex's reviewer,
    /// `classifier` for Claude's Auto, `rule` for a deny rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// What a [`HarnessEvent::DeliveryGap`] was observed from.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GapCause {
    /// Claude sent a narration block with no text.
    EmptyNarration,
    /// Text streamed, but the turn ended without its closing snapshot.
    MissingSnapshot,
    /// Streamed text the provider never said was answer or reasoning.
    UnknownProtocol,
    /// Oculus could not save a row the provider sent.
    StorageFailure,
}

impl GapCause {
    pub fn as_str(self) -> &'static str {
        match self {
            GapCause::EmptyNarration => "empty_narration",
            GapCause::MissingSnapshot => "missing_snapshot",
            GapCause::UnknownProtocol => "unknown_protocol",
            GapCause::StorageFailure => "storage_failure",
        }
    }
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
        /// The run whose question row is already written; never serialized.
        #[serde(skip)]
        run: Option<super::store::RunStart>,
    },
    /// The provider began a turn nobody sent a message for (Claude picking up
    /// a background task that finished). Holds the thread like a sent message
    /// until its [`Self::TurnFinished`]; persisted as a row. Comes right before
    /// that turn's [`Self::TurnStarted`].
    UnpromptedTurn {
        /// The finished background tasks' descriptions, if known.
        after: Vec<String>,
    },
    /// The provider began working on the last user message, or, after
    /// [`Self::UnpromptedTurn`], on a turn of its own.
    TurnStarted {
        /// Which turn of its session; the bridge's, never serialized.
        #[serde(skip)]
        turn: Option<TurnId>,
    },
    /// Live assistant text, superseded by the [`Self::AssistantMessage`] that follows.
    AssistantDelta {
        text: String,
        /// The provider's id for the message this text belongs to, the one
        /// its [`Self::AssistantMessage`] names as `source`; `None` when the
        /// stream gives none that would match. Never serialized.
        #[serde(skip)]
        item: Option<String>,
    },
    /// Live reasoning text.
    ThinkingDelta {
        text: String,
    },
    /// A finished block of assistant text: persisted, where deltas are display only.
    /// The same `source` again is the same message, re-sent or completed.
    AssistantMessage {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<Source>,
    },
    /// A finished block of reasoning text. Persisted, collapsed by default.
    Thinking {
        text: String,
        /// How long the block streamed, from its first delta. Bridges leave it
        /// `None`; the consumer loop times it (`app/thought_clock.rs`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<Source>,
    },
    /// Evidence that public text the provider meant to send did not arrive.
    /// Stored for the run, whose settlement shows it as a notice; nothing resends.
    DeliveryGap {
        /// The message or block it concerns, when the provider named one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        cause: GapCause,
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
    /// The CLI's own decision on a tool call (Auto's reviewer, or a deny
    /// rule): stored on the call's row (`meta.review`). The student was not
    /// asked.
    ToolReviewed {
        /// The call's id, as its `ToolStarted` named it.
        id: String,
        review: Review,
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
    /// The student's messages waiting behind the thread's turn, whole and
    /// in order, told again whenever the queue changes: one joined, was
    /// edited, removed or moved, or went out (its `UserMessage` follows).
    /// Each is a queued run (`store/queue.rs`); none has a timeline row.
    Queue {
        messages: Vec<super::store::QueuedMessage>,
    },
    /// The student asked this run to stop. Saved on the run
    /// (`stop_requested_at`); the run itself ends only on its turn's own
    /// terminal.
    StopRequested {
        run_id: i64,
    },
    /// Fifteen seconds after a stop the run is still open: its CLI is not
    /// answering. The student may force stop it, which ends `others` other
    /// chats' turns too when one server answers them all.
    StopUnresponsive {
        run_id: i64,
        others: i64,
    },
    /// The provider's handle for the turn just sent (Claude's message uuid,
    /// Codex's turn id), kept on the user row: a rewind names it, no CLI re-issues it.
    TurnAnchor {
        anchor: String,
    },
    /// A committed proposal snapshot, shared by every view of its thread.
    Proposal {
        proposal: crate::db::proposals::Proposal,
    },
    /// A card was applied or discarded, or its job finished. The consumer
    /// starts the card's follow-up run, if it has one (`app/continuation.rs`).
    CardSettled {
        proposal: String,
    },
    /// A run Oculus started after a card settled, in place of a question:
    /// its row says why ("Sync finished — continuing"), never as the student.
    Continued {
        text: String,
        proposal: String,
        /// The run whose row is already written; never serialized.
        #[serde(skip)]
        run: Option<super::store::RunStart>,
    },
    /// Rows from `from_item_id` on are gone: an edited question rewound the thread.
    Rewound {
        from_item_id: i64,
        /// Whether the provider's session rewound too (false: no anchor, or session gone).
        context: bool,
    },
    /// The provider compacted the conversation's context. A row of its own:
    /// never an answer, and its summary is never shown as the student's prose.
    Compaction {
        /// The provider's id for it; the same one again is the same compaction.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
    /// A student's run ended: which row is its one answer (none when no
    /// text arrived), which earlier replies are work, and whether everything
    /// the provider meant to say arrived (`delivered`, `incomplete`,
    /// `not_applicable`). Rust decides; the webview only applies it. `status`
    /// and the stored interval let the timeline fold the run's work.
    RunSettled {
        answer_item_id: Option<i64>,
        work_item_ids: Vec<i64>,
        delivery: String,
        #[serde(default)]
        status: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        started_at: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ended_at: Option<String>,
    },
    /// The chat's selection changed: the provider, model and reasoning
    /// level its next send will use. A run already written keeps its own.
    Selected {
        provider: Provider,
        model: Option<String>,
        effort: Option<String>,
    },
    /// A run's context handoff: the earlier conversation its first prompt
    /// carries to a session that has not seen it. Told when the run is
    /// written (`pending`, or `failed` for one that does not fit) and again
    /// when its start step is over (`inline` once the provider has the
    /// prompt, `failed` when it never did).
    Handoff {
        run_id: i64,
        #[serde(flatten)]
        handoff: crate::harness::handoff::store::HandoffView,
    },
    /// What a run is sent with: the provider, model and reasoning level of
    /// the chat's selection when it started. Told once, after the run's
    /// question; an answer's footer reads it, never the thread.
    RunSelected {
        provider: Provider,
        model: Option<String>,
        effort: Option<String>,
    },
    /// The CLI is waiting on the student: run a command outside the
    /// sandbox, change files, reach more. From a bridge only; the consumer
    /// checks it, saves it with `route` registered, and tells
    /// [`Self::RequestOpened`] in its place.
    RequestAsked {
        request: crate::harness::requests::NewRequest,
        /// The way back to the waiting callback; never serialized.
        #[serde(skip)]
        route: Option<crate::harness::requests::LiveRoute>,
    },
    /// A request saved, with its timeline row (`item_id`).
    RequestOpened {
        request: crate::harness::requests::RequestView,
        /// The thread's requests waiting on the student once this one is
        /// saved: what the thread list's Approval reads.
        #[serde(default)]
        waiting: u32,
        #[serde(default)]
        waiting_questions: u32,
    },
    /// A request changed: answered, its answer's delivery learned, or ended
    /// without one.
    RequestUpdated {
        request: crate::harness::requests::RequestView,
        /// As on [`Self::RequestOpened`], once this change is saved.
        #[serde(default)]
        waiting: u32,
        #[serde(default)]
        waiting_questions: u32,
    },
    /// The provider retired a callback (`native_ref`): after an answer it
    /// has it; with none, it withdrew the request. From a bridge only.
    RequestRetired {
        native_ref: String,
    },
    /// How the chat asks from its next run on.
    PermissionMode {
        mode: crate::harness::requests::PermissionMode,
    },
    /// A saved conversation could not load; the send starts a fresh session.
    SessionRestarted {
        text: String,
    },
    /// The student wrote into the running run: its `steer` row, owned by
    /// that run, is already written (`store/steer.rs`). `message` is the
    /// message's own identity, a `steered` run that never runs.
    Steer {
        text: String,
        message: i64,
        /// The id its provider knows it by, and its first `delivery`
        /// (`sending`, or `restart`), as the row's `meta` holds them.
        client: String,
        delivery: String,
        /// The row the commit wrote; never serialized.
        #[serde(skip)]
        item: Option<i64>,
    },
    /// What became of a steer: `written` (the provider took the write),
    /// `delivered` (it echoed the message back), `unknown` (the write broke
    /// partway; never resent), or `requeued` (it missed the turn: its row is
    /// gone and the message waits first in the queue as its own run).
    SteerUpdated {
        item_id: i64,
        message: i64,
        delivery: String,
    },
    /// The provider echoed a steer written under `client_id`. From a
    /// bridge only; the consumer saves it and tells [`Self::SteerUpdated`].
    SteerEchoed {
        client_id: String,
    },
    /// A restart steer stopped the run's turn: its attempt is superseded
    /// and `attempt_id` carries the run on with the correction. The run is
    /// still open; only the new attempt's terminal settles it.
    AttemptRestarted {
        run_id: i64,
        attempt_id: i64,
        superseded: i64,
        /// The superseded attempt's replies, now work (`commentary`).
        work_item_ids: Vec<i64>,
    },
    /// A completed run whose answer, or part of it, never reached Oculus.
    /// Persisted as a row; never followed by a resend.
    DeliveryNotice {
        text: String,
    },
    /// The provider stopped working on the user's message, or on its own turn.
    TurnFinished {
        /// `completed`, `interrupted`, `failed`.
        status: String,
        /// Which turn of its session ended: only the run that turn belongs
        /// to is settled by it. `None` from the app, for a send that failed
        /// before its turn. Never serialized.
        #[serde(skip)]
        turn: Option<TurnId>,
    },
    Error {
        message: String,
        /// Set when the provider has no usable credentials; drawn as a sign-in card.
        auth: Option<Provider>,
        /// The provider's usage limit is spent; drawn as the composer's
        /// "Usage limit reached" notice.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        limit: bool,
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
            limit: false,
        }
    }

    /// A provider's error, with `auth` set when the (deliberately narrow)
    /// [`signin::is_auth_failure`](super::signin::is_auth_failure) reads it as a credentials failure,
    /// and `limit` when [`signin::is_usage_limit`](super::signin::is_usage_limit) reads a spent usage limit.
    pub fn error_for(provider: Provider, message: impl Into<String>) -> Self {
        let message = message.into();
        let auth = super::signin::is_auth_failure(provider, &message).then_some(provider);
        let limit = super::signin::is_usage_limit(provider, &message);
        HarnessEvent::Error {
            message,
            auth,
            limit,
        }
    }
}
