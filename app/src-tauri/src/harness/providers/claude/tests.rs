//! The Claude bridge's permission prompts, line by line: what becomes a
//! request, what is declined unseen, what an answer writes and what its
//! write proves. Whole recordings are replayed in `testkit/claude_requests.rs`.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use serde_json::{json, Value};

use crate::harness::child::ChildProc;
use crate::harness::requests::{Action, PermissionMode, Reply};
use crate::harness::{HarnessEvent, Sink};

use super::approvals::{DECLINED, DECLINED_UNSEEN};
use super::session::{permission_args, ClaudeSession};
use super::translate::Translator;

/// A session whose writes land in `written` (or fail, with `broken`), and
/// whose events land in the receiver.
struct Rigged {
    session: Arc<ClaudeSession>,
    translator: Translator,
    written: Arc<Mutex<Vec<Value>>>,
    broken: Arc<AtomicBool>,
    sink: Sink,
    events: mpsc::Receiver<HarnessEvent>,
}

fn rigged(one_off: bool) -> Rigged {
    let written: Arc<Mutex<Vec<Value>>> = Arc::default();
    let broken: Arc<AtomicBool> = Arc::default();
    let wire = {
        let (written, broken) = (written.clone(), broken.clone());
        Arc::new(move |frame: Value| {
            if broken.load(Ordering::SeqCst) {
                return Err("claude stdin: Broken pipe (os error 32)".to_string());
            }
            written.lock().unwrap().push(frame);
            Ok(())
        })
    };
    let session = Arc::new(ClaudeSession {
        proc: ChildProc::replay(wire),
        request_ids: AtomicU64::new(1),
        interrupting: Arc::default(),
        expecting: Arc::default(),
        turns: Arc::default(),
        pending: Mutex::new(HashMap::new()),
        background: Arc::default(),
        one_off,
        callbacks: Mutex::new(HashSet::new()),
    });
    let translator = Translator {
        interrupting: session.interrupting.clone(),
        expecting: session.expecting.clone(),
        turns: session.turns.clone(),
        background_count: session.background.clone(),
        ..Default::default()
    };
    let (tx, events) = mpsc::channel();
    let sink: Sink = Arc::new(move |event| {
        let _ = tx.send(event);
    });
    Rigged {
        session,
        translator,
        written,
        broken,
        sink,
        events,
    }
}

impl Rigged {
    fn feed(&mut self, frame: Value) {
        self.session.line(&frame, &mut self.translator, &self.sink);
    }

    fn written(&self) -> Vec<Value> {
        self.written.lock().unwrap().clone()
    }

    /// The events told so far.
    fn told(&self) -> Vec<HarnessEvent> {
        self.events.try_iter().collect()
    }
}

fn prompt(id: &str, request: Value) -> Value {
    let mut request = request;
    request["subtype"] = json!("can_use_tool");
    json!({ "type": "control_request", "request_id": id, "request": request })
}

fn escape(id: &str) -> Value {
    prompt(
        id,
        json!({
            "tool_name": "Bash",
            "input": {
                "command": "touch /elsewhere/out",
                "description": "Touch a marker",
                "dangerouslyDisableSandbox": true,
            },
            "decision_reason_type": "sandboxOverride",
            "tool_use_id": "toolu_bash",
        }),
    )
}

fn asked(
    events: Vec<HarnessEvent>,
) -> (
    crate::harness::requests::NewRequest,
    crate::harness::requests::LiveRoute,
) {
    let mut found = events.into_iter().filter_map(|e| match e {
        HarnessEvent::RequestAsked {
            request,
            route: Some(route),
        } => Some((request, route)),
        _ => None,
    });
    let one = found.next().expect("the prompt became a request");
    assert!(found.next().is_none(), "one request");
    one
}

#[test]
fn a_chat_asks_the_host_in_each_mode_and_a_one_off_turn_asks_nobody() {
    for mode in [PermissionMode::Ask, PermissionMode::Quiet] {
        assert_eq!(
            permission_args(false, mode),
            [
                "--permission-mode",
                "acceptEdits",
                "--permission-prompts",
                "host",
                "--permission-prompt-tool",
                "stdio",
            ]
        );
    }
    assert_eq!(permission_args(false, PermissionMode::Auto)[1], "auto");
    assert_eq!(permission_args(false, PermissionMode::Auto)[3], "host");
    for mode in [
        PermissionMode::Ask,
        PermissionMode::Quiet,
        PermissionMode::Auto,
    ] {
        assert_eq!(
            permission_args(true, mode),
            [
                "--permission-mode",
                "default",
                "--permission-prompts",
                "none"
            ],
            "a one-off turn under {mode:?}"
        );
    }
}

