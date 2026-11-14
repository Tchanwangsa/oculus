use crate::harness::SendOptions;

use super::super::command::Command;
use super::super::commit::Change;
use super::super::state::State;
use super::{Answer, Ask, Decision, Plan, RewindAsk};

/// Rewind, and edit-and-resend: the rows from a question on go, and with
/// `resend` new text is asked in its place. The provider is asked to forget
/// them first, where it holds a handle for the question; with no handle or
/// no session the rows go anyway and the chat says its agent kept them.
pub(super) fn decide(
    state: &State,
    command: &Command,
    item_id: i64,
    resend: Option<(&String, &SendOptions)>,
    answer: Option<Answer>,
) -> Result<Decision, String> {
    let (Some(row), Some(question)) = (&state.thread, &state.question) else {
        return Err("a rewind names a question in a thread".into());
    };
    let context = match answer {
        Some(Answer::Rewound { context }) => context,
        None => {
            if state.busy() {
                return Err(match resend {
                    Some(_) => "stop the current turn before editing a question",
                    None => "stop the current turn before rewinding",
                }
                .into());
            }
            if let (Some(anchor), Some(resume)) =
                (question.anchor.clone(), row.provider_session_id.clone())
            {
                let options = match resend {
                    Some((_, options)) => options.clone(),
                    None => SendOptions::default(),
                };
                return Ok(Decision::Ask(Ask::Rewind(Box::new(RewindAsk {
                    thread_id: row.id,
                    provider: row.provider,
                    anchor,
                    last_seen: state.newest_anchor.clone(),
                    resume,
                    options: SendOptions {
                        model: options.model.clone().or_else(|| row.model.clone()),
                        ..options
                    },
                }))));
            }
            false
        }
    };
    let mut plan = Plan {
        changes: vec![Change::Truncate { item_id, context }],
        text: None,
    };
    match resend {
        Some((text, options)) => plan.changes.push(Change::Begin {
            provider: row.provider,
            text: text.clone(),
            options: options.clone(),
            receipt: command.id.clone(),
            queued: None,
        }),
        None => plan.text = Some(question.text.clone()),
    }
    Ok(Decision::Commit(plan))
}

#[cfg(test)]
mod tests {
    use super::super::super::state::Live;
    use super::*;
    use crate::harness::core::Kind;
    use crate::harness::store::{Question, ThreadRow};
    use crate::harness::Provider;

    fn state(busy: bool, anchor: Option<&str>, session: Option<&str>) -> State {
        State {
            thread: Some(ThreadRow {
                id: 4,
                provider: Provider::Codex,
                provider_session_id: session.map(String::from),
                model: Some("gpt".into()),
                subject_code: None,
                lecture: None,
            }),
            live: Live {
                held: busy,
                own_turn: false,
            },
            question: Some(Question {
                text: "the question".into(),
                anchor: anchor.map(String::from),
            }),
            newest_anchor: Some("newest".into()),
            waiting: 0,
            held: 0,
            open: None,
        }
    }

    fn command() -> Command {
        Command {
            id: "c9".into(),
            thread_id: Some(4),
            kind: Kind::Rewind { item_id: 12 },
        }
    }

    #[test]
    fn a_busy_chat_refuses_and_says_which_action_it_refused() {
        let state = state(true, Some("a"), Some("s"));
        let rewind = decide(&state, &command(), 12, None, None);
        assert_eq!(
            rewind.err().unwrap(),
            "stop the current turn before rewinding"
        );
        let (text, options) = ("again".to_string(), SendOptions::default());
        let edit = decide(&state, &command(), 12, Some((&text, &options)), None);
        assert_eq!(
            edit.err().unwrap(),
            "stop the current turn before editing a question"
        );
    }

    #[test]
    fn the_provider_is_asked_first_when_it_holds_a_handle_for_the_question() {
        let asked = decide(
            &state(false, Some("a"), Some("s")),
            &command(),
            12,
            None,
            None,
        );
        let Ok(Decision::Ask(Ask::Rewind(ask))) = asked else {
            panic!("the provider is asked to forget the question");
        };
        assert_eq!((ask.anchor.as_str(), ask.resume.as_str()), ("a", "s"));
        assert_eq!(ask.last_seen.as_deref(), Some("newest"));
        assert_eq!(ask.options.model.as_deref(), Some("gpt"), "the row's model");
    }

    #[test]
    fn with_no_handle_or_no_session_the_rows_go_and_the_context_is_said_to_stay() {
        for state in [state(false, None, Some("s")), state(false, Some("a"), None)] {
            let Ok(Decision::Commit(plan)) = decide(&state, &command(), 12, None, None) else {
                panic!("nothing to ask");
            };
            assert!(matches!(
                plan.changes.as_slice(),
                [Change::Truncate {
                    item_id: 12,
                    context: false
                }]
            ));
            assert_eq!(plan.text.as_deref(), Some("the question"));
        }
    }

    #[test]
    fn the_answer_decides_the_context_and_an_edit_sends_under_the_commands_id() {
        // The chat is held for the rewind by then, which is not a refusal.
        let state = state(true, Some("a"), Some("s"));
        let (text, options) = ("again".to_string(), SendOptions::default());
        let answer = Some(Answer::Rewound { context: true });
        let decided = decide(&state, &command(), 12, Some((&text, &options)), answer);
        let Ok(Decision::Commit(plan)) = decided else {
            panic!("the answer is in");
        };
        assert!(matches!(
            plan.changes.as_slice(),
            [
                Change::Truncate {
                    item_id: 12,
                    context: true
                },
                Change::Begin { receipt, text, .. }
            ] if receipt == "c9" && text == "again"
        ));
        assert_eq!(plan.text, None, "an edit hands nothing back");
    }
}
