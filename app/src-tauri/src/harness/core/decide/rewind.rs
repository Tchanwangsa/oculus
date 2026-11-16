use crate::harness::{Provider, SendOptions};

use super::super::command::Command;
use super::super::commit::Change;
use super::super::state::State;
use super::{select, Answer, Ask, Decision, Plan, RewindAsk};

/// An edit's new question, and what the composer showed when it was sent.
pub(super) struct Resend<'a> {
    pub text: &'a String,
    pub options: &'a SendOptions,
    /// `None` keeps the chat's provider.
    pub provider: Option<Provider>,
}

/// Rewind, and edit-and-resend: the rows from a question on go, and with
/// `resend` new text is asked in its place. The provider is asked to forget
/// them first, where it holds a handle for the question; with no handle or
/// no session the rows go anyway and the chat says its agent kept them.
pub(super) fn decide(
    state: &State,
    command: &Command,
    item_id: i64,
    resend: Option<Resend>,
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
            // An edit sent under another provider is a switch, refused
            // before anything is taken out.
            let target = resend.as_ref().and_then(|resend| resend.provider);
            if let Some(reason) = target.and_then(|to| select::refusal(state, to)) {
                return Err(reason);
            }
            // The session that answered the question is the one asked to
            // forget it, whichever provider the chat is on now.
            let held = state.holder.as_ref().and_then(|holder| {
                let session = holder.session.clone()?;
                Some((holder, session, question.anchor.clone()?))
            });
            if let Some((holder, resume, anchor)) = held {
                return Ok(Decision::Ask(Ask::Rewind(Box::new(RewindAsk {
                    thread_id: row.id,
                    provider: holder.provider,
                    anchor,
                    last_seen: state.newest_anchor.clone(),
                    resume,
                    // A respawn for the rewind runs under what the question
                    // was sent with, whatever the edit goes out under.
                    options: SendOptions {
                        model: holder.model.clone(),
                        reasoning_effort: holder.effort.clone(),
                        ..SendOptions::default()
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
        Some(resend) => {
            let provider = resend.provider.unwrap_or(row.provider);
            let options = resend.options;
            let (model, effort) = (options.model.as_ref(), options.reasoning_effort.as_ref());
            plan.changes
                .extend(select::change(row, provider, model, effort));
            plan.changes.push(Change::Begin {
                text: resend.text.clone(),
                options: options.clone(),
                receipt: command.id.clone(),
                queued: None,
            });
        }
        None => plan.text = Some(question.text.clone()),
    }
    Ok(Decision::Commit(plan))
}

#[cfg(test)]
mod tests {
    use super::super::super::state::Live;
    use super::*;
    use crate::harness::core::Kind;
    use crate::harness::store::bindings::Holder;
    use crate::harness::store::{Question, ThreadRow};
    use crate::harness::Provider;

    fn state(busy: bool, anchor: Option<&str>, session: Option<&str>) -> State {
        State {
            thread: Some(ThreadRow {
                id: 4,
                provider: Provider::Codex,
                model: Some("gpt".into()),
                effort: Some("high".into()),
                subject_code: None,
                lecture: None,
            }),
            live: Live {
                held: busy,
                ..Live::default()
            },
            question: Some(Question {
                text: "the question".into(),
                anchor: anchor.map(String::from),
            }),
            newest_anchor: Some("newest".into()),
            holder: Some(Holder {
                provider: Provider::Claude,
                session: session.map(String::from),
                model: Some("opus".into()),
                effort: Some("low".into()),
            }),
            waiting: 0,
            held: 0,
            open: None,
            request: None,
            pending: 0,
            questions: 0,
            approvals: Vec::new(),
            queued: None,
        }
    }

    fn resend<'a>(text: &'a String, options: &'a SendOptions) -> Resend<'a> {
        Resend {
            text,
            options,
            provider: None,
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
        let edit = decide(&state, &command(), 12, Some(resend(&text, &options)), None);
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
        // The chat is on Codex now; the question was Claude's, and it is
        // Claude's session that is asked, under what that run was sent with.
        assert_eq!(ask.provider, Provider::Claude);
        assert_eq!(ask.options.model.as_deref(), Some("opus"));
        assert_eq!(ask.options.reasoning_effort.as_deref(), Some("low"));
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
        let text = "again".to_string();
        let options = SendOptions {
            reasoning_effort: Some("high".into()),
            ..SendOptions::default()
        };
        let answer = Some(Answer::Rewound { context: true });
        let decided = decide(
            &state,
            &command(),
            12,
            Some(resend(&text, &options)),
            answer,
        );
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

    #[test]
    fn an_edit_sent_under_another_model_or_provider_changes_the_chats_selection() {
        let mut state = state(true, Some("a"), Some("s"));
        let text = "again".to_string();
        let options = SendOptions {
            model: Some("other".into()),
            reasoning_effort: Some("high".into()),
            ..SendOptions::default()
        };
        let answer = Some(Answer::Rewound { context: true });
        let edit = |state: &State, provider, answer| {
            let resend = Resend {
                text: &text,
                options: &options,
                provider,
            };
            decide(state, &command(), 12, Some(resend), answer)
        };
        let Ok(Decision::Commit(plan)) = edit(&state, None, answer) else {
            panic!("the answer is in");
        };
        assert!(matches!(
            plan.changes.as_slice(),
            [
                Change::Truncate { .. },
                Change::Select { provider: Provider::Codex, model: Some(model), .. },
                Change::Begin { .. }
            ] if model == "other"
        ));
        // Another provider is a switch: refused, before anything is asked
        // or removed, where its bridge takes no handoff.
        state.live.held = false;
        let refused = edit(&state, Some(Provider::Claude), None);
        assert!(refused
            .err()
            .unwrap()
            .contains("Start a new chat to switch"));
    }
}
