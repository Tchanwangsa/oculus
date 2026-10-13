//! A run's `dump_files`: Ed's filter, the value shapes it is decoded from,
//! what the pane gets, and the run waiting for it after `run_exit`.

use std::net::TcpStream;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use serde_json::{json, Value};
use tungstenite::Message;

use super::events::{Body, RunFile, WorkspaceEvent};
use super::program_tests::run_against;
use super::run::dump::{self, Dumped, INLINE_LIMIT};
use super::run::frames::Exit;
use super::run::stream::RunEvent;
use crate::test_support::Scratch;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

fn send(socket: &mut tungstenite::WebSocket<TcpStream>, frame: Value) {
    socket.send(Message::text(frame.to_string())).unwrap();
}

#[test]
fn eds_filter_hides_sources_dotfiles_and_names_without_a_dot() {
    for shown in [
        "graph.png",
        "out/plot.svg",
        "data.csv",
        "a.out",
        "report.txt",
        "dfa.gv.pdf",
    ] {
        assert!(dump::shown(shown), "{shown} is hidden");
    }
    for hidden in [
        "main.py",
        "A.java",
        "lib.o",
        "run.sh",
        "x.",
        ".hidden.png",
        ".bash_history",
        "program",
        "Makefile",
    ] {
        assert!(!dump::shown(hidden), "{hidden} is shown");
    }
}

#[test]
fn each_value_shape_decodes_to_the_files_bytes() {
    let png = BASE64.encode(PNG);
    let svg = "<svg xmlns='http://www.w3.org/2000/svg'/>";
    let data = json!({
        "graph.png": png,
        "plain.txt": "hello world\n",
        "encoded.txt": BASE64.encode("hi there\n"),
        "inline.svg": format!("data:image/svg+xml;base64,{}", BASE64.encode(svg)),
        "object.png": {"data": png, "encoding": "base64"},
        "content.json": {"content": "{\"a\": 1}"},
        "typed.gif": {"type": "image/gif", "url": format!("data:image/gif;base64,{}", BASE64.encode("GIF89a"))},
        "main.py": BASE64.encode("print(1)"),
        "count.txt": 12,
        "remote.png": "https://example.test/x.png",
        "empty.txt": {"name": "empty.txt"},
    });
    let files = dump::decode(&data);
    let got: Vec<(&str, &str, &[u8])> = files
        .iter()
        .map(|f| (f.name.as_str(), f.mime, f.bytes.as_slice()))
        .collect();
    assert_eq!(
        got,
        [
            ("graph.png", "image/png", PNG),
            ("plain.txt", "text/plain", b"hello world\n".as_slice()),
            ("encoded.txt", "text/plain", b"hi there\n"),
            ("inline.svg", "image/svg+xml", svg.as_bytes()),
            ("object.png", "image/png", PNG),
            ("content.json", "application/json", b"{\"a\": 1}"),
            ("typed.gif", "image/gif", b"GIF89a"),
        ]
    );
}

#[test]
fn a_list_of_named_entries_is_read_too() {
    let data = json!([
        {"filename": "a.png", "data": BASE64.encode(PNG)},
        {"name": "b.txt", "content": "b"},
        {"data": "no name"},
    ]);
    let names: Vec<String> = dump::decode(&data).into_iter().map(|f| f.name).collect();
    assert_eq!(names, ["a.png", "b.txt"]);
    assert!(dump::decode(&json!(null)).is_empty());
    assert!(dump::decode(&json!("x")).is_empty());
}

#[test]
fn the_type_comes_from_the_bytes_before_the_name() {
    assert_eq!(dump::mime_of("wrong.txt", PNG), "image/png");
    assert_eq!(
        dump::mime_of("x.jpg", &[0xFF, 0xD8, 0xFF, 0xE0]),
        "image/jpeg"
    );
    assert_eq!(dump::mime_of("x.pdf", b"%PDF-1.7"), "application/pdf");
    assert_eq!(
        dump::mime_of("x.bin", &[0, 159, 146, 150]),
        "application/octet-stream"
    );
    assert_eq!(dump::mime_of("notes", b"text"), "text/plain");
}

