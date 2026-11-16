//! What a send does to the chat's session: keep it, change its model in
//! place, restart it, or bring up another provider's. One pure decision,
//! made by `Harness::ensure` from the live session and the run's selection.

use crate::harness::providers::capabilities::Capabilities;
use crate::harness::requests::PermissionMode;
use crate::harness::Provider;

/// A provider, a model and a reasoning level: what a run is sent with, and
/// what a live session was started under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::harness) struct Selection {
    pub provider: Provider,
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// The chat's live session, as the decision reads it.
pub(in crate::harness) struct Current<'a> {
    pub selection: &'a Selection,
    /// The permission mode it was started or last told under.
    pub mode: PermissionMode,
    /// It has work running in the background that would die with it.
    pub background: bool,
    /// Its CLI is waiting on a request the student has not answered.
    pub pending_request: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::harness) enum Transition {
    /// The live session takes the message as it is.
    Reuse,
    /// The same session, told the new model with the turn (and the new
    /// permission mode, if that changed too).
    SwitchModelInSession,
    /// The same session, told the new permission mode with the turn; or a
    /// bridge that reads no mode, which only records it.
    SwitchModeInSession,
    /// The session is ended and its native conversation resumed under the
    /// new model or reasoning level.
    RestartAndResume,
    /// No session of the target's is up: any other provider's is ended and
    /// one is started or resumed for the target. Whatever it has not seen
    /// of the chat rides its first prompt as a handoff (`handoff/`).
    CreateWithHandoff,
    /// The switch cannot be made now, and why.
    Reject(String),
}

/// A bridge that cannot take over a chat that another provider started.
pub(in crate::harness) fn no_handoff(target: Provider) -> String {
    format!(
        "{} cannot take over a chat that has started. Start a new chat to switch.",
        target.label()
    )
}

/// The old provider still has a background task running in the chat.
pub(in crate::harness) fn background_alive(current: Provider) -> String {
    format!(
        "{} is still running a background task in this chat. Wait for it to finish, or stop it, before switching.",
        current.label()
    )
}

/// The old provider's CLI is waiting on the student: ending its session
/// would leave the request unanswerable.
pub(in crate::harness) fn request_pending(current: Provider) -> String {
    format!(
        "{} is waiting for you to answer a request in this chat. Answer it before switching.",
        current.label()
    )
}

