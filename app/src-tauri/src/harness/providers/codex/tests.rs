use serde_json::Value;

use crate::harness::event::{HarnessEvent, ToolKind};

use super::translate::{translate, translate_account};
use super::{CodexServer, CodexThreadOpts, ThreadState};
use crate::harness::requests::PermissionMode;

#[cfg(unix)]
#[test]
fn a_failed_request_write_does_not_leave_a_pending_reply() {
    use crate::harness::child::ChildProc;
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Read};
    use std::process::Command;
    use std::sync::atomic::AtomicI64;
    use std::sync::{mpsc, Mutex};
    use std::time::Duration;

    // The child prints one line and exits, so its stdin's read end is
    // gone and the write meets a broken pipe. `kill` and `reap` would
    // mark the server dead and take the early "not running" path, so the
    // exit is awaited through stdout's EOF instead. A child that held
    // stdin open and slept could instead inherit a pipe end another
    // thread's concurrent spawn leaked, and the write would succeed.
    let mut command = Command::new("sh");
    command.args(["-c", "printf 'ready\n'"]);
    let (proc, stdout) = ChildProc::spawn("codex", &mut command, true).unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut ready = String::new();
        let first = reader.read_line(&mut ready).map(|_| ready);
        let mut rest = Vec::new();
        let _ = reader.read_to_end(&mut rest);
        let _ = tx.send(first);
    });
    let ready = rx
        .recv_timeout(Duration::from_secs(60))
        .expect("the child neither answered nor exited within 60s")
        .unwrap();
    assert_eq!(ready.trim(), "ready");
    let server = CodexServer {
        proc,
        next_id: AtomicI64::new(1),
        pending: Mutex::new(HashMap::new()),
        routes: Mutex::new(HashMap::new()),
        children: Mutex::new(HashMap::new()),
        account_sink: None,
        me: std::sync::Weak::new(),
    };
    let result = server.request("test", Value::Null);
    server.proc.kill();
    assert!(matches!(result, Err(error) if error.starts_with("codex stdin:")));
    assert!(server.pending.lock().unwrap().is_empty());
}

