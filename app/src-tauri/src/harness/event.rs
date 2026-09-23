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
            // Lowercase is the project's own branding, and the frontend's
            // `PROVIDERS` entry spells it the same way.
            Provider::Opencode => "opencode",
            // The product is "Antigravity"; the binary is `agy`. A student
            // reads the former and types the latter, so the label is the
            // product and `discover::binary_name` is the only place the
            // three letters appear.
            Provider::Antigravity => "Antigravity",
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
    ///
    /// `text` is what the student typed. A message sent from the lecture
    /// player's dock can also carry the moment it was sent at — `at`, the
    /// playhead's second — which becomes the row's `meta` and the "at 3:40"
    /// on the bubble. It is skipped when absent so the recorded fixtures the
    /// bridge tests replay, and the frontend's own type, stay as they were.
    UserMessage {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<i64>,
    },
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
        /// A row title the provider only knew once the call was over, which
        /// replaces the one [`Self::ToolStarted`] opened with.
        ///
        /// Codex's web search is why it exists: its `item/started` carries an
        /// empty `query` and the real queries arrive with the results, so a
        /// search row could otherwise never say what was searched for — the
        /// one thing Claude's `WebSearch` row says from its first event.
        /// `None` everywhere else, and `None` leaves the row's title alone.
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
    /// The thread has a name. Neither CLI names a conversation over its own
    /// protocol, so this is the answer to a naming turn asked of the thread's
    /// provider once its first exchange is done
    /// ([`super::Harness::name_thread`]). It rides the same stream as
    /// everything else so the row is written before the webview hears it.
    ThreadTitled { title: String },
    /// A message typed while a turn was running. It is not in the
    /// conversation yet and no row is written for it: it is waiting its turn
    /// (`Queue` in [`super`]), and the webview draws it as pending.
    Queued { id: String, text: String },
    /// A queued message has left the queue — cancelled, cleared by an
    /// interrupt, or about to go out, in which case its
    /// [`Self::UserMessage`] follows immediately.
    Unqueued { id: String },
    /// The provider's own handle for the turn that just went out — Claude's
    /// uuid for the user message, Codex's turn id. It is kept on the user's
    /// row because it is the thing a later rewind has to name
    /// (`rewind_conversation`, `thread/revert`), and neither CLI will hand it
    /// out again afterwards.
    TurnAnchor { anchor: String },
    /// Rows from `from_item_id` on are gone: a question was edited and the
    /// thread rewound to it.
    Rewound {
        from_item_id: i64,
        /// Whether the provider's own session was rewound too. False only for
        /// a question asked before the anchor was recorded, or one whose
        /// session has since been deleted — there the agent keeps the
        /// original in its context and the timeline says so.
        context: bool,
    },
    /// The provider stopped working on the user's message.
    TurnFinished {
        /// `completed`, `interrupted`, `failed`.
        status: String,
    },
    Error {
        message: String,
        /// Set when the message is the provider saying it has no usable
        /// credentials. The timeline draws such a row as a sign-in card
        /// rather than red text, so this is the difference between an error
        /// a student can act on and one they can only read.
        auth: Option<Provider>,
    },
    /// The provider process is gone. The thread stays; the next message
    /// resumes it.
    Exited { code: Option<i32> },
}

impl HarnessEvent {
    /// An error with no provider behind it — the manager failing to start a
    /// bridge, a job that never reached a CLI. `auth` is `None` because there
    /// is nothing for a student to sign in to.
    pub fn error(message: impl Into<String>) -> Self {
        HarnessEvent::Error {
            message: message.into(),
            auth: None,
        }
    }

    /// An error the named provider reported, classified: if it reads as a
    /// credentials failure, the row says which agent to sign in to.
    ///
    /// The classification lives in [`signin::is_auth_failure`](super::signin::is_auth_failure)
    /// rather than here, because it is the same judgement the sign-in dialog
    /// makes and there should only be one copy of it. It is deliberately
    /// narrow: a row that offers a sign-in over an unrelated failure sends a
    /// student to re-authenticate and leaves them with the same error and one
    /// less reason to trust the card.
    pub fn error_for(provider: Provider, message: impl Into<String>) -> Self {
        let message = message.into();
        let auth = super::signin::is_auth_failure(provider, &message).then_some(provider);
        HarnessEvent::Error { message, auth }
    }
}

