//! Recorded question callbacks through the production command and reply path.
use crate::harness::core::{Command, Kind};
use crate::harness::requests::{QuestionAnswer, RequestView};

use super::{replay::Replay, rig::Rig, scenario::Scenario};

pub(super) const NAMES: [&str; 17] = [
    "codex-question-single",
    "codex-question-two",
    "codex-question-nonblocking",
    "codex-question-withdraw",
    "codex-question-typed-blocking",
    "codex-question-dismiss",
    "claude-question-single",
    "claude-question-multi",
    "claude-question-typed",
    "claude-question-two",
    "claude-question-withdraw",
    "claude-question-dismiss",
    "antigravity-question-single",
    "antigravity-question-typed",
    "antigravity-question-multi-two",
    "antigravity-question-dismiss",
    "antigravity-question-stop",
];

pub(super) fn requests(rig: &Rig) -> Vec<RequestView> {
    rig.runtime.block_on(async {
        let mut conn = rig.pool.acquire().await.unwrap();
        crate::harness::requests::store::list(&mut conn, rig.thread)
            .await
            .unwrap()
    })
}

pub(super) fn answer(
    rig: &mut Rig,
    command: &str,
    request: i64,
    option: &str,
    answers: Vec<QuestionAnswer>,
) -> Result<crate::harness::core::Outcome, String> {
    let thread = rig.thread;
    let outcome = rig.command(Command {
        id: command.into(),
        thread_id: Some(thread),
        kind: Kind::Respond {
            request_id: request,
            option_id: option.into(),
            feedback: None,
            answers: Some(answers),
        },
    });
    rig.drain();
    outcome
}

pub(super) fn recorded(name: &str) -> Replay {
    let scenario = Scenario::recorded(name);
    let frames = scenario.frames.clone();
    let answers = scenario.fixture.settings["question_answers"]
        .as_array()
        .unwrap()
        .clone();
    let stop = scenario.fixture.settings["stop"] == true;
    let mut rig = Rig::start(scenario);
    rig.open();
    let mut given = 0;
    let mut stopped = false;
    for frame in &frames {
        rig.feed(frame);
        rig.drain();
        if frame["method"] == "elicitation/create"
            || frame["method"] == "item/tool/requestUserInput"
            || (frame["type"] == "control_request"
                && frame["request"]["tool_name"] == "AskUserQuestion")
        {
            let request = requests(&rig).pop().expect("question saved");
            assert_eq!(request.kind, "question");
            let payload =
                crate::harness::requests::QuestionPayload::read(&request.payload).unwrap();
            assert!(!payload.questions.is_empty());
            if stop {
                if !stopped {
                    stopped = true;
                    let thread = rig.thread;
                    rig.command(Command {
                        id: "stop".into(),
                        thread_id: Some(thread),
                        kind: Kind::Stop { run_id: None },
                    })
                    .unwrap();
                    rig.drain();
                }
            } else {
                let a = &answers[given];
                let native = if matches!(
                    rig.provider,
                    crate::harness::Provider::Codex | crate::harness::Provider::Antigravity
                ) {
                    frame["id"].to_string()
                } else {
                    frame["request_id"].to_string()
                };
                assert_eq!(a["native"], native);
                let values: Vec<QuestionAnswer> =
                    serde_json::from_value(a["answers"].clone()).unwrap();
                answer(
                    &mut rig,
                    &format!("answer-{given}"),
                    request.id,
                    a["option"].as_str().unwrap(),
                    values.clone(),
                )
                .unwrap();
                let saved = requests(&rig).pop().unwrap();
                assert_eq!(saved.status, crate::harness::requests::Status::Resolved);
                assert_eq!(saved.answers, Some(values));
                // A second pane cannot overwrite a submitted answer.
                assert!(
                    answer(&mut rig, "second-pane", request.id, "dismiss", Vec::new()).is_err()
                );
                given += 1;
            }
        }
    }
    assert_eq!(given, answers.len());
    rig.close();
    rig.drain();
    for request in requests(&rig) {
        if stop {
            assert_eq!(request.status, crate::harness::requests::Status::Cancelled);
        } else {
            assert_ne!(request.status, crate::harness::requests::Status::Pending);
        }
    }
    rig.finish()
}

#[test]
fn question_recordings_continue_with_exact_native_answers_and_stop_or_dismiss() {
    for name in NAMES {
        recorded(name);
    }
}

#[test]
fn codex_questions_are_single_choice_with_other_and_keep_both_blocking_shapes() {
    for (name, blocking) in [
        ("codex-question-single", false),
        ("codex-question-typed-blocking", true),
    ] {
        let scenario =
            prefix(name).gate("question", |f| f["method"] == "item/tool/requestUserInput");
        let mut rig = Rig::start(scenario);
        rig.open();
        rig.until("question");
        let frame = rig
            .scenario
            .frames
            .iter()
            .find(|f| f["method"] == "item/tool/requestUserInput")
            .unwrap()
            .clone();
        rig.feed(&frame);
        rig.drain();
        let r = requests(&rig).pop().unwrap();
        let p = crate::harness::requests::QuestionPayload::read(&r.payload).unwrap();
        assert_eq!(p.is_blocking, Some(blocking));
        assert!(!p.questions[0].multi);
        assert!(p.questions[0].allows_typed_answer);
        // Do not feed the expected answer/result: this test examines a prefix.
        rig.finish();
    }
}