#[test]
fn an_escape_is_a_command_outside_the_sandbox_answered_with_its_own_input() {
    let mut rig = rigged(false);
    rig.feed(escape("req-1"));
    let (request, route) = asked(rig.told());
    assert_eq!(request.action, Action::Command);
    assert_eq!(request.native_ref, "\"req-1\"");
    assert_eq!(request.payload["command"], "touch /elsewhere/out");
    assert_eq!(request.payload["reason"], "Touch a marker");
    assert_eq!(request.payload["outsideSandbox"], true);
    assert_eq!(request.payload["tool"], "toolu_bash");
    assert!(request.paths.is_empty(), "a command names no path to check");
    let ids: Vec<&str> = request.options.iter().map(|o| o.id.as_str()).collect();
    assert_eq!(ids, ["accept", "decline"], "approve once and decline only");
    assert!(rig.written().is_empty(), "it waits for the student");
    assert!(rig.session.has_pending_request());

    assert_eq!(route.0.reply(&request.options[0]), Reply::Acknowledged);
    assert!(!rig.session.has_pending_request());
    assert_eq!(
        rig.written(),
        [json!({
            "type": "control_response",
            "response": {
                "subtype": "success",
                "request_id": "req-1",
                "response": {
                    "behavior": "allow",
                    "updatedInput": {
                        "command": "touch /elsewhere/out",
                        "description": "Touch a marker",
                        "dangerouslyDisableSandbox": true,
                    },
                },
            },
        })]
    );
}

#[test]
fn a_decline_is_a_deny_with_the_students_message() {
    let mut rig = rigged(false);
    rig.feed(escape("req-2"));
    let (request, route) = asked(rig.told());
    assert_eq!(route.0.reply(&request.options[1]), Reply::Acknowledged);
    let reply = &rig.written()[0]["response"];
    assert_eq!(reply["request_id"], "req-2");
    assert_eq!(
        reply["response"],
        json!({ "behavior": "deny", "message": DECLINED })
    );
}

#[test]
fn a_sandboxed_command_the_cli_asks_about_is_not_said_to_leave_the_sandbox() {
    let mut rig = rigged(false);
    rig.feed(prompt(
        "req-3",
        json!({ "tool_name": "Bash", "input": { "command": "ls\nls" }, "tool_use_id": "t" }),
    ));
    let (request, _) = asked(rig.told());
    assert_eq!(request.payload["outsideSandbox"], false);
}

#[test]
fn a_new_host_is_a_network_request_and_its_suggested_grant_is_never_sent() {
    let mut rig = rigged(false);
    rig.feed(prompt(
        "req-4",
        json!({
            "tool_name": "SandboxNetworkAccess",
            "input": { "host": "example.com" },
            "permission_suggestions": [{
                "type": "addRules",
                "rules": [{ "toolName": "WebFetch", "ruleContent": "domain:example.com" }],
                "behavior": "allow",
                "destination": "localSettings",
            }],
            "tool_use_id": "f6bd33eb-84ad-4c25-93f3-a6248943abaf",
            "description": "Allow network connection to example.com?",
        }),
    ));
    let (request, route) = asked(rig.told());
    assert_eq!(request.action, Action::Network);
    assert_eq!(request.payload, json!({ "host": "example.com" }));
    route.0.reply(&request.options[0]);
    let answer = &rig.written()[0]["response"]["response"];
    assert_eq!(
        answer,
        &json!({ "behavior": "allow", "updatedInput": { "host": "example.com" } })
    );
}

#[test]
fn a_write_outside_is_judged_by_its_file_not_the_folder_the_cli_would_grant() {
    let mut rig = rigged(false);
    rig.feed(prompt(
        "req-5",
        json!({
            "tool_name": "Write",
            "input": { "file_path": "/elsewhere/new.txt", "content": "one\ntwo" },
            "permission_suggestions": [
                { "type": "addDirectories", "directories": ["/elsewhere"], "destination": "session" },
            ],
            "decision_reason": "Path is outside allowed working directories",
            "decision_reason_type": "workingDir",
            "tool_use_id": "toolu_write",
        }),
    ));
    let (request, _) = asked(rig.told());
    assert_eq!(request.action, Action::FileChange);
    assert_eq!(request.paths, ["/elsewhere/new.txt"]);
    let change = &request.payload["changes"][0];
    assert_eq!(change["path"], "/elsewhere/new.txt");
    assert_eq!(change["kind"], "add");
    assert_eq!(change["diff"], "+one\n+two\n");
    assert_eq!(request.payload["tool"], "toolu_write");
    assert!(request.payload.get("outsideSandbox").is_none());
}

