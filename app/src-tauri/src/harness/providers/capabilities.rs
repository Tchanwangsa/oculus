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
            for flag in [Flag::Interrupt, Flag::ActiveSteering, Flag::SteerByRestart] {
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
    }
}
