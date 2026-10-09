use std::sync::atomic::Ordering;

use serde_json::Value;

use crate::harness::event::{HarnessEvent, ToolKind};

use super::super::translate::{command_word, is_question, refusal, Translator};

/// A real `agy` 1.2.9 session, recorded with the bridge's flags.
#[test]
fn folds_a_recorded_session() {
    let raw = include_str!("../../../../../fixtures/harness/antigravity-ls.ndjson");
    let mut t = Translator::default();
    let events: Vec<HarnessEvent> = raw
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .flat_map(|v| t.translate(&v))
        .collect();

    let session = events.iter().find_map(|e| match e {
        HarnessEvent::SessionStarted {
            provider_session_id,
            cwd,
            ..
        } => Some((provider_session_id.clone(), cwd.clone())),
        _ => None,
    });
    let (id, cwd) = session.expect("conversation_id and cwd off the init event");
    assert!(!id.is_empty());
    assert!(cwd.ends_with("agytest"));

    // PascalCase `CommandLine`: a lowercase lookup titles this "".
    let tools: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::ToolStarted {
                kind, title, name, ..
            } => Some((*kind, title.clone(), name.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        tools,
        vec![(
            ToolKind::Bash,
            "ls -a".to_string(),
            "run_command".to_string()
        )]
    );

    let finished: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::ToolFinished { ok, output, .. } => Some((*ok, output.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(finished.len(), 1);
    assert!(finished[0].0, "the command succeeded");
    assert!(finished[0].1.contains(".."), "stdout rode `output`");

    let deltas: String = events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::AssistantDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(!deltas.trim().is_empty(), "the agent said something");
    let messages = events
        .iter()
        .filter(|e| matches!(e, HarnessEvent::AssistantMessage { .. }))
        .count();
    assert!(messages >= 1);

    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, HarnessEvent::TurnStarted))
            .count(),
        1
    );
    assert!(matches!(
        events.last(),
        Some(HarnessEvent::TurnFinished { status }) if status == "completed"
    ));
    assert!(!events
        .iter()
        .any(|e| matches!(e, HarnessEvent::Error { .. })));
}

#[test]
fn a_result_status_maps_to_one_of_three() {
    for (status, want, err) in [
        ("SUCCESS", "completed", false),
        ("INTERRUPTED", "interrupted", false),
        ("CANCELED", "interrupted", false),
        ("ERROR", "failed", true),
        ("INVALID", "failed", true),
        ("WAITING", "failed", true),
    ] {
        let mut t = Translator::default();
        let v = serde_json::json!({
            "event": "result",
            "result": { "conversation_id": "x", "status": status },
        });
        let out = t.translate(&v);
        assert!(
            matches!(out.last(), Some(HarnessEvent::TurnFinished { status: s }) if s == want),
            "{status} → {want}"
        );
        assert_eq!(
            out.iter().any(|e| matches!(e, HarnessEvent::Error { .. })),
            err,
            "{status} error row"
        );
    }
}

#[test]
fn an_asked_for_stop_is_not_a_failure() {
    let mut t = Translator::default();
    t.interrupting.store(true, Ordering::SeqCst);
    let out = t.translate(&serde_json::json!({
        "event": "result",
        "result": { "status": "ERROR", "error": "killed" },
    }));
    assert!(
        matches!(out.last(), Some(HarnessEvent::TurnFinished { status }) if status == "interrupted")
    );
    assert!(!out.iter().any(|e| matches!(e, HarnessEvent::Error { .. })));
}