#[test]
fn a_thread_may_write_the_database_and_studio_and_nothing_else_outside_its_cwd() {
    let library = crate::test_support::Scratch::new("codex-roots");
    let db = crate::library::paths::db_path(&library);
    std::fs::write(&db, "").unwrap();
    // The WAL sidecars do not exist yet, so they are left out; studio/ is.
    std::fs::create_dir_all(library.join("studio")).unwrap();
    let opts = CodexThreadOpts {
        cwd: library.join("agents"),
        writable_roots: super::writable_roots(&library),
        model: Some("gpt-5.3-codex".into()),
        reasoning_effort: Some("high".into()),
        instructions: String::new(),
        ephemeral: false,
        mode: PermissionMode::Quiet,
        library: library.to_path_buf(),
    };
    let want = serde_json::json!([db, library.join("studio")]);
    for dir in crate::library::paths::agent_write_dirs(&library) {
        assert!(opts.writable_roots.contains(&dir), "{dir:?}");
    }

    let thread = CodexServer::thread_params(&opts);
    assert_eq!(thread["sandbox"], "workspace-write");
    assert_eq!(
        thread["config"]["shell_environment_policy.set"]["OCULUS_AGENT"],
        "1"
    );
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

/// `oculus` reaches keyd's Unix socket from inside Codex's sandbox only
/// while the workspace-write policy has network access (measured: the
/// connect gets EPERM without it); Codex has no per-socket grant.
#[test]
fn every_thread_and_turn_keeps_the_sandbox_network_open() {
    for (model, effort, instructions, ephemeral) in [
        (Some("gpt-5.3-codex"), Some("high"), "brief", false),
        (None, None, "", true),
    ] {
        let opts = CodexThreadOpts {
            cwd: "/lib/agents".into(),
            writable_roots: Vec::new(),
            model: model.map(String::from),
            reasoning_effort: effort.map(String::from),
            instructions: instructions.into(),
            ephemeral,
            mode: PermissionMode::Quiet,
            library: "/lib".into(),
        };
        let thread = CodexServer::thread_params(&opts);
        assert_eq!(thread["sandbox"], "workspace-write");
        assert_eq!(
            thread["config"]["shell_environment_policy.set"]["OCULUS_AGENT"],
            "1"
        );
        assert_eq!(
            thread["config"]["sandbox_workspace_write.network_access"],
            true
        );
        let turn = CodexServer::turn_params("t", "hi", &opts);
        assert_eq!(turn["sandboxPolicy"]["type"], "workspaceWrite");
        assert_eq!(turn["sandboxPolicy"]["networkAccess"], true);
    }
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
    assert!(matches!(
        events.first(),
        Some(HarnessEvent::TurnStarted { .. })
    ));
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
        matches!(events.last(), Some(HarnessEvent::TurnFinished { status, .. }) if status == "completed")
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
            HarnessEvent::AssistantDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(!deltas.is_empty(), "the fixture streams an answer");
    let message = events.iter().find_map(|e| match e {
        HarnessEvent::AssistantMessage { text, .. } => Some(text.clone()),
        _ => None,
    });
    assert_eq!(
        message.as_deref(),
        Some(deltas.as_str()),
        "the partial is committed as a row"
    );
    assert!(
        matches!(events.last(), Some(HarnessEvent::TurnFinished { status, .. }) if status == "interrupted")
    );
}

/// A `turn/completed` for another turn (a repeat of the last one, or a turn
/// the server ran itself) names that turn and leaves the open one alone.
#[test]
fn another_turns_end_leaves_the_open_turn_as_it_is() {
    let mut st = ThreadState::default();
    let turn =
        |id: &str| serde_json::json!({"threadId": "t", "turn": {"id": id, "status": "completed"}});
    let started = translate("turn/started", &turn("now"), &mut st);
    assert!(matches!(
        started.as_slice(),
        [HarnessEvent::TurnStarted { turn: Some(t) }] if t == "now"
    ));
    translate(
        "item/agentMessage/delta",
        &serde_json::json!({"threadId": "t", "itemId": "m1", "delta": "half an ans"}),
        &mut st,
    );

    let stray = translate("turn/completed", &turn("before"), &mut st);
    assert!(
        matches!(
            stray.as_slice(),
            [HarnessEvent::TurnFinished { turn: Some(t), .. }] if t == "before"
        ),
        "only the end it names: {stray:?}"
    );
    assert_eq!(st.active_turn.as_deref(), Some("now"));
    assert_eq!(st.partial_message, "half an ans", "the streamed text stays");

    let own = translate("turn/completed", &turn("now"), &mut st);
    assert!(matches!(
        own.last(),
        Some(HarnessEvent::TurnFinished { turn: Some(t), .. }) if t == "now"
    ));
    assert!(st.active_turn.is_none());
}

/// A server whose writes are captured, with one routed chat (`chat`) in
/// `mode`, and what that chat's sink receives.
fn captured(
    mode: PermissionMode,
) -> (
    std::sync::Arc<CodexServer>,
    std::sync::Arc<std::sync::Mutex<Vec<Value>>>,
    std::sync::mpsc::Receiver<HarnessEvent>,
) {
    use crate::harness::child::ChildProc;
    use std::collections::HashMap;
    use std::sync::atomic::AtomicI64;
    use std::sync::{Arc, Mutex};
    let written: Arc<Mutex<Vec<Value>>> = Arc::default();
    let wire = written.clone();
    let server = Arc::new_cyclic(|me| CodexServer {
        proc: ChildProc::replay(Arc::new(move |frame: Value| {
            wire.lock().unwrap().push(frame);
            Ok(())
        })),
        next_id: AtomicI64::new(1),
        pending: Mutex::new(HashMap::new()),
        routes: Mutex::new(HashMap::new()),
        children: Mutex::new(HashMap::new()),
        account_sink: None,
        me: me.clone(),
    });
    let (tx, events) = std::sync::mpsc::channel();
    server.route(
        "chat",
        Arc::new(move |event| {
            let _ = tx.send(event);
        }),
        false,
    );
    server.routes.lock().unwrap()["chat"]
        .state
        .lock()
        .unwrap()
        .mode = mode;
    (server, written, events)
}

fn ask(server: &CodexServer, id: Value, params: Value) {
    let mut params = params;
    params["threadId"] = serde_json::json!("chat");
    server.dispatch(serde_json::json!({
        "id": id, "method": "item/commandExecution/requestApproval", "params": params,
    }));
}

#[test]
fn a_string_callback_id_is_kept_as_sent_and_answered_on_it() {
    let (server, written, events) = captured(PermissionMode::Ask);
    ask(
        &server,
        serde_json::json!("call-7"),
        serde_json::json!({ "command": "touch x", "cwd": "/w", "itemId": "exec-1" }),
    );
    let HarnessEvent::RequestAsked {
        request,
        route: Some(route),
    } = events.try_recv().unwrap()
    else {
        panic!("the callback became a request");
    };
    assert_eq!(request.native_ref, "\"call-7\"");
    assert!(
        written.lock().unwrap().is_empty(),
        "it waits for the student"
    );
    assert!(server.has_pending_request("chat"));
    let accept = &request.options[0];
    assert_eq!(
        route.0.reply(accept),
        crate::harness::requests::Reply::Written
    );
    let reply = written.lock().unwrap().pop().unwrap();
    assert_eq!(reply["id"], "call-7");
    assert_eq!(reply["result"]["decision"], "accept");
    assert!(!server.has_pending_request("chat"));
    server.handle_notification(
        "serverRequest/resolved",
        &serde_json::json!({ "threadId": "chat", "requestId": "call-7" }),
    );
    assert!(matches!(
        events.try_recv().unwrap(),
        HarnessEvent::RequestRetired { native_ref } if native_ref == "\"call-7\""
    ));
}

#[test]
fn a_dont_ask_chat_and_an_unrouted_thread_are_declined_without_a_request() {
    let (server, written, events) = captured(PermissionMode::Quiet);
    ask(
        &server,
        serde_json::json!(1),
        serde_json::json!({ "command": "rm x" }),
    );
    server.dispatch(serde_json::json!({
        "id": 2, "method": "item/fileChange/requestApproval",
        "params": { "threadId": "nobody", "itemId": "f" },
    }));
    assert!(events.try_recv().is_err());
    let replies = written.lock().unwrap().clone();
    assert_eq!(replies[0]["result"]["decision"], "decline");
    assert_eq!(replies[1]["result"]["decision"], "decline");
    assert!(!server.has_pending_request("chat"));
}

#[test]
fn a_file_change_whose_item_is_unknown_is_declined_and_a_known_one_names_its_paths() {
    let (server, written, events) = captured(PermissionMode::Ask);
    server.dispatch(serde_json::json!({
        "id": 3, "method": "item/fileChange/requestApproval",
        "params": { "threadId": "chat", "itemId": "unseen" },
    }));
    assert!(events.try_recv().is_err());
    assert_eq!(written.lock().unwrap()[0]["result"]["decision"], "decline");
    server.handle_notification(
        "item/started",
        &serde_json::json!({ "threadId": "chat", "item": {
            "type": "fileChange", "id": "f1",
            "changes": [{ "path": "/lib/courses/a.md", "kind": { "type": "update", "move_path": "/lib/b.md" }, "diff": "+x" }],
        }}),
    );
    let _ = events.try_recv();
    server.dispatch(serde_json::json!({
        "id": 4, "method": "item/fileChange/requestApproval",
        "params": { "threadId": "chat", "itemId": "f1" },
    }));
    let HarnessEvent::RequestAsked { request, .. } = events.try_recv().unwrap() else {
        panic!("a request");
    };
    assert_eq!(request.paths, ["/lib/courses/a.md", "/lib/b.md"]);
    assert_eq!(request.payload["changes"][0]["diff"], "+x");
}

#[test]
fn a_childs_request_reaches_its_parents_chat_with_its_agent_path() {
    let (server, _, events) = captured(PermissionMode::Ask);
    server.handle_notification(
        "item/started",
        &serde_json::json!({ "threadId": "chat", "item": {
            "type": "subAgentActivity", "agentThreadId": "child", "agentPath": "/root/helper",
        }}),
    );
    while events.try_recv().is_ok() {}
    server.dispatch(serde_json::json!({
        "id": 0, "method": "item/commandExecution/requestApproval",
        "params": { "threadId": "child", "command": "ls", "itemId": "e" },
    }));
    let HarnessEvent::RequestAsked { request, .. } = events.try_recv().unwrap() else {
        panic!("a request");
    };
    assert_eq!(request.payload["agent"]["path"], "/root/helper");
    assert!(server.has_pending_request("chat"));
}

#[test]
fn each_mode_is_sent_on_every_turn_and_one_off_turns_never_ask() {
    let opts = |mode| CodexThreadOpts {
        cwd: "/lib/agents".into(),
        writable_roots: Vec::new(),
        model: None,
        reasoning_effort: None,
        instructions: String::new(),
        ephemeral: false,
        mode,
        library: "/lib".into(),
    };
    for (mode, policy, reviewer) in [
        (PermissionMode::Ask, "on-request", Some("user")),
        (PermissionMode::Auto, "on-request", Some("auto_review")),
        (PermissionMode::Quiet, "never", None),
    ] {
        let opts = opts(mode);
        for params in [
            CodexServer::thread_params(&opts),
            CodexServer::turn_params("t", "hi", &opts),
        ] {
            assert_eq!(params["approvalPolicy"], policy, "{mode:?}");
            assert_eq!(params["approvalsReviewer"].as_str(), reviewer, "{mode:?}");
        }
        let thread = CodexServer::thread_params(&opts);
        // `OCULUS_AGENT` refuses its verbs inside an approved command too.
        assert_eq!(
            thread["config"]["shell_environment_policy.set"]["OCULUS_AGENT"],
            "1"
        );
        assert_eq!(
            thread["config"]["features.request_permissions_tool"].as_bool(),
            (mode != PermissionMode::Quiet).then_some(true)
        );
    }
}
