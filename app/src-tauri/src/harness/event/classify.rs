//! Naming a tool call: one table that turns each CLI's raw tool name and
//! input into a [`ToolKind`] and a short title, and the cap on tool output.

use super::ToolKind;

/// Classify a tool by raw name and input: one table for every bridge. Each CLI
/// spells argument keys differently (Claude `file_path`, opencode `path` despite
/// its registry saying `filePath`), so names share an arm only where the key does.
pub fn classify(name: &str, input: &serde_json::Value) -> (ToolKind, String) {
    let s = |k: &str| {
        input
            .get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
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
            let kind = if is_oculus_cli(&cmd) {
                ToolKind::OculusCli
            } else {
                ToolKind::Bash
            };
            (kind, cmd)
        }
        "view_file" | "read_resource" => (
            ToolKind::Read,
            base(&any(&["AbsolutePath", "TargetFile", "Path"])),
        ),
        "write_to_file" => (
            ToolKind::Write,
            base(&any(&["AbsolutePath", "TargetFile", "Path"])),
        ),
        "replace_file_content" | "multi_replace_file_content" | "sed_file" | "notebook_edit" => (
            ToolKind::Edit,
            base(&any(&["AbsolutePath", "TargetFile", "Path"])),
        ),
        "list_dir" => (
            ToolKind::Search,
            base(&any(&["DirectoryPath", "AbsolutePath", "Path"])),
        ),
        "find_by_name" => (
            ToolKind::Search,
            any(&["Pattern", "Query", "SearchDirectory"]),
        ),
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
        for verb in [
            "*** Add File:",
            "*** Update File:",
            "*** Delete File:",
            "*** Move to:",
        ] {
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
        let (k, t) = classify(
            "Bash",
            &serde_json::json!({"command": "oculus grep foo -s COMP30026"}),
        );
        assert_eq!(k, ToolKind::OculusCli);
        assert_eq!(t, "oculus grep foo -s COMP30026");
        let (k, _) = classify(
            "Bash",
            &serde_json::json!({"command": "/usr/local/bin/oculus files X"}),
        );
        assert_eq!(k, ToolKind::OculusCli);
        let (k, _) = classify("Bash", &serde_json::json!({"command": "ls myoculus"}));
        assert_eq!(k, ToolKind::Bash);
    }

    #[test]
    fn opencode_tool_names_are_titled_off_their_own_arguments() {
        use serde_json::json;
        assert_eq!(
            classify("read", &json!({"path": "../courses/COMP30026/w1.md"})),
            (ToolKind::Read, "w1.md".into())
        );
        assert_eq!(
            classify("write", &json!({"path": "memories/a.md"})),
            (ToolKind::Write, "a.md".into())
        );
        assert_eq!(
            classify("edit", &json!({"path": "memories/a.md"})),
            (ToolKind::Edit, "a.md".into())
        );
        assert_eq!(
            classify("glob", &json!({"pattern": "**/*.md"})),
            (ToolKind::Search, "**/*.md".into())
        );
        assert_eq!(
            classify("todowrite", &json!({"todos": []})),
            (ToolKind::Plan, String::new())
        );
        assert_eq!(
            classify("skill", &json!({"name": "customize-opencode"})),
            (ToolKind::Other, "customize-opencode".into())
        );
        assert_eq!(
            classify("bash", &json!({"command": "oculus files COMP30026"})),
            (ToolKind::OculusCli, "oculus files COMP30026".into())
        );
        assert_eq!(
            classify(
                "apply_patch",
                &json!({"patchText": "*** Begin Patch\n*** Update File: memories/x.md\n@@\n-a\n+b\n*** End Patch"})
            ),
            (ToolKind::Edit, "x.md".into())
        );
        assert_eq!(
            classify("Read", &json!({"file_path": "/a/b.md"})),
            (ToolKind::Read, "b.md".into())
        );
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
