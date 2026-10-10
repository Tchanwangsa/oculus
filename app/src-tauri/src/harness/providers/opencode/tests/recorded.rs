//! Turns replayed from the 1.18.31 recordings.

use serde_json::{json, Value};

use crate::harness::event::{HarnessEvent, ToolKind};

use super::super::events::{friendly, session_of, translate};
use super::super::session::SessionState;
use super::{count, deltas, errors, finishes, messages, replay};

const V1_LS: &str = include_str!("../../../../../fixtures/harness/opencode-v1-ls.ndjson");

/// Two steps, one bash call with growing output snapshots, then a streamed
/// answer. Opens on the user row (the anchor), closes once on `stop`.
#[test]
fn folds_a_recorded_v1_turn() {
    let events = replay(V1_LS);
    assert!(matches!(events.first(), Some(HarnessEvent::TurnStarted)));
    assert_eq!(
        count(&events, |e| matches!(e, HarnessEvent::TurnStarted)),
        1,
        "user re-emits open nothing"
    );
    let anchors: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::TurnAnchor { anchor } => Some(anchor.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        anchors,
        vec!["msg_0add5cba9001dBRhWoUADhRh8h"],
        "the user msg id, once"
    );

    let tools: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::ToolStarted {
                id,
                kind,
                name,
                title,
                input,
            } => Some((
                id.clone(),
                *kind,
                name.clone(),
                title.clone(),
                input.clone(),
            )),
            _ => None,
        })
        .collect();
    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].0, "call_UnhBtW5By24FPWOwYzi30lZ6",
        "keyed by callID"
    );
    assert_eq!(tools[0].1, ToolKind::Bash);
    assert_eq!(tools[0].2, "bash");
    assert!(
        tools[0].3.contains("ls"),
        "titled off the running snapshot's input, not pending's `{{}}`"
    );
    assert_eq!(tools[0].4["command"], "ls");

    // Two empty `running` snapshots, then the whole listing: one delta.
    let out_deltas: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::ToolOutputDelta { id, text } => {
                assert_eq!(id, "call_UnhBtW5By24FPWOwYzi30lZ6");
                Some(text.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(out_deltas, vec!["README-scratch.txt\nopencode.json\n"]);

    let finished_tools: Vec<(bool, &str)> = events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::ToolFinished { ok, output, .. } => Some((*ok, output.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(finished_tools.len(), 1);
    assert!(finished_tools[0].0);
    assert!(finished_tools[0].1.contains("opencode.json"));

    let text = messages(&events);
    assert_eq!(text.len(), 1, "one answer, from the closing snapshot");
    assert_eq!(
        deltas(&events),
        text[0],
        "the deltas add up to the snapshot"
    );
    assert!(!text[0].is_empty());
    assert!(!events.iter().any(|e| matches!(
        e,
        HarnessEvent::ThinkingDelta { .. } | HarnessEvent::Thinking { .. }
    )));

    // One `Usage` per `step-finish`, accumulating fresh + cached input.
    let usage: Vec<(u64, u64, Option<u64>)> = events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::Usage {
                input_tokens,
                output_tokens,
                context_tokens,
                ..
            } => Some((*input_tokens, *output_tokens, *context_tokens)),
            _ => None,
        })
        .collect();
    assert_eq!(
        usage,
        vec![
            (5827, 79, Some(5906)),
            (5827 + 160 + 5760, 79 + 17, Some(5937))
        ]
    );
    assert!(events
        .iter()
        .any(|e| matches!(e, HarnessEvent::Usage { cost_usd: Some(c), .. } if *c > 0.003)));

    assert_eq!(finishes(&events), vec!["completed"]);
    assert!(
        matches!(events.last(), Some(HarnessEvent::TurnFinished { .. })),
        "idle and the user re-emit after it add nothing"
    );
    assert!(errors(&events).is_empty());
}

/// The smallest v1 turn: one step, one delta, `ok`.
#[test]
fn folds_a_recorded_v1_text_turn() {
    let events = replay(include_str!(
        "../../../../../fixtures/harness/opencode-v1-text.ndjson"
    ));
    assert_eq!(
        count(&events, |e| matches!(e, HarnessEvent::TurnStarted)),
        1
    );
    assert_eq!(messages(&events), vec!["ok"]);
    assert_eq!(deltas(&events), "ok");
    assert_eq!(
        count(&events, |e| matches!(e, HarnessEvent::Usage { .. })),
        1
    );
    assert_eq!(finishes(&events), vec!["completed"]);
    assert!(errors(&events).is_empty());
    assert!(!events
        .iter()
        .any(|e| matches!(e, HarnessEvent::ToolStarted { .. })));
}

/// Reasoning deltas say `field: "text"` too; only the opening snapshot's
/// part type marks them as thinking.
#[test]
fn folds_a_recorded_v1_reasoning_turn() {
    let events = replay(include_str!(
        "../../../../../fixtures/harness/opencode-v1-reasoning.ndjson"
    ));
    let thinking_deltas: String = events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::ThinkingDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    let thinking: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::Thinking { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(thinking.len(), 1);
    assert!(thinking[0].starts_with("The user is asking"));
    assert_eq!(thinking_deltas, thinking[0]);
    assert_eq!(messages(&events), vec!["ok"]);
    assert_eq!(
        deltas(&events),
        "ok",
        "the reasoning deltas were not read as answer text"
    );
    let think_at = events
        .iter()
        .position(|e| matches!(e, HarnessEvent::Thinking { .. }))
        .unwrap();
    let answer_at = events
        .iter()
        .position(|e| matches!(e, HarnessEvent::AssistantMessage { .. }))
        .unwrap();
    assert!(think_at < answer_at);
    // 0 output + 31 reasoning.
    assert!(events.iter().any(|e| matches!(
        e,
        HarnessEvent::Usage {
            output_tokens: 31,
            ..
        }
    )));
    assert_eq!(finishes(&events), vec!["completed"]);
    assert!(errors(&events).is_empty());
    assert!(matches!(
        events.last(),
        Some(HarnessEvent::TurnFinished { .. })
    ));
}

/// The first idle of an abort must not close the turn, or the partial
/// answer that follows it is lost.
#[test]
fn a_v1_abort_is_interrupted_not_failed() {
    let events = replay(include_str!(
        "../../../../../fixtures/harness/opencode-v1-interrupt.ndjson"
    ));
    let text = messages(&events);
    assert_eq!(text.len(), 1, "the partial answer is one row");
    assert!(!text[0].is_empty());
    assert_eq!(deltas(&events), text[0]);
    assert_eq!(finishes(&events), vec!["interrupted"]);
    assert!(
        errors(&events).is_empty(),
        "stopping a turn is not an error"
    );
    let answer_at = events
        .iter()
        .position(|e| matches!(e, HarnessEvent::AssistantMessage { .. }))
        .unwrap();
    let finish_at = events
        .iter()
        .position(|e| matches!(e, HarnessEvent::TurnFinished { .. }))
        .unwrap();
    assert!(
        answer_at < finish_at,
        "the row lands before the turn closes"
    );
    assert!(
        matches!(events.last(), Some(HarnessEvent::TurnFinished { .. })),
        "the two idles add nothing"
    );
}

/// An unknown model: no assistant row; the `session.error` closes the
/// turn in the provider's words, and its stack-trace re-emit is dropped.
#[test]
fn a_v1_model_not_found_fails_once_with_the_providers_words() {
    let events = replay(include_str!(
        "../../../../../fixtures/harness/opencode-v1-error.ndjson"
    ));
    assert_eq!(
        count(&events, |e| matches!(e, HarnessEvent::TurnStarted)),
        1
    );
    let errs = errors(&events);
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert!(errs[0].contains("Model not found"));
    assert!(errs[0].contains("Did you mean"));
    assert!(!errs[0].contains("\n    at "));
    assert_eq!(finishes(&events), vec!["failed"]);
    let err_at = events
        .iter()
        .position(|e| matches!(e, HarnessEvent::Error { .. }))
        .unwrap();
    let finish_at = events
        .iter()
        .position(|e| matches!(e, HarnessEvent::TurnFinished { .. }))
        .unwrap();
    assert!(err_at < finish_at);
    assert!(
        matches!(events.last(), Some(HarnessEvent::TurnFinished { .. })),
        "idle and the trace add nothing"
    );
    assert!(
        messages(&events).is_empty(),
        "the user's own text part is not an answer"
    );
}

/// A repeated completed row and a late idle add no second finish.
#[test]
fn a_v1_turn_finishes_exactly_once() {
    let last_of = |ty: &str, role: Option<&str>| {
        V1_LS
            .lines()
            .filter(|l| {
                let v: Value = serde_json::from_str(l).unwrap();
                v["type"] == ty && role.is_none_or(|r| v["properties"]["info"]["role"] == r)
            })
            .last()
            .unwrap()
            .to_string()
    };
    let again = format!(
        "{V1_LS}\n{}\n{}\n",
        last_of("message.updated", Some("assistant")),
        last_of("session.idle", None)
    );
    let events = replay(&again);
    assert_eq!(finishes(&events), vec!["completed"]);
    assert_eq!(
        count(&events, |e| matches!(e, HarnessEvent::TurnStarted)),
        1
    );
    assert_eq!(
        count(&events, |e| matches!(e, HarnessEvent::Usage { .. })),
        2
    );
    assert!(errors(&events).is_empty());
}

/// A bad agent name: `session.error` twice and no idle, so the error closes.
#[test]
fn a_v1_bad_agent_closes_on_the_error_because_no_idle_follows() {
    let mut st = SessionState::default();
    let go = |st: &mut SessionState, ty: &str, p: Value| translate(ty, &p, st);
    let opened = go(
        &mut st,
        "message.updated",
        json!({"info": {"id": "msg_u1", "role": "user", "sessionID": "ses_1"}}),
    );
    assert!(
        matches!(opened.as_slice(), [HarnessEvent::TurnStarted, HarnessEvent::TurnAnchor { anchor }] if anchor == "msg_u1")
    );
    let msg =
        "Agent not found: \"nope-agent\". Available agents: build, explore, general, oculus, plan";
    let out = go(
        &mut st,
        "session.error",
        json!({"sessionID": "ses_1", "error": {"name": "UnknownError", "data": {"message": msg}}}),
    );
    assert_eq!(errors(&out), vec![msg]);
    assert_eq!(finishes(&out), vec!["failed"]);
    let trace = go(
        &mut st,
        "session.error",
        json!({"sessionID": "ses_1", "error": {"name": "UnknownError", "data": {"message": format!("AgentNotFoundError: {msg}\n    at <anonymous> (/$bunfs/root/x.js:1:1)")}}}),
    );
    assert!(trace.is_empty(), "{trace:?}");
}

/// A tool that skips `running`, and an abort before any assistant row,
/// both leave the timeline balanced.
#[test]
fn a_v1_tool_that_never_ran_still_opens_a_row_off_its_input() {
    let mut st = SessionState::default();
    let go = |st: &mut SessionState, ty: &str, p: Value| translate(ty, &p, st);
    go(
        &mut st,
        "message.updated",
        json!({"info": {"id": "msg_u1", "role": "user"}}),
    );
    go(
        &mut st,
        "message.updated",
        json!({"info": {"id": "msg_a1", "role": "assistant", "parentID": "msg_u1", "time": {"created": 1}}}),
    );
    let pending = go(
        &mut st,
        "message.part.updated",
        json!({"part": {"id": "prt_1", "messageID": "msg_a1", "type": "tool", "callID": "call_1", "tool": "read", "state": {"status": "pending", "input": {}}}}),
    );
    assert!(pending.is_empty());
    let done = go(
        &mut st,
        "message.part.updated",
        json!({"part": {"id": "prt_1", "messageID": "msg_a1", "type": "tool", "callID": "call_1", "tool": "read", "state": {"status": "completed", "input": {"path": "../courses/COMP30026/w1.md"}, "output": "# Week 1", "title": "w1.md"}}}),
    );
    assert!(
        matches!(&done[0], HarnessEvent::ToolStarted { id, kind: ToolKind::Read, name, title, .. } if id == "call_1" && name == "read" && title == "w1.md")
    );
    assert!(
        matches!(&done[1], HarnessEvent::ToolFinished { id, ok: true, output, .. } if id == "call_1" && output == "# Week 1")
    );
    assert_eq!(done.len(), 2);

    let mut st = SessionState::default();
    go(
        &mut st,
        "message.updated",
        json!({"info": {"id": "msg_u2", "role": "user"}}),
    );
    let out = go(
        &mut st,
        "session.error",
        json!({"error": {"name": "MessageAbortedError", "data": {"message": "Aborted"}}}),
    );
    assert_eq!(finishes(&out), vec!["interrupted"]);
    assert!(errors(&out).is_empty());
    assert!(go(&mut st, "session.idle", json!({"sessionID": "ses_1"})).is_empty());
}

#[test]
fn the_zen_free_tier_gets_a_sentence_rather_than_a_400() {
    let raw = "Provider request failed with HTTP 400: {\"type\":\"error\",\"error\":\
                   {\"type\":\"MissingSessionID\",\"message\":\"Error from provider (Console): \
                   OpenCode's free tier can only be used in OpenCode\"}}";
    let out = friendly(raw);
    assert!(out.contains("opencode auth login"));
    assert!(!out.contains("HTTP 400"));
}

/// An assistant row created and never completed, then idle: idle fails
/// the turn, once.
#[test]
fn a_v1_idle_with_the_assistant_still_open_fails_the_turn_once() {
    let cut: Vec<&str> = V1_LS.lines().take(7).collect();
    let last: Value = serde_json::from_str(cut[6]).unwrap();
    assert_eq!(last["type"], "message.updated");
    assert_eq!(last["properties"]["info"]["role"], "assistant");
    assert!(
        last["properties"]["info"]["time"]["completed"].is_null(),
        "the bare row, not a completed one"
    );
    let raw = format!(
            "{}\n{{\"type\":\"session.idle\",\"properties\":{{\"sessionID\":\"ses_f522a3477ffeV1Sb54a1xq018p\"}}}}\n",
            cut.join("\n")
        );
    let events = replay(&raw);
    assert_eq!(
        count(&events, |e| matches!(e, HarnessEvent::TurnStarted)),
        1
    );
    assert_eq!(finishes(&events), vec!["failed"]);
    let errs = errors(&events);
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert!(errs[0].contains("without answering"));
    assert!(matches!(
        events.last(),
        Some(HarnessEvent::TurnFinished { .. })
    ));
    let mut st = SessionState::default();
    for line in raw.lines() {
        let v: Value = serde_json::from_str(line).unwrap();
        if let Some(ty) = v["type"].as_str() {
            if session_of(&v).is_some() {
                translate(ty, &v["properties"], &mut st);
            }
        }
    }
    assert!(translate("session.idle", &json!({"sessionID": "ses_1"}), &mut st).is_empty());
}

#[test]
fn a_v1_user_row_opens_on_its_id_not_on_the_absence_of_summary() {
    let mut st = SessionState::default();
    let go = |st: &mut SessionState, p: Value| translate("message.updated", &p, st);
    let first = go(
        &mut st,
        json!({"info": {"id": "msg_u1", "role": "user", "summary": {"diffs": []}}}),
    );
    assert!(
        matches!(first.as_slice(), [HarnessEvent::TurnStarted, HarnessEvent::TurnAnchor { anchor }] if anchor == "msg_u1")
    );
    assert!(go(
        &mut st,
        json!({"info": {"id": "msg_u1", "role": "user", "summary": {"diffs": []}}})
    )
    .is_empty());
    assert!(go(&mut st, json!({"info": {"id": "msg_u1", "role": "user"}})).is_empty());
}
