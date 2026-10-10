//! The real thing: a live opencode, driven through the actual endpoints.

use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use crate::harness::event::{HarnessEvent, Provider};
use crate::harness::Sink;

use super::super::{
    write_config, OneOffPrompts, OpencodeServer, OpencodeSessionOpts, OpencodeSpawn, AGENT,
};
use super::{count, errors, finishes, messages};

struct Kill(Arc<OpencodeServer>);

impl Drop for Kill {
    fn drop(&mut self) {
        self.0.kill();
    }
}

fn one_turn(rx: &mpsc::Receiver<HarnessEvent>, budget: Duration) -> Vec<HarnessEvent> {
    let deadline = Instant::now() + budget;
    let mut out = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(ev) => {
                let done = matches!(ev, HarnessEvent::TurnFinished { .. });
                out.push(ev);
                if done {
                    return out;
                }
            }
            Err(_) => panic!("no TurnFinished within {}s; got {out:#?}", budget.as_secs()),
        }
    }
}

/// The endpoints themselves: a tool turn, a rewind, a prompt on another
/// model, and a stop. Needs opencode, OpenRouter signed in, and about a
/// cent of tokens:
///
/// ```text
/// cargo test --lib harness::providers::opencode::tests::live::a_real_opencode_runs -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs opencode on PATH and OpenRouter signed in — see the doc comment"]
fn a_real_opencode_runs_a_v1_thread_end_to_end() {
    const TOOL_MODEL: &str = "openrouter/openai/gpt-4.1-mini";
    const TEXT_MODEL: &str = "openrouter/openai/gpt-4.1-nano";
    let bin = match crate::harness::cli::discover::binary(Provider::Opencode) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("skipped: no opencode binary — {e}");
            return;
        }
    };
    let dir = crate::test_support::Scratch::new("opencode-thread");
    let directory = dir.join("agents");
    write_config(
        &directory,
        &dir,
        "You are a test agent. Do exactly what the message asks, briefly.",
        &OneOffPrompts {
            naming: "You name conversations.",
            writer: "You complete notes.",
            lecture_end: "You find where lectures end.",
        },
    )
    .expect("the agent config");
    let server = OpencodeServer::spawn(OpencodeSpawn {
        bin,
        directory,
        env: crate::harness::cli::discover::child_env(),
        raw_log: None,
        default_sink: None,
    })
    .expect("opencode serve");
    let _kill = Kill(server.clone());

    let (tx, rx) = mpsc::channel::<HarnessEvent>();
    let sink: Sink = Arc::new(move |ev| {
        let _ = tx.send(ev);
    });
    let opts = |model: &str| OpencodeSessionOpts {
        model: Some(model.to_string()),
        variant: None,
        brief: String::new(),
        agent: AGENT,
    };

    // 1. A tool turn.
    let session = server
        .start_session(&opts(TOOL_MODEL), sink.clone())
        .expect("a session");
    assert!(session.starts_with("ses_"), "{session}");
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(1)),
        Ok(HarnessEvent::SessionStarted { .. })
    ));
    let started = Instant::now();
    server
        .prompt(
            &session,
            "Use the bash tool to run `ls`, then say one file name you saw.",
        )
        .expect("prompt_async");
    let events = one_turn(&rx, Duration::from_secs(120));
    eprintln!(
        "tool turn: {} events in {:.1}s",
        events.len(),
        started.elapsed().as_secs_f64()
    );
    assert_eq!(
        count(&events, |e| matches!(e, HarnessEvent::TurnStarted)),
        1
    );
    let anchors: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::TurnAnchor { anchor } => Some(anchor.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(anchors.len(), 1, "{anchors:?}");
    assert!(anchors[0].starts_with("msg_"), "{anchors:?}");
    let anchor = anchors[0].to_string();
    assert_eq!(
        count(
            &events,
            |e| matches!(e, HarnessEvent::ToolStarted { name, .. } if name == "bash")
        ),
        1,
        "{events:#?}"
    );
    assert_eq!(
        count(&events, |e| matches!(
            e,
            HarnessEvent::ToolFinished { ok: true, .. }
        )),
        1
    );
    assert!(!messages(&events).is_empty(), "{events:#?}");
    assert_eq!(finishes(&events), vec!["completed"]);
    assert!(errors(&events).is_empty(), "{:?}", errors(&events));
    assert!(!server.busy(), "the turn is closed");
    eprintln!("  anchor {anchor}, answer {:?}", messages(&events));

    // 2. Rewind to that question (inclusive).
    server.revert(&session, &anchor).expect("revert");
    eprintln!("  revert {anchor}: ok");

    // 3. Another model: the one asked for on attach wins.
    server
        .attach_session(&session, &opts(TEXT_MODEL), sink.clone())
        .expect("attach");
    assert!(
        matches!(rx.recv_timeout(Duration::from_secs(1)), Ok(HarnessEvent::SessionStarted { model: Some(m), .. }) if m == TEXT_MODEL)
    );
    let started = Instant::now();
    server
        .prompt(&session, "Reply with the single word: ok")
        .expect("prompt_async");
    let events = one_turn(&rx, Duration::from_secs(60));
    eprintln!(
        "text turn after revert: {} events in {:.1}s, answer {:?}",
        events.len(),
        started.elapsed().as_secs_f64(),
        messages(&events)
    );
    assert_eq!(finishes(&events), vec!["completed"]);
    assert!(errors(&events).is_empty(), "{:?}", errors(&events));

    // 4. Stop a long answer mid-stream.
    let started = Instant::now();
    server
        .prompt(&session, "Count from 1 to 300, one per line")
        .expect("prompt_async");
    let mut before = Vec::new();
    loop {
        let ev = rx
            .recv_timeout(Duration::from_secs(60))
            .expect("the turn to start streaming");
        let go = matches!(
            ev,
            HarnessEvent::AssistantDelta { .. } | HarnessEvent::ToolStarted { .. }
        );
        let over = matches!(ev, HarnessEvent::TurnFinished { .. });
        before.push(ev);
        assert!(
            !over,
            "the turn finished before it could be stopped: {before:#?}"
        );
        if go {
            break;
        }
    }
    server.interrupt(&session).expect("abort");
    let mut events = before;
    events.extend(one_turn(&rx, Duration::from_secs(60)));
    eprintln!(
        "interrupted turn: {} events in {:.1}s",
        events.len(),
        started.elapsed().as_secs_f64()
    );
    assert_eq!(finishes(&events), vec!["interrupted"], "{events:#?}");
    assert!(
        errors(&events).is_empty(),
        "stopping is not an error: {:?}",
        errors(&events)
    );
    assert!(!server.busy());

    server.delete_session(&session);
}