/// Classify a tool by its raw name and input. Provider-specific names are
/// mapped here so all three bridges share one table.
///
/// The three CLIs spell their tools differently and, worse, spell their
/// *arguments* differently. Claude sends `Read { file_path }`; opencode's
/// runner sends `read { path }` — measured off the wire, not off its
/// `/experimental/tool` registry, which still advertises `filePath`. So the
/// lowercase names below have their own arms wherever the accessor differs,
/// and only join a Claude arm where the key is genuinely the same
/// (`command`, `pattern`, `url`). A fall-through would have titled every
/// opencode file row with an empty string, which looks like a missing title
/// rather than a wrong lookup.
pub fn classify(name: &str, input: &serde_json::Value) -> (ToolKind, String) {
    let s = |k: &str| input.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    // Antigravity is the fourth spelling of the same handful of tools, and the
    // only one that **PascalCases its parameters**: `run_command` takes
    // `CommandLine` and `view_file` takes `AbsolutePath` — both read off the
    // wire from `agy` 1.2.9, not from a schema. A lowercase fall-through would
    // have titled every one of its rows with an empty string, which is exactly
    // the failure opencode's `path`-vs-`file_path` arms are already here for.
    //
    // The two above are measured. The rest of the keys in each list are the
    // same convention applied to the tool names in `agy`'s own `init.tools`,
    // and are there so a row is titled rather than blank if the guess is
    // right; narrow each list as a recording confirms it.
    let any = |ks: &[&str]| {
        ks.iter()
            .find_map(|k| input.get(*k).and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string()
    };
    let base = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
    match name {
        // Antigravity's own names, off the `tools` array its `init` event
        // prints. `CommandLine` and `AbsolutePath` are measured; see above.
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
        // The agent asking the student something. There is nowhere for it to
        // be answered from a timeline, so it is a row like any other.
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
        // One tool for a whole multi-file patch; the row is titled with the
        // first file the patch names, which is the one an `*** Update File:`
        // header carries.
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
                // `mcp__server__tool` — title by the tool, the server rides
                // along in the raw name.
                let tool = rest.splitn(2, "__").nth(1).unwrap_or(rest);
                return (ToolKind::Other, tool.to_string());
            }
            (ToolKind::Other, String::new())
        }
    }
}

/// The first file an `apply_patch` envelope names. The patch text is a
/// sequence of `*** Add File: p` / `*** Update File: p` / `*** Delete File: p`
/// headers; anything else has no path in it and titles the row with nothing,
/// which is what an unparseable patch deserves.
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

    /// opencode's runner spells its tools in lowercase and its file argument
    /// `path`. Letting those fall through to Claude's arms would have read
    /// `file_path` off every one of them and titled the row with nothing.
    #[test]
    fn opencode_tool_names_are_titled_off_their_own_arguments() {
        use serde_json::json;
        assert_eq!(classify("read", &json!({"path": "../courses/COMP30026/w1.md"})), (ToolKind::Read, "w1.md".into()));
        assert_eq!(classify("write", &json!({"path": "memories/a.md"})), (ToolKind::Write, "a.md".into()));
        assert_eq!(classify("edit", &json!({"path": "memories/a.md"})), (ToolKind::Edit, "a.md".into()));
        assert_eq!(classify("glob", &json!({"pattern": "**/*.md"})), (ToolKind::Search, "**/*.md".into()));
        assert_eq!(classify("todowrite", &json!({"todos": []})), (ToolKind::Plan, String::new()));
        assert_eq!(classify("skill", &json!({"name": "customize-opencode"})), (ToolKind::Other, "customize-opencode".into()));
        // Bash is the one place the two CLIs agree on the argument name.
        assert_eq!(
            classify("bash", &json!({"command": "oculus files COMP30026"})),
            (ToolKind::OculusCli, "oculus files COMP30026".into())
        );
        assert_eq!(
            classify("apply_patch", &json!({"patchText": "*** Begin Patch\n*** Update File: memories/x.md\n@@\n-a\n+b\n*** End Patch"})),
            (ToolKind::Edit, "x.md".into())
        );
        // Claude's own spelling is untouched by any of it.
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