#[test]
fn an_edit_shows_what_it_replaces() {
    let mut rig = rigged(false);
    rig.feed(prompt(
        "req-6",
        json!({
            "tool_name": "Edit",
            "input": { "file_path": "/elsewhere/a.md", "old_string": "x", "new_string": "y" },
            "tool_use_id": "t",
        }),
    ));
    let (request, _) = asked(rig.told());
    assert_eq!(request.payload["changes"][0]["diff"], "-x\n+y\n");
    assert_eq!(request.paths, ["/elsewhere/a.md"]);
}

#[test]
fn a_question_and_a_file_tool_naming_no_file_are_declined_unseen() {
    let mut rig = rigged(false);
    rig.feed(prompt(
        "q",
        json!({ "tool_name": "AskUserQuestion", "input": { "questions": [] } }),
    ));
    rig.feed(prompt(
        "w",
        json!({ "tool_name": "Write", "input": { "content": "x" } }),
    ));
    assert!(rig.told().is_empty(), "nobody is asked");
    assert!(!rig.session.has_pending_request());
    let written = rig.written();
    assert_eq!(written.len(), 2);
    for (frame, id) in written.iter().zip(["q", "w"]) {
        assert_eq!(frame["response"]["request_id"], id);
        assert_eq!(
            frame["response"]["response"],
            json!({ "behavior": "deny", "message": DECLINED_UNSEEN })
        );
    }
}

/// The `tool` request a prompt for `tool` with `input` (and any of the
/// prompt's other fields in `extra`) becomes.
fn tool_request(tool: &str, input: Value, extra: Value) -> crate::harness::requests::NewRequest {
    let mut rig = rigged(false);
    let mut request = json!({ "tool_name": tool, "input": input, "tool_use_id": "toolu_t" });
    for (k, v) in extra.as_object().unwrap() {
        request[k] = v.clone();
    }
    rig.feed(prompt("req-t", request));
    let (request, _) = asked(rig.told());
    assert_eq!(request.action, Action::Tool, "{tool}");
    assert_eq!(request.payload["tool"], "toolu_t", "{tool}: the call's id");
    assert!(request.paths.is_empty(), "{tool}: nothing to check");
    let ids: Vec<&str> = request.options.iter().map(|o| o.id.as_str()).collect();
    assert_eq!(ids, ["accept", "decline"], "{tool}");
    request
}

#[test]
fn a_read_outside_is_asked_with_the_path_it_reads() {
    let read = tool_request(
        "Read",
        json!({ "file_path": "/elsewhere/notes.md" }),
        json!({ "decision_reason_type": "workingDir" }),
    );
    assert_eq!(read.payload["name"], "Read");
    assert_eq!(read.payload["kind"], "read");
    assert_eq!(read.payload["path"], "/elsewhere/notes.md");
    assert_eq!(read.payload["pattern"], Value::Null);

    let grep = tool_request(
        "Grep",
        json!({ "pattern": "TODO", "path": "/elsewhere" }),
        json!({}),
    );
    assert_eq!(
        (&grep.payload["path"], &grep.payload["pattern"]),
        (&json!("/elsewhere"), &json!("TODO"))
    );

    let glob = tool_request(
        "Glob",
        json!({ "pattern": "**/*.md" }),
        json!({ "blocked_path": "/elsewhere/docs" }),
    );
    assert_eq!(
        glob.payload["path"], "/elsewhere/docs",
        "no path of its own: the one the CLI says it blocked"
    );
}

#[test]
fn a_web_fetch_names_its_url_and_what_it_asks_of_the_page() {
    let fetch = tool_request(
        "WebFetch",
        json!({ "url": "https://example.com/", "prompt": "Return the page title." }),
        json!({}),
    );
    assert_eq!(
        fetch.payload,
        json!({
            "name": "WebFetch",
            "kind": "fetch",
            "url": "https://example.com/",
            "prompt": "Return the page title.",
            "tool": "toolu_t",
        })
    );
}

#[test]
fn a_web_search_names_its_query() {
    let search = tool_request(
        "WebSearch",
        json!({ "query": "rust borrow checker", "allowed_domains": ["rust-lang.org"] }),
        json!({}),
    );
    assert_eq!(
        search.payload,
        json!({ "name": "WebSearch", "kind": "search", "query": "rust borrow checker", "tool": "toolu_t" })
    );
}