/// A transport EOF while a question waits must preserve its answer route,
/// even when the bridge reports the failed terminal before Exited.
#[test]
fn a_process_exit_leaves_a_question_answerable_as_a_message_on_all_question_bridges() {
    for name in [
        "codex-question-single",
        "claude-question-single",
        "antigravity-question-single",
    ] {
        let scenario = prefix(name);
        let mut rig = Rig::start(scenario);
        rig.open();
        rig.resume();
        rig.close();
        rig.drain();
        let r = requests(&rig).pop().unwrap();
        assert_eq!(r.status, crate::harness::requests::Status::Pending);
        assert_eq!(r.responder, "message");
        assert_eq!(r.delivery, crate::harness::requests::Delivery::None);
        // Dismissing the detached question sends no new message.
        answer(&mut rig, "dismiss", r.id, "dismiss", vec![]).unwrap();
        assert_eq!(
            requests(&rig)[0].delivery,
            crate::harness::requests::Delivery::Acknowledged
        );
        rig.finish();
    }
}

pub(super) fn prefix(name: &str) -> Scenario {
    let mut scenario = Scenario::recorded(name);
    let at = scenario
        .frames
        .iter()
        .position(|f| {
            f["method"] == "elicitation/create"
                || f["method"] == "item/tool/requestUserInput"
                || (f["type"] == "control_request"
                    && f["request"]["tool_name"] == "AskUserQuestion")
        })
        .unwrap();
    scenario.frames.truncate(at + 1);
    scenario.fixture.expected_outbound.truncate(1);
    scenario
}

#[test]
fn a_lost_session_answer_is_one_new_run_after_exit_or_restart_even_after_retries() {
    for restart in [false, true] {
        for name in [
            "codex-question-single",
            "claude-question-single",
            "antigravity-question-single",
        ] {
            let mut rig = Rig::start(prefix(name));
            rig.open();
            rig.resume();
            rig.drain();
            let r = requests(&rig).pop().unwrap();
            let mut rig = if restart {
                let rig = rig.restart();
                rig.startup().unwrap();
                rig
            } else {
                rig.close();
                rig.drain();
                rig
            };
            assert_eq!(requests(&rig)[0].responder, "message");
            let mut next = rig.recording(match rig.provider {
                crate::harness::Provider::Codex => "codex-ls",
                crate::harness::Provider::Antigravity => "antigravity-acp-ls",
                _ => "claude-ls",
            });
            if rig.provider == crate::harness::Provider::Codex {
                next.expected[0].frame["params"]["approvalPolicy"] =
                    serde_json::json!("on-request");
                next.expected[0].frame["params"]["approvalsReviewer"] = serde_json::json!("user");
            }
            let mut bridge = rig.session_of(&next);
            let q = crate::harness::requests::QuestionPayload::read(&r.payload)
                .unwrap()
                .questions
                .remove(0);
            let answers = vec![QuestionAnswer {
                question_id: q.id,
                answers: vec!["Red".into()],
                dismissed: false,
            }];
            let first = answer(
                &mut rig,
                "answer-after-restart",
                r.id,
                "answer",
                answers.clone(),
            )
            .unwrap();
            assert!(first.run.is_some());
            let again = answer(&mut rig, "answer-after-restart", r.id, "answer", answers).unwrap();
            assert_eq!(first, again);
            assert!(answer(&mut rig, "other-pane", r.id, "dismiss", vec![]).is_err());
            assert_eq!(
                rig.prompts().len(),
                1,
                "exactly one fallback answer message"
            );
            assert!(rig.prompts()[0].contains("Red"));
            next.play(&mut bridge);
            (bridge.eof)();
            rig.drain();
            rig.finish();
        }
    }
}

#[test]
fn invalid_answers_are_refused_before_any_write() {
    let mut rig = Rig::start(prefix("claude-question-two"));
    rig.open();
    rig.resume();
    rig.drain();
    let r = requests(&rig).pop().unwrap();
    let questions = crate::harness::requests::QuestionPayload::read(&r.payload)
        .unwrap()
        .questions;
    let valid: Vec<_> = questions
        .iter()
        .map(|q| QuestionAnswer {
            question_id: q.id.clone(),
            answers: vec![q.options[0].label.clone()],
            dismissed: false,
        })
        .collect();
    for answers in [
        vec![],
        vec![valid[0].clone()],
        vec![valid[0].clone(), valid[0].clone()],
        vec![
            QuestionAnswer {
                question_id: "foreign".into(),
                ..valid[0].clone()
            },
            valid[1].clone(),
        ],
        vec![
            QuestionAnswer {
                answers: vec![" ".into()],
                ..valid[0].clone()
            },
            valid[1].clone(),
        ],
        vec![
            QuestionAnswer {
                answers: vec!["Red".into(), "Blue".into()],
                ..valid[0].clone()
            },
            valid[1].clone(),
        ],
    ] {
        assert!(answer(&mut rig, "invalid", r.id, "answer", answers).is_err());
        assert_eq!(
            requests(&rig)[0].status,
            crate::harness::requests::Status::Pending
        );
    }
    rig.close();
    rig.drain();
    rig.finish();
}

#[test]
fn antigravity_questions_are_answerable_in_quiet_and_auto_modes() {
    for mode in [
        crate::harness::requests::PermissionMode::Quiet,
        crate::harness::requests::PermissionMode::Auto,
    ] {
        let mut scenario = Scenario::recorded("antigravity-question-single");
        scenario.fixture.permission_mode = Some(mode);
        let frames = scenario.frames.clone();
        let mut rig = Rig::start(scenario);
        rig.open();
        for frame in &frames {
            rig.feed(frame);
            rig.drain();
            if frame["method"] == "elicitation/create" {
                let r = requests(&rig).pop().unwrap();
                answer(
                    &mut rig,
                    "answer",
                    r.id,
                    "answer",
                    vec![QuestionAnswer {
                        question_id: "q1".into(),
                        answers: vec!["Blue".into()],
                        dismissed: false,
                    }],
                )
                .unwrap();
            }
        }
        rig.close();
        rig.drain();
        rig.finish();
    }
}
