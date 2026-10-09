use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::AtomicI64;
use std::sync::Mutex;

use serde_json::Value;

use crate::harness::child::ChildProc;
use crate::harness::event::{HarnessEvent, ToolKind};

use super::translate::{translate, translate_account};
use super::{CodexServer, CodexThreadOpts, ThreadState};

#[cfg(unix)]
#[test]
fn a_failed_request_write_does_not_leave_a_pending_reply() {
    use std::io::{BufRead, BufReader};

    // Close stdin but keep the process alive, so the request exercises a
    // broken pipe rather than the early "server is not running" check.
    let mut command = Command::new("sh");
    command.args(["-c", "exec 0<&-; printf 'ready\\n'; exec sleep 10"]);
    let (proc, stdout) = ChildProc::spawn("codex", &mut command, true).unwrap();
    let mut ready = String::new();
    BufReader::new(stdout).read_line(&mut ready).unwrap();
    assert_eq!(ready.trim(), "ready");
    let server = CodexServer {
        proc,
        next_id: AtomicI64::new(1),
        pending: Mutex::new(HashMap::new()),
        routes: Mutex::new(HashMap::new()),
        account_sink: None,
    };
    let result = server.request("test", Value::Null);
    server.proc.kill();
    assert!(matches!(result, Err(error) if error.starts_with("codex stdin:")));
    assert!(server.pending.lock().unwrap().is_empty());
}

#[test]
fn a_thread_may_write_the_database_and_nothing_else_outside_its_cwd() {
    let library = std::path::Path::new("/Users/x/Library/Application Support/com.tchan.oculus");
    let opts = CodexThreadOpts {
        cwd: library.join("agents"),
        writable_files: crate::library::paths::db_write_paths(library),
        model: Some("gpt-5.3-codex".into()),
        reasoning_effort: Some("high".into()),
        instructions: String::new(),
        ephemeral: false,
    };
    let want = serde_json::json!([
        "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db",
        "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db-wal",
        "/Users/x/Library/Application Support/com.tchan.oculus/oculus.db-shm",
    ]);

    let thread = CodexServer::thread_params(&opts);
    assert_eq!(thread["sandbox"], "workspace-write");
    assert_eq!(
        thread["cwd"],
        library.join("agents").to_string_lossy().as_ref()
    );
    assert_eq!(
        thread["config"]["sandbox_workspace_write.writable_roots"],
        want
    );

    let turn = CodexServer::turn_params("t1", "hello", &opts);
    assert_eq!(turn["sandboxPolicy"]["writableRoots"], want);
    assert_eq!(turn["sandboxPolicy"]["type"], "workspaceWrite");
    assert_eq!(
        turn["approvalPolicy"], "never",
        "a prompt has nowhere to go"
    );
}

/// Recorded (0.153.4): a finished `webSearch` has no `status`, and
/// `item/started` leaves its query empty.
#[test]
fn a_web_search_that_worked_is_not_drawn_as_a_failure() {
    let raw = include_str!("../../../../fixtures/harness/codex-websearch.ndjson");
    let mut st = ThreadState::default();
    let mut events = Vec::new();
    for line in raw.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(method) = v.get("method").and_then(|m| m.as_str()) else {
            continue;
        };
        events.extend(translate(method, &v["params"], &mut st));
    }
    assert!(matches!(
        events.first(),
        Some(HarnessEvent::ToolStarted { kind: ToolKind::Web, name, .. }) if name == "webSearch"
    ));
    let Some(HarnessEvent::ToolFinished {
        ok, output, title, ..
    }) = events.last()
    else {
        panic!("no finish: {events:?}");
    };
    assert!(*ok, "a search with results is not a failure");
    let title = title.as_deref().unwrap_or_default();
    assert!(
        title.starts_with("site:torproject.org bridges obfs4"),
        "{title}"
    );
    assert!(title.ends_with("(+3 more)"), "{title}");
    assert_eq!(output.matches("search: ").count(), 4);
    assert!(
        output.contains("https://support.torproject.org/little-t-tor/circumvention/using-bridges/")
    );
}

/// Recorded from `codex app-server` 0.153.4 asked to `ls` the library.
#[test]
fn folds_a_recorded_thread() {
    let raw = include_str!("../../../../fixtures/harness/codex-ls.ndjson");
    let mut st = ThreadState::default();
    let mut events = Vec::new();
    for line in raw.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(method) = v.get("method").and_then(|m| m.as_str()) else {
            continue;
        };
        // As `handle_notification` routes them.
        let account = translate_account(method, &v["params"]);
        if !account.is_empty() {
            events.extend(account);
            continue;
        }
        events.extend(translate(method, &v["params"], &mut st));
    }
    assert!(matches!(events.first(), Some(HarnessEvent::TurnStarted)));
    let tools: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::ToolStarted { kind, title, .. } => Some((*kind, title.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(tools, vec![(ToolKind::Bash, "/bin/zsh -lc ls".to_string())]);
    assert!(events.iter().any(|e| matches!(e, HarnessEvent::ToolFinished { ok: true, output, .. } if output.contains("oculus.db"))));
    let messages = events
        .iter()
        .filter(|e| matches!(e, HarnessEvent::AssistantMessage { .. }))
        .count();
    assert_eq!(messages, 2);
    assert!(events.iter().any(|e| matches!(
        e,
        HarnessEvent::Usage {
            context_window: Some(_),
            ..
        }
    )));
    assert!(events
        .iter()
        .any(|e| matches!(e, HarnessEvent::RateLimits { windows } if windows.len() == 2)));
    assert!(
        matches!(events.last(), Some(HarnessEvent::TurnFinished { status }) if status == "completed")
    );
}

/// Recorded: an interrupted turn never sends the `item/completed`.
#[test]
fn a_stopped_turn_keeps_what_was_said() {
    let raw = include_str!("../../../../fixtures/harness/codex-interrupt.ndjson");
    let mut st = ThreadState::default();
    let mut events = Vec::new();
    for line in raw.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(method) = v.get("method").and_then(|m| m.as_str()) else {
            continue;
        };
        events.extend(translate(method, &v["params"], &mut st));
    }
    let deltas: String = events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::AssistantDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(!deltas.is_empty(), "the fixture streams an answer");
    let message = events.iter().find_map(|e| match e {
        HarnessEvent::AssistantMessage { text } => Some(text.clone()),
        _ => None,
    });
    assert_eq!(
        message.as_deref(),
        Some(deltas.as_str()),
        "the partial is committed as a row"
    );
    assert!(
        matches!(events.last(), Some(HarnessEvent::TurnFinished { status }) if status == "interrupted")
    );
}
