//! Terminals, Run and the commit they take, without Ed: frames are checked
//! as text, and the run loop runs against a local WebSocket server.

use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::protocol::CloseFrame;
use tungstenite::Message;

use super::events::{Body, TermState, WorkspaceEvent};
use super::frames::{self, Incoming};
use super::run::frames::{self as run_frames, Exit, RunIncoming};
use super::run::stream::{self, RunEvent, RunOut};
use super::session::Session;
use super::session_tests::{context, go_online, slide};
use super::snapshot::{self, Snapshots};
use super::socket::Out;
use super::terminal::{self, PtySize, Terminals};
use crate::sources::ed::workspace_state::{self, WorkspaceState};
use crate::test_support::Scratch;

const SIZE: PtySize = PtySize { cols: 80, rows: 24 };

fn value(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn init(data: Value) -> frames::Init {
    frames::init(data)
}

#[test]
fn init_lists_persisting_terminals_in_either_shape() {
    let keyed = init(json!({"program_state": {
        "term-1": {"state": "running", "persist": true},
        "term-2": {"persist": false},
        "service-x": {"state": "running"},
    }}));
    assert_eq!(terminal::listed_in(&keyed), ["term-1"]);
    let listed = init(json!({
        "programs": [{"id": "term-7", "command": "bash --login"}, "term-8", {"id": "lsp"}],
        "program_state": {"term-7": {}},
    }));
    assert_eq!(terminal::listed_in(&listed), ["term-7", "term-8"]);
    assert!(terminal::listed_in(&init(json!({}))).is_empty());
}

#[test]
fn a_listed_terminal_is_resumed_once_then_new_ones_are_opened() {
    let mut terms = Terminals::default();
    let (attach, gone) = terms.connected(&init(json!({"program_state": {"term-5": {}}})));
    assert!(attach.is_empty() && gone.is_empty());

    let first = terms.open(SIZE);
    assert_eq!((first.id.as_str(), first.resumed), ("term-5", true));
    let second = terms.open(SIZE);
    assert!(!second.resumed);
    assert!(second.id.starts_with("term-") && second.id != "term-5");

    // A reconnect re-attaches what the pane holds and still runs, at its
    // last size, and reports the one the server lost.
    terms.resized(
        "term-5",
        PtySize {
            cols: 100,
            rows: 30,
        },
    );
    let (attach, gone) = terms.connected(&init(json!({"program_state": {"term-5": {}}})));
    assert_eq!(gone, std::slice::from_ref(&second.id));
    assert_eq!(attach.len(), 1);
    let open = value(&attach[0]);
    assert_eq!(open["data"]["id"], "term-5");
    assert_eq!(open["data"]["pty_size"], json!({"cols": 100, "rows": 30}));
    assert!(!terms.holds(&second.id));

    terms.ended("term-5");
    assert!(!terms.holds("term-5"));
    assert!(!terms.open(SIZE).resumed);
}

#[test]
fn terminal_frames_are_what_eds_web_app_sends() {
    assert_eq!(
        value(&terminal::open_frame("term-3", SIZE)),
        json!({"type": "program_open", "data": {"id": "term-3", "command": "bash --login",
            "pty_size": {"cols": 80, "rows": 24}, "start_if_stopped": true, "persist": true}})
    );
    assert_eq!(
        value(&terminal::input_frame("term-3", "ls\r")),
        json!({"type": "program_data", "data": {"id": "term-3", "data": "bHMN"}})
    );
    assert_eq!(
        value(&terminal::resize_frame("term-3", SIZE)),
        json!({"type": "program_set_pty", "data": {"id": "term-3", "pty_size": {"cols": 80, "rows": 24}}})
    );
    assert_eq!(
        value(&terminal::kill_frame("term-3")),
        json!({"type": "program_signal", "data": {"id": "term-3", "signal": "kill"}})
    );
}

#[test]
fn program_states_map_to_running_or_exited() {
    for word in ["running", "starting", "anything"] {
        assert_eq!(terminal::state_of(&json!(word)), TermState::Running);
    }
    for word in ["exited", "stopped", "killed"] {
        assert_eq!(terminal::state_of(&json!(word)), TermState::Exited);
    }
    assert_eq!(
        terminal::state_of(&json!({"state": "exited"})),
        TermState::Exited
    );
    assert_eq!(terminal::state_of(&Value::Null), TermState::Running);
}

#[test]
fn terminal_frames_from_the_server_become_events_and_offline_is_an_error() {
    let dir = Scratch::new("ed-ws-terminal");
    let (context, events) = context(&dir, "http://127.0.0.1:1");
    let (session, out) = Session::detached(context.clone(), slide());

    let offline = session.terminal_open(SIZE).unwrap_err();
    assert!(offline.contains("offline"), "{offline}");
    assert!(out.try_recv().is_err());

    go_online(&context);
    let opened = session.terminal_open(SIZE).unwrap();
    assert!(!opened.resumed);
    let Ok(Out::Frame(sent)) = out.try_recv() else {
        panic!("no program_open")
    };
    assert_eq!(value(&sent)["type"], "program_open");
    session.terminal_input(&opened.id, "x").unwrap();
    assert!(session.terminal_input("term-0", "x").is_err());

    let parse = |v: Value| frames::parse(&v.to_string()).unwrap();
    let id = opened.id.clone();
    terminal::on_frame(
        &context,
        parse(json!({"type": "program_data", "data": {"id": id, "data": "G1szMm0="}})),
    );
    terminal::on_frame(
        &context,
        parse(json!({"type": "program_data", "data": {"id": "lsp-1", "data": "eA=="}})),
    );
    terminal::on_frame(
        &context,
        parse(json!({"type": "program_close", "data": id})),
    );
    let bodies: Vec<Body> = events
        .lock()
        .unwrap()
        .iter()
        .map(|e| e.body.clone())
        .filter(|b| matches!(b, Body::Term { .. } | Body::TermState { .. }))
        .collect();
    assert_eq!(
        bodies,
        [
            Body::Term {
                id: id.clone(),
                data: "G1szMm0=".into()
            },
            Body::TermState {
                id: id.clone(),
                state: TermState::Exited
            },
        ]
    );
    assert!(session.terminal_input(&id, "x").is_err(), "it ended");
    assert!(matches!(
        frames::parse(
            &json!({"type": "program_update", "data": {"id": "term-1", "state": "exited"}})
                .to_string()
        ),
        Some(Incoming::ProgramUpdate(_))
    ));
}

#[test]
fn events_serialise_with_the_contracts_names() {
    let event = |body| {
        serde_json::to_value(WorkspaceEvent {
            challenge_id: 7,
            body,
        })
        .unwrap()
    };
    assert_eq!(
        event(Body::TermState {
            id: "term-1".into(),
            state: TermState::Running
        }),
        json!({"challengeId": 7, "kind": "termState", "id": "term-1", "state": "running"})
    );
    assert_eq!(
        event(Body::RunExit {
            run_id: "run-1".into(),
            code: Some(0),
            signal: None,
            message: None
        }),
        json!({"challengeId": 7, "kind": "runExit", "runId": "run-1", "code": 0, "signal": null, "message": null})
    );
    assert_eq!(
        event(Body::Run {
            run_id: "run-1".into(),
            data: "aGk=".into()
        }),
        json!({"challengeId": 7, "kind": "run", "runId": "run-1", "data": "aGk="})
    );
    assert_eq!(
        event(Body::Commit {
            hash: "h".into(),
            ours: true
        })["kind"],
        "commit"
    );
}

#[test]
fn a_commit_asked_for_is_answered_by_its_context_id() {
    let mut asked = Snapshots::default();
    let (id, answer) = asked.ask();
    assert_eq!(
        value(&snapshot::commit_frame(id)),
        json!({"type": "commit", "data": {"id": id}})
    );
    assert!(!asked.answered(None, "x"));
    assert!(!asked.answered(Some(&json!({"id": id + 1})), "x"));
    assert!(asked.answered(Some(&json!({"id": id})), "h1"));
    assert_eq!(answer.recv().unwrap(), "h1");
    assert!(!asked.answered(Some(&json!({"id": id})), "h1"), "once");

    let (id, answer) = asked.ask();
    assert!(asked.answered(Some(&json!({"id": id.to_string()})), "h2"));
    assert_eq!(answer.recv().unwrap(), "h2");

    let (_, answer) = asked.ask();
    asked.fail_all();
    assert!(answer.recv().is_err());
}

#[test]
fn run_and_mark_take_the_records_base_else_ask_for_a_commit() {
    let dir = Scratch::new("ed-ws-latest");
    let (context, _) = context(&dir, "http://127.0.0.1:1");
    let (out, queue) = mpsc::channel();

    assert!(snapshot::latest(&context, &out).is_err(), "offline");

    go_online(&context);
    let asker = {
        let context = context.clone();
        std::thread::spawn(move || snapshot::latest(&context, &out))
    };
    let Ok(Out::Frame(frame)) = queue.recv_timeout(Duration::from_secs(5)) else {
        panic!("no commit frame")
    };
    let id = value(&frame)["data"]["id"].clone();
    assert!(context
        .shared()
        .snapshots
        .answered(Some(&json!({ "id": id })), "fresh"));
    assert_eq!(asker.join().unwrap().unwrap(), "fresh");

    let state = WorkspaceState {
        base_commit: Some("base".into()),
        ..WorkspaceState::default()
    };
    workspace_state::save(&dir, 267880, &state).unwrap();
    let (out, queue) = mpsc::channel();
    assert_eq!(snapshot::latest(&context, &out).unwrap(), "base");
    assert!(queue.try_recv().is_err(), "nothing asked");
}

#[test]
fn run_frames_are_read_and_exits_mapped() {
    let parse = |v: Value| run_frames::parse(&v.to_string());
    assert_eq!(parse(json!({"type": "status_created", "data": {}})), None);
    assert_eq!(
        parse(json!({"type": "status_built", "data": {"ok": false, "output": "E: x\n"}})),
        Some(RunIncoming::Built {
            ok: false,
            output: "E: x\n".into()
        })
    );
    assert_eq!(
        parse(json!({"type": "run_frame", "data": {"pid": 1, "type": "stderr", "data": "aGk="}})),
        Some(RunIncoming::Output("aGk=".into()))
    );
    assert_eq!(
        parse(json!({"type": "run_frame", "data": {"pid": 0, "type": "stdin", "data": "aGk="}})),
        None
    );
    assert_eq!(parse(json!({"type": "dump_files", "data": {}})), None);
    assert_eq!(
        parse(
            json!({"type": "run_exit", "data": {"exit_status": {"type": "normal", "value": 3, "wall_time": 0.4}}})
        ),
        Some(RunIncoming::Exit(Exit {
            code: Some(3),
            ..Exit::default()
        }))
    );

    let exit = |v: Value| run_frames::exit_of(&v);
    assert_eq!(
        exit(json!({"type": "signal", "value": 9}))
            .signal
            .as_deref(),
        Some("9")
    );
    assert_eq!(
        exit(json!({"type": "signal", "value": "SIGSEGV"}))
            .signal
            .as_deref(),
        Some("SIGSEGV")
    );
    assert_eq!(
        exit(json!({"type": "killed"})).message.as_deref(),
        Some("The program was killed")
    );
    assert_eq!(
        exit(json!({"type": "exec", "value": "no such file"}))
            .message
            .as_deref(),
        Some("The program could not start: no such file")
    );

    assert_eq!(
        value(&run_frames::stdin_frame("aGk=")),
        json!({"type": "run_frame", "data": {"pid": 0, "type": "stdin", "data": "aGk="}})
    );
    assert_eq!(
        value(&run_frames::size_frame(90, 20)),
        json!({"type": "run_set_size", "data": {"cols": 90, "rows": 20}})
    );
}

/// Serve one run against a local WebSocket server playing `server`, with
/// `queued` already sent by the commands.
fn run_against(
    server: impl FnOnce(&mut tungstenite::WebSocket<TcpStream>) + Send + 'static,
    queued: Vec<RunOut>,
) -> Vec<RunEvent> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let fake = std::thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(tcp).unwrap();
        server(&mut socket);
    });
    let tcp = TcpStream::connect(("127.0.0.1", port)).unwrap();
    tcp.set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();
    let (mut socket, _) = tungstenite::client(format!("ws://127.0.0.1:{port}/run"), tcp).unwrap();
    let (tx, queue) = mpsc::channel();
    for out in queued {
        tx.send(out).unwrap();
    }
    let events = Arc::new(Mutex::new(Vec::new()));
    let log = events.clone();
    stream::serve(&mut socket, &queue, &move |e| log.lock().unwrap().push(e));
    drop(tx);
    fake.join().unwrap();
    let events = events.lock().unwrap().clone();
    events
}