/// In order: another provider's session is live (refused while the target
/// cannot take a handoff, the old one has background work or a request
/// waiting, else a new session with a handoff); a different reasoning level
/// restarts, since every CLI binds it at session start; a different model,
/// or permission mode, is told with the turn where the bridge can, else
/// restarts; otherwise the session is kept. A bridge with no approvals
/// reads no mode, so a mode change there only records it.
pub(in crate::harness) fn decide_transition(
    current: Option<Current>,
    target: (&Selection, PermissionMode),
    caps: &Capabilities,
) -> Transition {
    let (target, mode) = target;
    let Some(current) = current else {
        return Transition::CreateWithHandoff;
    };
    let live = current.selection;
    if live.provider != target.provider {
        if !caps.provider_handoff {
            return Transition::Reject(no_handoff(target.provider));
        }
        if current.background {
            return Transition::Reject(background_alive(live.provider));
        }
        if current.pending_request {
            return Transition::Reject(request_pending(live.provider));
        }
        return Transition::CreateWithHandoff;
    }
    if live.effort != target.effort {
        return Transition::RestartAndResume;
    }
    let model = live.model != target.model;
    let moded = current.mode != mode;
    if (model && !caps.model_switch_in_session)
        || (moded && caps.approvals && !caps.mode_switch_in_session)
    {
        return Transition::RestartAndResume;
    }
    match (model, moded) {
        (true, _) => Transition::SwitchModelInSession,
        (false, true) => Transition::SwitchModeInSession,
        (false, false) => Transition::Reuse,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::providers::capabilities;

    fn selection(provider: Provider, model: &str, effort: Option<&str>) -> Selection {
        Selection {
            provider,
            model: Some(model.into()),
            effort: effort.map(String::from),
        }
    }

    fn decide(live: Option<(&Selection, bool)>, target: &Selection, handoff: bool) -> Transition {
        let caps = Capabilities {
            provider_handoff: handoff,
            ..capabilities::of(target.provider)
        };
        let current = live.map(|(selection, background)| Current {
            selection,
            mode: PermissionMode::Ask,
            background,
            pending_request: false,
        });
        decide_transition(current, (target, PermissionMode::Ask), &caps)
    }

    #[test]
    fn no_live_session_means_one_is_brought_up() {
        let target = selection(Provider::Codex, "gpt", None);
        for handoff in [true, false] {
            assert_eq!(
                decide(None, &target, handoff),
                Transition::CreateWithHandoff
            );
        }
    }

    #[test]
    fn another_providers_session_gives_way_to_a_bridge_that_takes_handoffs() {
        let live = selection(Provider::Claude, "opus", Some("high"));
        let target = selection(Provider::Codex, "gpt", Some("high"));
        assert_eq!(
            decide(Some((&live, false)), &target, true),
            Transition::CreateWithHandoff
        );
    }

    #[test]
    fn a_bridge_that_cannot_take_a_handoff_refuses_and_says_to_start_a_new_chat() {
        let live = selection(Provider::Claude, "opus", None);
        let target = selection(Provider::Antigravity, "gemini", None);
        assert_eq!(
            decide(Some((&live, false)), &target, false),
            Transition::Reject(
                "Antigravity cannot take over a chat that has started. Start a new chat to switch."
                    .into()
            )
        );
    }

    #[test]
    fn a_switch_away_from_a_session_with_background_work_is_refused() {
        let live = selection(Provider::Claude, "opus", None);
        let target = selection(Provider::Codex, "gpt", None);
        let Transition::Reject(reason) = decide(Some((&live, true)), &target, true) else {
            panic!("the background task would die with the session");
        };
        assert!(reason.starts_with("Claude Code is still running a background task"));
        // The same provider's own next message is no switch.
        assert_eq!(decide(Some((&live, true)), &live, true), Transition::Reuse);
    }

    #[test]
    fn a_new_reasoning_level_restarts_on_every_bridge() {
        for provider in crate::harness::discover::PROVIDERS {
            let live = selection(provider, "m", Some("low"));
            let target = selection(provider, "m", Some("high"));
            assert_eq!(
                decide(Some((&live, false)), &target, false),
                Transition::RestartAndResume,
                "{provider:?}"
            );
            let none = selection(provider, "m", None);
            assert_eq!(
                decide(Some((&live, false)), &none, false),
                Transition::RestartAndResume,
                "{provider:?}"
            );
        }
    }

    #[test]
    fn a_new_model_is_told_with_the_turn_where_the_bridge_can_and_restarts_where_not() {
        let switches = |provider| {
            let live = selection(provider, "one", Some("high"));
            let target = selection(provider, "two", Some("high"));
            decide(Some((&live, false)), &target, false)
        };
        assert_eq!(switches(Provider::Codex), Transition::SwitchModelInSession);
        assert_eq!(
            switches(Provider::Opencode),
            Transition::SwitchModelInSession
        );
        assert_eq!(switches(Provider::Claude), Transition::RestartAndResume);
        assert_eq!(
            switches(Provider::Antigravity),
            Transition::RestartAndResume
        );
        // Both changed: the level decides, since no bridge changes it in place.
        let live = selection(Provider::Codex, "one", Some("low"));
        let target = selection(Provider::Codex, "two", Some("high"));
        assert_eq!(
            decide(Some((&live, false)), &target, false),
            Transition::RestartAndResume
        );
    }

    #[test]
    fn the_same_selection_keeps_the_session() {
        for provider in crate::harness::discover::PROVIDERS {
            let live = selection(provider, "m", Some("high"));
            assert_eq!(
                decide(Some((&live, false)), &live.clone(), false),
                Transition::Reuse
            );
        }
    }

    fn moded(
        provider: Provider,
        live: PermissionMode,
        target: PermissionMode,
        model: Option<&str>,
    ) -> Transition {
        let caps = capabilities::of(provider);
        let running = selection(provider, "m", Some("high"));
        let next = selection(provider, model.unwrap_or("m"), Some("high"));
        let current = Current {
            selection: &running,
            mode: live,
            background: false,
            pending_request: false,
        };
        decide_transition(Some(current), (&next, target), &caps)
    }

    #[test]
    fn a_permission_mode_change_is_part_of_the_reuse_key() {
        use PermissionMode::{Ask, Auto, Quiet};
        // Codex takes the mode with each `turn/start`: the session stays.
        assert_eq!(
            moded(Provider::Codex, Ask, Auto, None),
            Transition::SwitchModeInSession
        );
        assert_eq!(
            moded(Provider::Codex, Quiet, Ask, Some("other")),
            Transition::SwitchModelInSession
        );
        assert_eq!(moded(Provider::Codex, Ask, Ask, None), Transition::Reuse);
        // A bridge with no approvals reads no mode: it is only recorded.
        for provider in [Provider::Opencode, Provider::Antigravity] {
            assert!(!capabilities::of(provider).approvals);
            assert_eq!(
                moded(provider, Ask, Quiet, None),
                Transition::SwitchModeInSession,
                "{provider:?}"
            );
        }
        // Claude fixes its mode at spawn (whether a command may leave the
        // sandbox is in its settings), so every change restarts it, and the
        // same mode keeps it.
        assert!(capabilities::of(Provider::Claude).approvals);
        for (from, to) in [(Ask, Auto), (Auto, Ask), (Ask, Quiet), (Quiet, Auto)] {
            assert_eq!(
                moded(Provider::Claude, from, to, None),
                Transition::RestartAndResume,
                "{from:?} to {to:?}"
            );
        }
        assert_eq!(moded(Provider::Claude, Auto, Auto, None), Transition::Reuse);
    }

    #[test]
    fn a_switch_away_from_a_session_with_a_request_waiting_is_refused() {
        let live = selection(Provider::Codex, "gpt", None);
        let target = selection(Provider::Claude, "opus", None);
        let caps = Capabilities {
            provider_handoff: true,
            ..capabilities::of(Provider::Claude)
        };
        let current = Current {
            selection: &live,
            mode: PermissionMode::Ask,
            background: false,
            pending_request: true,
        };
        assert_eq!(
            decide_transition(Some(current), (&target, PermissionMode::Ask), &caps),
            Transition::Reject(
                "Codex is waiting for you to answer a request in this chat. Answer it before switching."
                    .into()
            )
        );
    }
}
