//! The one event stream every bridge produces.
//!
//! Claude Code and Codex speak different dialects — Anthropic stream events
//! wrapped in `stream-json` lines on one side, JSON-RPC notifications on the
//! other — and nothing downstream of the bridge should know which. The
//! bridges fold both into this enum; the manager persists it, the frontend
//! renders it, and the CLI prints it. Adding a provider means writing one
//! translator to here, not touching the timeline.
//!
//! Shapes are kept deliberately flat. bb's delta grammar carries keys,
//! channels and presentation hints so its UI can stay provider-agnostic;
//! this app has one timeline and one style, so the same information is a
//! handful of variants with named fields.

use serde::{Deserialize, Serialize};

/// Which CLI a thread is bound to. Stored on the thread row as its string
/// form, so the names are part of the schema.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Claude,
    Codex,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Claude => "claude",
            Provider::Codex => "codex",
        }
    }

    pub fn parse(s: &str) -> Option<Provider> {
        match s {
            "claude" => Some(Provider::Claude),
            "codex" => Some(Provider::Codex),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Provider::Claude => "Claude Code",
            Provider::Codex => "Codex",
        }
    }
}

/// What a tool call *is*, independent of what the provider calls it. The
/// timeline picks an icon and a verb from this; the raw tool name rides
/// along in [`HarnessEvent::ToolStarted::name`] for anything it does not
/// cover.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Read,
    Edit,
    Write,
    Bash,
    /// Grep, Glob, file search.
    Search,
    /// A shell command that runs the `oculus` binary — the library's own
    /// door, worth naming so the row can say "Searched the library".
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
    /// The provider has a session for this thread. Arrives once per process
    /// start — a resumed thread announces the same id again.
    SessionStarted {
        provider_session_id: String,
        model: Option<String>,
        cwd: String,
    },
    /// The user's message, echoed once it is on its way to the provider —
    /// so the timeline has one shape for both sides of the conversation.
    UserMessage { text: String },
    /// The provider began working on the last user message.
    TurnStarted,
    /// Live assistant text; the frontend appends. Superseded by the
    /// [`Self::AssistantMessage`] that follows it.
    AssistantDelta { text: String },
    /// Live reasoning text.
    ThinkingDelta { text: String },
    /// A finished block of assistant text. This is what gets persisted;
    /// deltas are only ever display.
    AssistantMessage { text: String },
    /// A finished block of reasoning text. Persisted, collapsed by default.
    Thinking { text: String },
    ToolStarted {
        /// Provider's id for the call; [`Self::ToolFinished`] closes on it.
        id: String,
        kind: ToolKind,
        /// Raw provider tool name (`Bash`, `commandExecution`, `mcp__x__y`).
        name: String,
        /// One line for the row: the command, the path, the query.
        title: String,
        /// The full input, for the expanded row.
        input: serde_json::Value,
    },
    /// Streamed tool output (command stdout so far).
    ToolOutputDelta { id: String, text: String },
    ToolFinished {
        id: String,
        ok: bool,
        /// Output or error text, capped by the bridge.
        output: String,
    },
    Usage {
        input_tokens: u64,
        output_tokens: u64,
        /// Tokens the last request occupied — what "context used" means.
        context_tokens: Option<u64>,
        context_window: Option<u64>,
        cost_usd: Option<f64>,
    },
    RateLimits { windows: Vec<RateWindow> },
    /// The provider stopped working on the user's message.
    TurnFinished {
        /// `completed`, `interrupted`, `failed`.
        status: String,
    },
    Error { message: String },
    /// The provider process is gone. The thread stays; the next message
    /// resumes it.
    Exited { code: Option<i32> },
}

impl HarnessEvent {
    pub fn error(message: impl Into<String>) -> Self {
        HarnessEvent::Error {
            message: message.into(),
        }
    }
}