fn read_text(socket: &mut tungstenite::WebSocket<TcpStream>) -> Value {
    loop {
        if let Message::Text(text) = socket.read().unwrap() {
            return value(text.as_str());
        }
    }
}

#[test]
fn a_run_streams_output_takes_stdin_and_ends_with_its_exit_code() {
    let events = run_against(
        |socket| {
            let send = |s: &mut tungstenite::WebSocket<TcpStream>, v: Value| {
                s.send(Message::text(v.to_string())).unwrap()
            };
            send(socket, json!({"type": "status_created", "data": {}}));
            send(
                socket,
                json!({"type": "status_built", "data": {"ok": true, "output": ""}}),
            );
            send(
                socket,
                json!({"type": "run_frame", "data": {"pid": 1, "type": "stdout", "data": "MCANCg=="}}),
            );
            let stdin = read_text(socket);
            assert_eq!(stdin["data"]["type"], "stdin");
            assert_eq!(stdin["data"]["data"], "Kw==");
            send(
                socket,
                json!({"type": "run_exit", "data": {"exit_status": {"type": "normal", "value": 0}}}),
            );
            send(socket, json!({"type": "dump_files", "data": {}}));
            let _ = socket.read();
        },
        vec![RunOut::Frame(run_frames::stdin_frame("Kw=="))],
    );
    assert_eq!(
        events,
        [
            RunEvent::Output("MCANCg==".into()),
            RunEvent::Exit(Exit {
                code: Some(0),
                ..Exit::default()
            }),
        ]
    );
}