#[test]
fn small_pictures_and_text_go_inline_and_the_rest_to_a_folder() {
    let small = Dumped {
        name: "graph.png".into(),
        mime: "image/png",
        bytes: PNG.to_vec(),
    };
    let spill = Scratch::new("ed-run-files-spill");
    assert_eq!(
        dump::for_pane(small, &spill),
        Some(RunFile {
            name: "graph.png".into(),
            mime: "image/png".into(),
            data_url: Some(format!("data:image/png;base64,{}", BASE64.encode(PNG))),
            path: None,
        })
    );
    let pdf = Dumped {
        name: "dfa.pdf".into(),
        mime: "application/pdf",
        bytes: b"%PDF-1.7".to_vec(),
    };
    let file = dump::for_pane(pdf, &spill.join("run-1")).unwrap();
    assert_eq!(file.data_url, None);
    assert!(file.path.unwrap().ends_with("run-1/dfa.pdf"));
    let large = Dumped {
        name: "out/big.txt".into(),
        mime: "text/plain",
        bytes: vec![b'a'; INLINE_LIMIT + 1],
    };
    let file = dump::for_pane(large, &spill.join("run-1")).unwrap();
    assert_eq!(file.data_url, None);
    let path = file.path.unwrap();
    assert!(path.ends_with("run-1/big.txt"), "{path}");
    assert_eq!(
        std::fs::metadata(&path).unwrap().len() as usize,
        INLINE_LIMIT + 1
    );
}

#[test]
fn a_run_ends_only_once_dump_files_follows_its_exit() {
    let png = BASE64.encode(PNG);
    let events = run_against(
        move |socket| {
            send(
                socket,
                json!({"type": "run_exit", "data": {"exit_status": {"type": "normal", "value": 0}}}),
            );
            std::thread::sleep(Duration::from_millis(100));
            send(
                socket,
                json!({"type": "run_frame", "data": {"pid": 1, "type": "stdout", "data": "bGF0ZQ=="}}),
            );
            send(
                socket,
                json!({"type": "dump_files", "data": {"main.py": "eA==", "dfa.png": png}}),
            );
            let _ = socket.read();
        },
        Vec::new(),
    );
    assert_eq!(
        events,
        [
            RunEvent::Output("bGF0ZQ==".into()),
            RunEvent::Files(vec![Dumped {
                name: "dfa.png".into(),
                mime: "image/png",
                bytes: PNG.to_vec(),
            }]),
            RunEvent::Exit(Exit {
                code: Some(0),
                ..Exit::default()
            }),
        ]
    );
}

#[test]
fn a_socket_closed_after_the_exit_keeps_the_exit() {
    let events = run_against(
        |socket| {
            send(
                socket,
                json!({"type": "run_exit", "data": {"exit_status": {"type": "normal", "value": 2}}}),
            );
            socket.close(None).unwrap();
            let _ = socket.read();
        },
        Vec::new(),
    );
    assert_eq!(
        events,
        [RunEvent::Exit(Exit {
            code: Some(2),
            ..Exit::default()
        })]
    );
}

#[test]
fn run_files_serialise_with_the_contracts_names() {
    let event = serde_json::to_value(WorkspaceEvent {
        challenge_id: 7,
        body: Body::RunFiles {
            run_id: "run-1".into(),
            files: vec![RunFile {
                name: "dfa.png".into(),
                mime: "image/png".into(),
                data_url: Some("data:image/png;base64,AA==".into()),
                path: None,
            }],
        },
    })
    .unwrap();
    assert_eq!(
        event,
        json!({"challengeId": 7, "kind": "runFiles", "runId": "run-1", "files": [
            {"name": "dfa.png", "mime": "image/png", "dataUrl": "data:image/png;base64,AA==", "path": null}
        ]})
    );
}
