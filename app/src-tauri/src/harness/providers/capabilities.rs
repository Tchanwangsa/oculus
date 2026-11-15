//! What each bridge can do, as one struct the runtime and the webview both
//! read (`harness_capabilities`, `app/src/lib/harness/capabilities.ts`). A
//! flag says what the bridge's own code does today; a flag for work a later
//! stage builds starts false and turns true only with the recording that
//! shows the CLI doing it ([`fixture`]).

use serde::Serialize;

use crate::harness::Provider;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub provider: Provider,
    /// Stop ends the running turn and the session answers with its terminal.
    pub interrupt: bool,
    /// The CLI can drop a question, and what followed, from its own context.
    pub rewind: bool,
    /// One server process answers every chat of this provider, so a forced
    /// stop of one chat ends the others' turns too.
    pub shared_process: bool,
    /// A message can be written into the running turn.
    pub active_steering: bool,
    /// A running turn can be stopped and started again with a correction,
    /// as one run.
    pub steer_by_restart: bool,
    /// A live session takes a different model with its next turn. Where
    /// false, the model is fixed at spawn and a change restarts the session
    /// and resumes its conversation.
    pub model_switch_in_session: bool,
    /// The bridge can take over a chat another provider started: it treats
    /// the earlier conversation at the top of its first prompt as history
    /// and answers the new message (`handoff/`).
    pub provider_handoff: bool,
    /// The CLI asks before what its sandbox blocks, and the student's
    /// answer goes back to the waiting call (`requests/`). Where false,
    /// such a step is declined without a prompt, whatever the chat's mode.
    pub approvals: bool,
    /// The CLI's own reviewer decides requests in the chat's Auto mode,
    /// and the student is never asked. Where false, Auto behaves as Ask.
    pub auto_review: bool,
    /// An answer can allow more than the one call ("Always allow this
    /// exact command in this chat"); each option's `scope` says how much.
    pub session_grants: bool,
    /// A live session takes a new permission mode with its next turn.
    /// Where false on a bridge with approvals, a mode change restarts it.
    pub mode_switch_in_session: bool,
}

/// One bridge's flags. Antigravity stops by killing its own child, which
/// needs nothing of the CLI, and has no way back to an earlier message.
pub const fn of(provider: Provider) -> Capabilities {
    let (rewind, shared_process) = match provider {
        Provider::Claude => (true, false),
        Provider::Codex | Provider::Opencode => (true, true),
        Provider::Antigravity => (false, false),
    };
    Capabilities {
        provider,
        interrupt: true,
        rewind,
        shared_process,
        active_steering: false,
        steer_by_restart: false,
        // Codex names the model in `turn/start` and opencode in each
        // prompt; Claude and `agy` take `--model` at spawn.
        model_switch_in_session: shared_process,
        provider_handoff: matches!(
            provider,
            Provider::Claude | Provider::Codex | Provider::Antigravity
        ),
        approvals: false,
        auto_review: false,
        session_grants: false,
        mode_switch_in_session: false,
    }
}

/// Every bridge's flags, in picker order.
pub fn all() -> Vec<Capabilities> {
    crate::harness::discover::PROVIDERS
        .iter()
        .map(|p| of(*p))
        .collect()
}

/// The recording under `fixtures/harness/` that shows a bridge doing what a
/// flag claims. A flag a later stage owns may be true only with one, which
/// this module's tests hold every bridge to.
#[cfg(test)]
pub const fn fixture(provider: Provider, flag: Flag) -> Option<&'static str> {
    match (provider, flag) {
        (Provider::Claude, Flag::Interrupt) => Some("claude-interrupt"),
        (Provider::Claude, Flag::ProviderHandoff) => Some("handoff-claude-full"),
        (Provider::Codex, Flag::ProviderHandoff) => Some("handoff-codex-full"),
        (Provider::Antigravity, Flag::ProviderHandoff) => Some("handoff-antigravity-full"),
        (Provider::Codex, Flag::Interrupt) => Some("codex-interrupt"),
        (Provider::Opencode, Flag::Interrupt) => Some("opencode-v1-interrupt"),
        _ => None,
    }
}

/// The flags a recording can vouch for.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    Interrupt,
    ActiveSteering,
    SteerByRestart,
    ProviderHandoff,
    Approvals,
    AutoReview,
    SessionGrants,
    ModeSwitchInSession,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recorded(name: &str) -> bool {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/harness")
            .join(format!("{name}.ndjson"))
            .exists()
    }

    #[test]
    fn a_later_stages_flag_is_true_only_with_its_recording() {
        for caps in all() {
            for (flag, on) in [
                (Flag::ActiveSteering, caps.active_steering),
                (Flag::SteerByRestart, caps.steer_by_restart),
                (Flag::ProviderHandoff, caps.provider_handoff),
                (Flag::Approvals, caps.approvals),
                (Flag::AutoReview, caps.auto_review),
                (Flag::SessionGrants, caps.session_grants),
                (Flag::ModeSwitchInSession, caps.mode_switch_in_session),
            ] {
                let named = fixture(caps.provider, flag);
                assert!(
                    !on || named.is_some_and(recorded),
                    "{:?} claims {flag:?} with no recording",
                    caps.provider
                );
            }
        }
    }

    #[test]
    fn every_named_recording_exists() {
        for caps in all() {
            for flag in [
                Flag::Interrupt,
                Flag::ActiveSteering,
                Flag::SteerByRestart,
                Flag::ProviderHandoff,
                Flag::Approvals,
                Flag::AutoReview,
                Flag::SessionGrants,
                Flag::ModeSwitchInSession,
            ] {
                if let Some(name) = fixture(caps.provider, flag) {
                    assert!(recorded(name), "{name} is not under fixtures/harness");
                }
            }
        }
    }

    #[test]
    fn todays_flags_are_what_the_bridges_do() {
        let rewinds: Vec<Provider> = all()
            .into_iter()
            .filter(|c| c.rewind)
            .map(|c| c.provider)
            .collect();
        assert_eq!(
            rewinds,
            [Provider::Claude, Provider::Codex, Provider::Opencode]
        );
        let shared: Vec<Provider> = all()
            .into_iter()
            .filter(|c| c.shared_process)
            .map(|c| c.provider)
            .collect();
        assert_eq!(shared, [Provider::Codex, Provider::Opencode]);
        assert!(all().iter().all(|c| c.interrupt));
        let wire = serde_json::to_value(of(Provider::Codex)).unwrap();
        assert_eq!(wire["provider"], "codex");
        assert_eq!(wire["sharedProcess"], true);
        assert_eq!(wire["activeSteering"], false);
        assert_eq!(wire["modelSwitchInSession"], true);
        assert_eq!(
            wire["providerHandoff"],
            of(Provider::Codex).provider_handoff
        );
        let in_session: Vec<Provider> = all()
            .into_iter()
            .filter(|c| c.model_switch_in_session)
            .map(|c| c.provider)
            .collect();
        assert_eq!(in_session, [Provider::Codex, Provider::Opencode]);
    }
}
