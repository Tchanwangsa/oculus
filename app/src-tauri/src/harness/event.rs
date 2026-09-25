//! The one event stream every bridge produces.
//!
//! Each bridge folds its CLI's dialect into this enum; the manager persists
//! it, the frontend renders it and the CLI prints it. A new provider is one
//! translator to here, not a timeline change.

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
    AssistantDelta { text: String },
    /// Live reasoning text.
    ThinkingDelta { text: String },
    /// A finished block of assistant text: persisted, where deltas are display only.
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
        input: serde_json::Value,
    },
    /// Streamed tool output (command stdout so far).
    ToolOutputDelta { id: String, text: String },
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
    RateLimits { windows: Vec<RateWindow> },
    /// The answer to the naming turn ([`super::Harness::name_thread`]).
    ThreadTitled { title: String },
    /// A message typed mid-turn, waiting in `Queue`; no row is written for it.
    Queued { id: String, text: String },
    /// Left the queue: cancelled, cleared, or sent (its `UserMessage` follows).
    Unqueued { id: String },
    /// The provider's handle for the turn just sent (Claude's message uuid,
    /// Codex's turn id), kept on the user row: a rewind names it, no CLI re-issues it.
    TurnAnchor { anchor: String },
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
    Exited { code: Option<i32> },
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

/// Classify a tool by raw name and input: one table for every bridge. Each CLI
/// spells argument keys differently (Claude `file_path`, opencode `path` despite
/// its registry saying `filePath`), so names share an arm only where the key does.
pub fn classify(name: &str, input: &serde_json::Value) -> (ToolKind, String) {
    let s = |k: &str| input.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    // Antigravity PascalCases its parameters (`CommandLine`, `AbsolutePath` on
    // agy 1.2.9); the other keys in each list follow that convention unconfirmed.
    let any = |ks: &[&str]| {
        ks.iter()
            .find_map(|k| input.get(*k).and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string()
    };
    let base = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
    match name {
        // Antigravity's names, from its `init` event's `tools`.
        "run_command" => {
            let cmd = any(&["CommandLine", "Command"]);
            let kind = if is_oculus_cli(&cmd) { ToolKind::OculusCli } else { ToolKind::Bash };
            (kind, cmd)
        }
        "view_file" | "read_resource" => (ToolKind::Read, base(&any(&["AbsolutePath", "TargetFile", "Path"]))),
        "write_to_file" => (ToolKind::Write, base(&any(&["AbsolutePath", "TargetFile", "Path"]))),
        "replace_file_content" | "multi_replace_file_content" | "sed_file" | "notebook_edit" => {
            (ToolKind::Edit, base(&any(&["AbsolutePath", "TargetFile", "Path"])))
        }
        "list_dir" => (ToolKind::Search, base(&any(&["DirectoryPath", "AbsolutePath", "Path"]))),
        "find_by_name" => (ToolKind::Search, any(&["Pattern", "Query", "SearchDirectory"])),
        "grep_search" => (ToolKind::Search, any(&["Query", "SearchTerm", "Pattern"])),
        "read_url_content" | "open_browser_url" => (ToolKind::Web, any(&["Url", "URL"])),
        "search_web" => (ToolKind::Web, any(&["Query", "SearchTerm"])),
        "manage_task" | "schedule" => (ToolKind::Plan, String::new()),
        "invoke_subagent" | "define_subagent" | "browser_subagent" => {
            (ToolKind::Task, any(&["Name", "Prompt", "TypeName"]))
        }
        // The agent asking the student something; a timeline can't answer it.
        "ask_question" | "ask_permission" | "ask_custom_permission" => {
            (ToolKind::Other, any(&["Question", "Prompt"]))
        }
        "Bash" | "commandExecution" | "bash" => {
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
        "Grep" | "Glob" | "grep" | "glob" => (ToolKind::Search, s("pattern")),
        "WebSearch" | "websearch" | "webSearch" => (ToolKind::Web, s("query")),
        "WebFetch" | "webfetch" => (ToolKind::Web, s("url")),
        // opencode's own file tools. `path`, not `file_path`.
        "read" => (ToolKind::Read, base(&s("path"))),
        "edit" => (ToolKind::Edit, base(&s("path"))),
        "write" => (ToolKind::Write, base(&s("path"))),
        "list" => (ToolKind::Search, base(&s("path"))),
        // A multi-file patch is titled by the first file it names.
        "apply_patch" => (ToolKind::Edit, base(&patch_target(&s("patchText")))),
        "skill" => (ToolKind::Other, s("name")),
        "question" => (ToolKind::Other, String::new()),
        "Task" | "Agent" | "collabAgentToolCall" | "task" => (
            ToolKind::Task,
            if s("description").is_empty() {
                s("prompt").lines().next().unwrap_or("").to_string()
            } else {
                s("description")
            },
        ),
        "TodoWrite" | "TaskCreate" | "TaskUpdate" | "TaskList" | "TaskGet" | "todowrite" => {
            (ToolKind::Plan, String::new())
        }
        "Skill" => (ToolKind::Other, s("skill")),
        _ => {
            if let Some(rest) = name.strip_prefix("mcp__") {
                // `mcp__server__tool`: title by the tool.
                let tool = rest.splitn(2, "__").nth(1).unwrap_or(rest);
                return (ToolKind::Other, tool.to_string());
            }
            (ToolKind::Other, String::new())
        }
    }
}

/// The first path an `apply_patch` envelope's `*** … File:` headers name, or "".
fn patch_target(patch: &str) -> String {
    for line in patch.lines() {
        let line = line.trim();
        for verb in ["*** Add File:", "*** Update File:", "*** Delete File:", "*** Move to:"] {
            if let Some(rest) = line.strip_prefix(verb) {
                return rest.trim().to_string();
            }
        }
    }
    String::new()
}

/// `oculus grep …`, `/path/to/oculus search …`, or the same behind an env
/// prefix. Word-boundary rather than substring, so `myoculus` is not it.
fn is_oculus_cli(cmd: &str) -> bool {
    cmd.split_whitespace()
        .take(3)
        .any(|w| w == "oculus" || w.ends_with("/oculus"))
}

/// Tool output past this is truncated with a marker: it is a timeline row and a Tauri event.
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
    fn opencode_tool_names_are_titled_off_their_own_arguments() {
        use serde_json::json;
        assert_eq!(classify("read", &json!({"path": "../courses/COMP30026/w1.md"})), (ToolKind::Read, "w1.md".into()));
        assert_eq!(classify("write", &json!({"path": "memories/a.md"})), (ToolKind::Write, "a.md".into()));
        assert_eq!(classify("edit", &json!({"path": "memories/a.md"})), (ToolKind::Edit, "a.md".into()));
        assert_eq!(classify("glob", &json!({"pattern": "**/*.md"})), (ToolKind::Search, "**/*.md".into()));
        assert_eq!(classify("todowrite", &json!({"todos": []})), (ToolKind::Plan, String::new()));
        assert_eq!(classify("skill", &json!({"name": "customize-opencode"})), (ToolKind::Other, "customize-opencode".into()));
        assert_eq!(
            classify("bash", &json!({"command": "oculus files COMP30026"})),
            (ToolKind::OculusCli, "oculus files COMP30026".into())
        );
        assert_eq!(
            classify("apply_patch", &json!({"patchText": "*** Begin Patch\n*** Update File: memories/x.md\n@@\n-a\n+b\n*** End Patch"})),
            (ToolKind::Edit, "x.md".into())
        );
        assert_eq!(classify("Read", &json!({"file_path": "/a/b.md"})), (ToolKind::Read, "b.md".into()));
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