#[test]
fn an_mcp_tool_names_its_server_its_tool_and_its_arguments() {
    let args = json!({ "owner": "o", "repo": "r", "title": "Bug" });
    let call = tool_request("mcp__github__create_issue", args.clone(), json!({}));
    assert_eq!(call.payload["kind"], "mcp");
    assert_eq!(call.payload["name"], "mcp__github__create_issue");
    assert_eq!(
        call.payload["mcp"],
        json!({ "server": "github", "tool": "create_issue" })
    );
    assert_eq!(call.payload["arguments"], args);

    // The server as configured, when the prompt names it; its tool name
    // carries it normalised.
    let named = tool_request(
        "mcp__my_notes__search__all",
        json!({}),
        json!({ "mcp_server": { "name": "my_notes", "source": "user" } }),
    );
    assert_eq!(
        named.payload["mcp"],
        json!({ "server": "my_notes", "tool": "search__all" })
    );
    let spaced = tool_request(
        "mcp__my_notes__search",
        json!({}),
        json!({ "mcp_server": { "name": "my notes", "source": "user" } }),
    );
    assert_eq!(
        spaced.payload["mcp"],
        json!({ "server": "my notes", "tool": "search" })
    );
}

#[test]
fn any_other_tool_is_asked_by_name_with_its_arguments() {
    let other = tool_request("Skill", json!({ "skill": "oculus-plan" }), json!({}));
    assert_eq!(
        other.payload,
        json!({
            "name": "Skill",
            "kind": "other",
            "arguments": { "skill": "oculus-plan" },
            "tool": "toolu_t",
        })
    );
}

#[test]
fn an_approved_tool_call_is_allowed_with_its_input_as_sent() {
    let mut rig = rigged(false);
    let input = json!({ "url": "https://example.com/", "prompt": "Title?" });
    rig.feed(prompt(
        "req-f",
        json!({ "tool_name": "WebFetch", "input": input, "tool_use_id": "toolu_f" }),
    ));
    let (request, route) = asked(rig.told());
    assert_eq!(route.0.reply(&request.options[0]), Reply::Acknowledged);
    assert_eq!(
        rig.written()[0]["response"],
        json!({
            "subtype": "success",
            "request_id": "req-f",
            "response": { "behavior": "allow", "updatedInput": input },
        })
    );
}

#[test]
fn a_one_off_turn_declines_every_prompt_unseen() {
    let mut rig = rigged(true);
    rig.feed(escape("req-7"));
    assert!(rig.told().is_empty());
    assert_eq!(
        rig.written()[0]["response"]["response"]["behavior"],
        "deny",
        "a stray prompt is refused rather than left hanging"
    );
    assert!(!rig.session.has_pending_request());
}

#[test]
fn a_prompt_while_the_turn_is_being_stopped_is_declined_unseen() {
    let mut rig = rigged(false);
    rig.session.interrupting.store(true, Ordering::SeqCst);
    rig.feed(escape("req-8"));
    assert!(rig.told().is_empty());
    assert_eq!(rig.written()[0]["response"]["response"]["behavior"], "deny");
}

#[test]
fn a_control_request_that_is_no_prompt_is_refused() {
    let mut rig = rigged(false);
    rig.feed(json!({
        "type": "control_request",
        "request_id": "e1",
        "request": { "subtype": "elicitation", "mcp_server_name": "x" },
    }));
    assert!(rig.told().is_empty());
    let refused = &rig.written()[0]["response"];
    assert_eq!(refused["subtype"], "error");
    assert_eq!(refused["request_id"], "e1");
}

#[test]
fn a_withdrawal_retires_the_callback_unless_our_stop_caused_it() {
    let mut rig = rigged(false);
    rig.feed(escape("req-9"));
    let _ = rig.told();
    rig.feed(json!({ "type": "control_cancel_request", "request_id": "req-9" }));
    assert!(matches!(
        rig.told().as_slice(),
        [HarnessEvent::RequestRetired { native_ref }] if native_ref == "\"req-9\""
    ));
    assert!(!rig.session.has_pending_request());
    // Withdrawn after our interrupt: the turn's end cancels it as stopped.
    rig.feed(escape("req-10"));
    let _ = rig.told();
    rig.session.interrupting.store(true, Ordering::SeqCst);
    rig.feed(json!({ "type": "control_cancel_request", "request_id": "req-10" }));
    assert!(rig.told().is_empty());
    assert!(!rig.session.has_pending_request());
    // One nobody raised, or one already answered, changes nothing.
    rig.feed(json!({ "type": "control_cancel_request", "request_id": "never" }));
    assert!(rig.told().is_empty());
    assert!(rig.written().is_empty(), "a withdrawal is never answered");
}