/// Classify a tool by its raw name and input. Provider-specific names are
/// mapped here so both bridges share one table.
pub fn classify(name: &str, input: &serde_json::Value) -> (ToolKind, String) {
    let s = |k: &str| input.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let base = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
    match name {
        "Bash" | "commandExecution" => {
            let cmd = s("command");
            let kind = if is_oculus_cli(&cmd) {
                ToolKind::OculusCli
            } else {
                ToolKind::Bash
            };
            (kind, cmd)
        }
        "Read" => (ToolKind::Read, base(&s("file_path"))),
        "Edit" | "MultiEdit" => (ToolKind::Edit, base(&s("file_path"))),
        "Write" => (ToolKind::Write, base(&s("file_path"))),
        "NotebookEdit" => (ToolKind::Edit, base(&s("notebook_path"))),
        "fileChange" => (ToolKind::Edit, s("title")),
        "Grep" | "Glob" => (ToolKind::Search, s("pattern")),
        "WebSearch" => (ToolKind::Web, s("query")),
        "WebFetch" | "webSearch" => (ToolKind::Web, s("url")),
        "Task" | "Agent" | "collabAgentToolCall" => (
            ToolKind::Task,
            if s("description").is_empty() {
                s("prompt").lines().next().unwrap_or("").to_string()
            } else {
                s("description")
            },
        ),
        "TodoWrite" | "TaskCreate" | "TaskUpdate" | "TaskList" | "TaskGet" => {
            (ToolKind::Plan, String::new())
        }
        "Skill" => (ToolKind::Other, s("skill")),
        _ => {
            if let Some(rest) = name.strip_prefix("mcp__") {
                // `mcp__server__tool` — title by the tool, the server rides
                // along in the raw name.
                let tool = rest.splitn(2, "__").nth(1).unwrap_or(rest);
                return (ToolKind::Other, tool.to_string());
            }
            (ToolKind::Other, String::new())
        }
    }
}

/// `oculus grep …`, `/path/to/oculus search …`, or the same behind an env
/// prefix. Word-boundary rather than substring, so `myoculus` is not it.
fn is_oculus_cli(cmd: &str) -> bool {
    cmd.split_whitespace()
        .take(3)
        .any(|w| w == "oculus" || w.ends_with("/oculus"))
}

/// Tool output is a row in a timeline, not a transcript; past this it is
/// truncated with a marker. Bash output of 200 KB would otherwise be a
/// 200 KB row in `harness_items` and a 200 KB Tauri event.
pub const MAX_TOOL_OUTPUT: usize = 16 * 1024;

pub fn cap_output(s: &str) -> String {
    if s.len() <= MAX_TOOL_OUTPUT {
        return s.to_string();
    }
    let mut end = MAX_TOOL_OUTPUT;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n… [truncated {} bytes]", &s[..end], s.len() - end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_oculus_commands_by_word() {
        let (k, t) = classify("Bash", &serde_json::json!({"command": "oculus grep foo -s COMP30026"}));
        assert_eq!(k, ToolKind::OculusCli);
        assert_eq!(t, "oculus grep foo -s COMP30026");
        let (k, _) = classify("Bash", &serde_json::json!({"command": "/usr/local/bin/oculus files X"}));
        assert_eq!(k, ToolKind::OculusCli);
        let (k, _) = classify("Bash", &serde_json::json!({"command": "ls myoculus"}));
        assert_eq!(k, ToolKind::Bash);
    }

    #[test]
    fn mcp_tools_title_by_tool_name() {
        let (k, t) = classify("mcp__github__list_prs", &serde_json::json!({}));
        assert_eq!(k, ToolKind::Other);
        assert_eq!(t, "list_prs");
    }

    #[test]
    fn output_cap_keeps_char_boundaries() {
        let s = "é".repeat(MAX_TOOL_OUTPUT);
        let capped = cap_output(&s);
        assert!(capped.contains("[truncated"));
        assert!(capped.len() < s.len());
    }
}