#[test]
fn a_full_run_server_closes_1013_and_a_failed_build_shows_its_output() {
    let events = run_against(
        |socket| {
            socket
                .close(Some(CloseFrame {
                    code: CloseCode::from(1013),
                    reason: "".into(),
                }))
                .unwrap();
            let _ = socket.read();
        },
        Vec::new(),
    );
    assert_eq!(
        events,
        [RunEvent::Exit(Exit::because(run_frames::NO_RESOURCES))]
    );

    let events = run_against(
        |socket| {
            socket
                .send(Message::text(
                    json!({"type": "status_built", "data": {"ok": false, "output": "error: x"}})
                        .to_string(),
                ))
                .unwrap();
            let _ = socket.read();
        },
        Vec::new(),
    );
    assert_eq!(
        events,
        [
            RunEvent::Output("ZXJyb3I6IHg=".into()),
            RunEvent::Exit(Exit::because(run_frames::BUILD_FAILED)),
        ]
    );
}

#[test]
fn stopping_a_run_ends_it_with_one_exit() {
    let events = run_against(
        |socket| {
            let _ = socket.read();
        },
        vec![RunOut::Stop],
    );
    assert_eq!(events, [RunEvent::Exit(Exit::because(run_frames::STOPPED))]);
}

#[test]
fn run_needs_a_run_command_and_an_ended_run_takes_no_input() {
    let dir = Scratch::new("ed-ws-run-command");
    let (context, _) = context(&dir, "http://127.0.0.1:1");
    go_online(&context);
    let (session, _out) = Session::detached(context, slide());
    let error = session.run(session.place(), SIZE).unwrap_err();
    assert!(error.contains("no run command"), "{error}");
    assert!(session.run_input("run-1", "x").is_err());
    session.run_stop("run-1");
}