#[test]
fn a_pipe_that_breaks_mid_reply_is_uncertain_and_a_gone_process_writes_nothing() {
    let mut rig = rigged(false);
    rig.feed(escape("req-11"));
    rig.feed(escape("req-12"));
    let mut both = rig.told().into_iter().filter_map(|e| match e {
        HarnessEvent::RequestAsked {
            request,
            route: Some(route),
        } => Some((request, route)),
        _ => None,
    });
    let (first, first_route) = both.next().unwrap();
    let (second, second_route) = both.next().unwrap();
    rig.broken.store(true, Ordering::SeqCst);
    assert!(matches!(
        first_route.0.reply(&first.options[0]),
        Reply::Uncertain(e) if e.contains("Broken pipe")
    ));
    rig.broken.store(false, Ordering::SeqCst);
    rig.session.kill();
    assert!(matches!(
        second_route.0.reply(&second.options[0]),
        Reply::Failed(_)
    ));
    assert!(rig.written().is_empty(), "nothing reached the CLI");
}

#[test]
fn a_subagents_prompt_names_the_agent_its_task_started() {
    let mut rig = rigged(false);
    rig.feed(json!({
        "type": "system",
        "subtype": "task_started",
        "task_id": "ab7e769d9c5be9cc7",
        "tool_use_id": "toolu_agent",
        "description": "Wire probe foreground touch",
        "subagent_type": "wire-probe",
        "is_backgrounded": false,
        "task_type": "local_agent",
    }));
    let mut child = escape("req-13");
    child["request"]["agent_id"] = json!("ab7e769d9c5be9cc7");
    rig.feed(child);
    let mut unknown = escape("req-14");
    unknown["request"]["agent_id"] = json!("not-seen");
    rig.feed(unknown);
    let told = rig.told();
    let agents: Vec<Value> = told
        .into_iter()
        .filter_map(|e| match e {
            HarnessEvent::RequestAsked { request, .. } => Some(request.payload["agent"].clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        agents[0],
        json!({
            "path": "wire-probe",
            "tool": "toolu_agent",
            "description": "Wire probe foreground touch",
        })
    );
    assert_eq!(agents[1]["path"], "not-seen", "still a subagent's");
}

#[test]
fn the_classifier_and_a_deny_rule_are_shown_on_the_call_and_the_rest_is_not() {
    let mut rig = rigged(false);
    let result = |id: &str, decision: Value| {
        json!({
            "type": "user",
            "message": { "role": "user", "content": [
                { "tool_use_id": id, "type": "tool_result", "content": "done", "is_error": false },
            ]},
            "parent_tool_use_id": null,
            "tool_result_meta": [{ "id": id, "permission_decision": decision }],
        })
    };
    rig.feed(result(
        "a",
        json!({ "decision": "accept", "source": "config", "reason_type": "classifier" }),
    ));
    rig.feed(result(
        "b",
        json!({ "decision": "reject", "source": "config", "reason_type": "rule" }),
    ));
    rig.feed(result(
        "c",
        json!({ "decision": "accept", "source": "user_temporary" }),
    ));
    rig.feed(result(
        "d",
        json!({ "decision": "accept", "source": "config", "reason_type": "mode" }),
    ));
    let reviews: Vec<(String, String, Option<String>)> = rig
        .told()
        .into_iter()
        .filter_map(|e| match e {
            HarnessEvent::ToolReviewed { id, review } => Some((id, review.status, review.source)),
            _ => None,
        })
        .collect();
    assert_eq!(
        reviews,
        [
            ("a".into(), "approved".into(), Some("classifier".into())),
            ("b".into(), "denied".into(), Some("rule".into())),
        ]
    );
}

#[test]
fn a_one_off_question_is_denied_even_if_a_callback_arrives() {
    let mut rig = rigged(true);
    rig.feed(prompt("question", json!({"tool_name":"AskUserQuestion","input":{"questions":[{
        "question":"Choose?","header":"Color","options":[{"label":"Red","description":"Warm"}],"multiSelect":false
    }]}})));
    assert!(rig.told().is_empty());
    assert_eq!(rig.written()[0]["response"]["response"]["behavior"], "deny");
    assert!(!rig.session.has_pending_request());
}