/// A refused step as 1.2.9 sends it, then the `result` that ends the turn.
#[test]
fn a_refusal_closes_the_row_and_names_the_rule() {
    let mut t = Translator::default();
    let step = |state: &str, error: Value| {
        serde_json::json!({
            "event": "step_update",
            "step_update": {
                "step_index": 4, "state": state, "step_type": "tool",
                "tool_info": {
                    "name": "run_command",
                    "parameters": { "CommandLine": "python3 -c \"print(6*7)\"" },
                    "error": error,
                },
            },
        })
    };
    let mut out = t.translate(&step("ACTIVE", Value::Null));
    out.extend(t.translate(&step(
            "ERROR",
            serde_json::json!({ "message": "permission check failed for command \"python3 -c \\\"print(6*7)\\\"\": user denied permission to run command: python3" }),
        )));
    out.extend(t.translate(&serde_json::json!({
        "event": "result",
        "result": {
            "status": "SUCCESS", "response": "",
            "denied_actions": [{ "action": "command", "display_name": "RunCommand" }],
        },
    })));

    assert!(out.iter().any(|e| matches!(
        e,
        HarnessEvent::ToolFinished { id, ok: false, output, .. }
            if id == "step-4" && output.starts_with("permission check failed")
    )));
    let needed: Vec<_> = out
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::PermissionNeeded {
                tool,
                action,
                target,
                rule,
            } => Some((tool.clone(), action.clone(), target.clone(), rule.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        needed,
        vec![(
            "run_command".to_string(),
            "command".to_string(),
            Some("python3 -c \"print(6*7)\"".to_string()),
            Some("command(python3)".to_string()),
        )]
    );
    assert!(
        matches!(out.last(), Some(HarnessEvent::TurnFinished { status }) if status == "completed")
    );
    assert!(!out.iter().any(|e| matches!(e, HarnessEvent::Error { .. })));
}

/// A deny rule's refusal, verbatim off 1.2.9.
#[test]
fn a_deny_rule_is_not_a_question() {
    let mut t = Translator::default();
    let out = t.translate(&serde_json::json!({
            "event": "step_update",
            "step_update": {
                "step_index": 2, "state": "ERROR", "step_type": "tool",
                "tool_info": {
                    "name": "write_to_file",
                    "parameters": { "TargetFile": "/lib/agents/skills/x.md" },
                    "error": { "type": "TOOL_ERROR", "message": "permission check failed for write_file \"/lib/agents/skills/x.md\": Permission denied for write_file(/lib/agents/skills/x.md). Matches user-configured deny rule." },
                },
            },
        }));
    assert!(out
        .iter()
        .any(|e| matches!(e, HarnessEvent::ToolFinished { ok: false, .. })));
    assert!(!out
        .iter()
        .any(|e| matches!(e, HarnessEvent::PermissionNeeded { .. })));
    assert!(!is_question(
        "permission check failed for unsandboxed \"sqlite3 /lib/oculus.db 'select 1'\": denied"
    ));
    assert!(is_question(
            "permission check failed for command \"python3\": user denied permission to run command: python3"
        ));
}

#[test]
fn a_denied_action_with_no_step_is_still_reported() {
    let mut t = Translator::default();
    let out = t.translate(&serde_json::json!({
        "event": "result",
        "result": { "status": "SUCCESS", "response": "",
                    "denied_actions": [{ "action": "write_file", "display_name": "WriteFile" }] },
    }));
    assert!(out.iter().any(|e| matches!(
            e,
            HarnessEvent::PermissionNeeded { action, target: None, rule: None, .. } if action == "write_file"
        )));
}

#[test]
fn a_refused_file_write_suggests_its_folder() {
    let (action, target, rule) = refusal(
            "write_to_file",
            &serde_json::json!({ "TargetFile": "/Users/s/elsewhere/notes.md" }),
            "permission check failed for write_file \"/Users/s/elsewhere/notes.md\": user denied permission",
        );
    assert_eq!(action, "write_file");
    assert_eq!(target.as_deref(), Some("/Users/s/elsewhere/notes.md"));
    assert_eq!(rule.as_deref(), Some("write_file(/Users/s/elsewhere)"));
}

#[test]
fn a_command_rule_is_its_first_real_word() {
    assert_eq!(command_word("python3 -c 'x'").as_deref(), Some("python3"));
    assert_eq!(
        command_word("FOO=1 BAR_2=x node a.js").as_deref(),
        Some("node")
    );
    assert_eq!(
        command_word("/opt/bin/tool --flag").as_deref(),
        Some("/opt/bin/tool")
    );
    assert_eq!(command_word("  ").as_deref(), None);
}
