//! The chat's selection: the provider, model and reasoning level its next
//! send uses. A pick is stored at once, mid-turn or not, and changes
//! nothing that is already written; the switch itself happens when a run
//! starts (`manager/transition.rs`, `handoff/`).

use crate::harness::manager::transition::{background_alive, no_handoff};
use crate::harness::store::ThreadRow;
use crate::harness::Provider;

use super::super::commit::Change;
use super::super::state::State;
use super::{Decision, Plan};

/// Why the chat cannot move to `provider` now; `None` when it can, or is
/// already there. A bridge that cannot take a handoff never takes over a
/// started chat, and a provider with a background task alive is not left
/// while it runs: ending its session would kill the task.
pub(super) fn refusal(state: &State, provider: Provider) -> Option<String> {
    let row = state.thread.as_ref()?;
    if row.provider == provider {
        return None;
    }
    if !state.live.takeover.allows(provider) {
        return Some(no_handoff(provider));
    }
    state
        .live
        .background
        .then(|| background_alive(row.provider))
}

/// The change that makes `row`'s selection this one; `None` when it
/// already is. A send that names no model keeps its provider's.
pub(super) fn change(
    row: &ThreadRow,
    provider: Provider,
    model: Option<&String>,
    effort: Option<&String>,
) -> Option<Change> {
    let model = match (model, row.provider == provider) {
        (Some(model), _) => Some(model.clone()),
        (None, true) => row.model.clone(),
        (None, false) => None,
    };
    let effort = effort.cloned();
    let same = row.provider == provider && row.model == model && row.effort == effort;
    (!same).then_some(Change::Select {
        provider,
        model,
        effort,
    })
}

pub(super) fn decide(
    state: &State,
    provider: Provider,
    model: Option<&String>,
    effort: Option<&String>,
) -> Result<Decision, String> {
    let row = state.thread.as_ref().ok_or("a selection names a thread")?;
    if let Some(reason) = refusal(state, provider) {
        return Err(reason);
    }
    Ok(Decision::Commit(Plan {
        changes: change(row, provider, model, effort).into_iter().collect(),
        text: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::super::super::state::{Live, OpenRun, Takeover};
    use super::*;

    fn state(provider: Provider, model: &str, effort: Option<&str>) -> State {
        State {
            thread: Some(ThreadRow {
                id: 4,
                provider,
                model: Some(model.into()),
                effort: effort.map(String::from),
                subject_code: None,
                lecture: None,
            }),
            live: Live {
                takeover: Takeover::read(|p| matches!(p, Provider::Claude | Provider::Codex)),
                ..Live::default()
            },
            question: None,
            newest_anchor: None,
            holder: None,
            waiting: 0,
            held: 0,
            open: None,
        }
    }

    fn pick(state: &State, provider: Provider, model: &str, effort: Option<&str>) -> Vec<Change> {
        let (model, effort) = (model.to_string(), effort.map(String::from));
        match decide(state, provider, Some(&model), effort.as_ref()).unwrap() {
            Decision::Commit(plan) => plan.changes,
            Decision::Ask(_) => panic!("a pick asks the provider nothing"),
        }
    }

    #[test]
    fn a_pick_is_stored_whole_and_the_same_pick_changes_nothing() {
        let claude = state(Provider::Claude, "opus", Some("high"));
        assert!(pick(&claude, Provider::Claude, "opus", Some("high")).is_empty());
        let model = pick(&claude, Provider::Claude, "sonnet", Some("high"));
        assert!(matches!(
            model.as_slice(),
            [Change::Select { provider: Provider::Claude, model: Some(m), effort: Some(e) }]
                if m == "sonnet" && e == "high"
        ));
        // A model with no levels clears the one the old model had.
        let level = pick(&claude, Provider::Claude, "opus", None);
        assert!(matches!(
            level.as_slice(),
            [Change::Select { effort: None, .. }]
        ));
        let other = pick(&claude, Provider::Codex, "gpt", Some("medium"));
        assert!(matches!(
            other.as_slice(),
            [Change::Select { provider: Provider::Codex, model: Some(m), .. }] if m == "gpt"
        ));
    }

    #[test]
    fn a_pick_is_stored_while_a_turn_runs() {
        let mut busy = state(Provider::Claude, "opus", None);
        busy.open = Some(OpenRun {
            run_id: 9,
            starting: false,
            stop_requested: false,
        });
        busy.live.held = true;
        assert_eq!(pick(&busy, Provider::Codex, "gpt", None).len(), 1);
    }

    #[test]
    fn a_bridge_that_takes_no_handoff_cannot_take_over_a_started_chat() {
        let claude = state(Provider::Claude, "opus", None);
        for provider in [Provider::Opencode, Provider::Antigravity] {
            let refused = decide(&claude, provider, None, None);
            assert_eq!(
                refused.err().unwrap(),
                format!(
                    "{} cannot take over a chat that has started. Start a new chat to switch.",
                    provider.label()
                )
            );
        }
        // Its own chat is no switch: a model pick there is stored.
        let own = state(Provider::Antigravity, "gemini", None);
        assert_eq!(pick(&own, Provider::Antigravity, "other", None).len(), 1);
    }

    #[test]
    fn a_switch_away_from_a_provider_with_a_background_task_is_refused_until_it_ends() {
        let mut claude = state(Provider::Claude, "opus", None);
        claude.live.background = true;
        let refused = decide(&claude, Provider::Codex, None, None);
        assert_eq!(
            refused.err().unwrap(),
            "Claude Code is still running a background task in this chat. Wait for it to finish, or stop it, before switching."
        );
        // Its own model can still be picked.
        assert_eq!(pick(&claude, Provider::Claude, "sonnet", None).len(), 1);
        claude.live.background = false;
        assert_eq!(pick(&claude, Provider::Codex, "gpt", None).len(), 1);
    }

    #[test]
    fn a_send_that_names_no_model_keeps_its_providers() {
        let row = state(Provider::Claude, "opus", Some("high"))
            .thread
            .unwrap();
        let high = "high".to_string();
        assert!(change(&row, Provider::Claude, None, Some(&high)).is_none());
        assert!(matches!(
            change(&row, Provider::Codex, None, None),
            Some(Change::Select {
                provider: Provider::Codex,
                model: None,
                effort: None
            })
        ));
    }
}
