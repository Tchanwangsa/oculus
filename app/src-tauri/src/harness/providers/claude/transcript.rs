//! Reading the CLI's own transcript file, which holds what stdout never
//! says: the uuid of the question a turn answered.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::Value;

/// Where the CLI keeps a session's transcript: `projects/<cwd slug>/<id>.jsonl`,
/// the slug being the cwd with every non-alphanumeric char turned to `-`
/// (the CLI does not announce it with auto-memory off). Falls back to a
/// search by file name, since session ids are unique.
pub(super) fn transcript_path(cwd: &str, session_id: &str) -> Option<PathBuf> {
    if cwd.is_empty() || session_id.is_empty() {
        return None;
    }
    let root = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".claude")))?;
    let projects = root.join("projects");
    let slug: String = cwd
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let name = format!("{session_id}.jsonl");
    let direct = projects.join(&slug).join(&name);
    if direct.exists() {
        return Some(direct);
    }
    std::fs::read_dir(&projects).ok()?.flatten().find_map(|e| {
        let p = e.path().join(&name);
        p.exists().then_some(p)
    })
}

/// The uuid of the question a turn answered. The transcript is a tree by
/// `parentUuid`, so walk up from the turn's first answer to a `user` row that
/// is not a tool result, skipping attachments. With no answer (an interrupted
/// turn) take the newest question, which is this turn's.
pub(super) fn anchor_for(path: &Path, first_assistant: Option<&str>) -> Option<String> {
    struct Row {
        parent: Option<String>,
        question: bool,
    }
    let file = std::fs::File::open(path).ok()?;
    let mut by_uuid: HashMap<String, Row> = HashMap::new();
    let mut newest_question: Option<String> = None;
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(uuid) = v.get("uuid").and_then(|u| u.as_str()) else {
            continue;
        };
        let question = v.get("type").and_then(|t| t.as_str()) == Some("user")
            && v.get("tool_use_result").is_none();
        if question {
            newest_question = Some(uuid.to_string());
        }
        by_uuid.insert(
            uuid.to_string(),
            Row {
                parent: v
                    .get("parentUuid")
                    .and_then(|p| p.as_str())
                    .map(String::from),
                question,
            },
        );
    }
    let Some(start) = first_assistant else {
        return newest_question;
    };
    let mut at = start.to_string();
    // Bounded: a malformed parent chain must not loop forever.
    for _ in 0..by_uuid.len() {
        let row = by_uuid.get(&at)?;
        if row.question {
            return Some(at);
        }
        at = row.parent.clone()?;
    }
    None
}

/// A tool result's text: the structured `tool_use_result` (stdout + stderr
/// for Bash) when the CLI attached one, else the content blocks.
pub(super) fn tool_result_text(block: &Value, structured: Option<&Value>) -> String {
    if let Some(s) = structured {
        if let Some(text) = s.as_str() {
            return text.to_string();
        }
        let stdout = s.get("stdout").and_then(|x| x.as_str());
        let stderr = s.get("stderr").and_then(|x| x.as_str());
        if stdout.is_some() || stderr.is_some() {
            let mut out = stdout.unwrap_or("").to_string();
            if let Some(e) = stderr.filter(|e| !e.is_empty()) {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(e);
            }
            return out;
        }
    }
    match block.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A question, its attachments, the answer, then a tool result that is
    /// also a `user` row and must not be mistaken for a question.
    fn transcript(dir: &Path) -> PathBuf {
        let rows = [
            r#"{"type":"user","uuid":"q1","parentUuid":null,"message":{"role":"user","content":"first"}}"#,
            r#"{"type":"assistant","uuid":"a1","parentUuid":"q1"}"#,
            r#"{"type":"user","uuid":"q2","parentUuid":"a1","message":{"role":"user","content":"second"}}"#,
            r#"{"type":"attachment","uuid":"at1","parentUuid":"q2"}"#,
            r#"{"type":"attachment","uuid":"at2","parentUuid":"at1"}"#,
            r#"{"type":"assistant","uuid":"a2","parentUuid":"at2"}"#,
            r#"{"type":"user","uuid":"tr1","parentUuid":"a2","tool_use_result":{"ok":true}}"#,
            r#"{"type":"assistant","uuid":"a3","parentUuid":"tr1"}"#,
        ];
        let path = dir.join("session.jsonl");
        std::fs::write(&path, rows.join("\n")).unwrap();
        path
    }

    #[test]
    fn the_anchor_is_the_question_the_answer_hangs_off() {
        let dir = crate::test_support::Scratch::new("anchor");
        let path = transcript(&dir);

        assert_eq!(anchor_for(&path, Some("a2")).as_deref(), Some("q2"));
        assert_eq!(anchor_for(&path, Some("a1")).as_deref(), Some("q1"));
        assert_eq!(anchor_for(&path, Some("a3")).as_deref(), Some("q2"));
        assert_eq!(anchor_for(&path, None).as_deref(), Some("q2"));
        assert_eq!(anchor_for(&path, Some("nope")), None);
    }

    /// A dotfile in the cwd produces a double dash.
    #[test]
    fn the_transcript_slug_flattens_everything_but_letters_and_digits() {
        let dir = crate::test_support::Scratch::new("slug");
        let projects = dir.join("projects").join("-tmp-a-b--claude-c-d");
        std::fs::create_dir_all(&projects).unwrap();
        std::fs::write(projects.join("sess.jsonl"), "").unwrap();
        std::env::set_var("CLAUDE_CONFIG_DIR", &*dir);

        let found = transcript_path("/tmp/a b/.claude/c_d", "sess");
        assert_eq!(
            found.as_deref(),
            Some(projects.join("sess.jsonl").as_path())
        );
        assert_eq!(transcript_path("/tmp/a b/.claude/c_d", "gone"), None);

        std::env::remove_var("CLAUDE_CONFIG_DIR");
    }
}
