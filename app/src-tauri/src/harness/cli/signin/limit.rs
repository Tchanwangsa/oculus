//! Whether a provider's error text means "your plan's usage limit is spent".
//!
//! Each bridge's list holds only wording recorded from that CLI (a replay
//! fixture or a captured message in the repo). None has been recorded yet, so
//! every list is empty and nothing is marked: a guessed phrase would put a
//! "Usage limit reached" banner over an ordinary failure. Add a phrase here
//! together with the recording it came from and its test.

use crate::harness::event::Provider;

const CLAUDE: &[&str] = &[];
const CODEX: &[&str] = &[];
const OPENCODE: &[&str] = &[];
const ANTIGRAVITY: &[&str] = &[];

/// Whether `msg` is `provider` reporting a spent usage limit. Whole clauses,
/// never words: "limit" alone is in compiler errors and file listings.
pub fn is_usage_limit(provider: Provider, msg: &str) -> bool {
    let own = match provider {
        Provider::Claude => CLAUDE,
        Provider::Codex => CODEX,
        Provider::Opencode => OPENCODE,
        Provider::Antigravity => ANTIGRAVITY,
    };
    let m = msg.to_lowercase();
    own.iter().any(|n| m.contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every line each bridge's recorded sessions hold, its error session
    /// included: none of it is a usage limit.
    #[test]
    fn no_recorded_line_is_a_limit() {
        let fixtures: [(Provider, &[&str]); 4] = [
            (
                Provider::Claude,
                &[
                    include_str!("../../../../fixtures/harness/claude-ls.ndjson"),
                    include_str!("../../../../fixtures/harness/claude-interrupt.ndjson"),
                    include_str!("../../../../fixtures/harness/claude-background-agent.ndjson"),
                ],
            ),
            (
                Provider::Codex,
                &[
                    include_str!("../../../../fixtures/harness/codex-ls.ndjson"),
                    include_str!("../../../../fixtures/harness/codex-interrupt.ndjson"),
                    include_str!("../../../../fixtures/harness/codex-websearch.ndjson"),
                ],
            ),
            (
                Provider::Opencode,
                &[
                    include_str!("../../../../fixtures/harness/opencode-v1-error.ndjson"),
                    include_str!("../../../../fixtures/harness/opencode-v1-ls.ndjson"),
                    include_str!("../../../../fixtures/harness/opencode-v1-interrupt.ndjson"),
                ],
            ),
            (
                Provider::Antigravity,
                &[include_str!(
                    "../../../../fixtures/harness/antigravity-acp-ls.ndjson"
                )],
            ),
        ];
        for (provider, files) in fixtures {
            for line in files.iter().flat_map(|f| f.lines()) {
                assert!(
                    !is_usage_limit(provider, line),
                    "{provider:?} marked {line:?}"
                );
            }
        }
    }

    /// Until a bridge's limit message is recorded, its classifier says no,
    /// even to text that sounds like one.
    #[test]
    fn unrecorded_bridges_mark_nothing() {
        for provider in [
            Provider::Claude,
            Provider::Codex,
            Provider::Opencode,
            Provider::Antigravity,
        ] {
            for msg in [
                "rate limit reached, retry after 30s",
                "usage limit reached",
                "429 Too Many Requests",
            ] {
                assert!(!is_usage_limit(provider, msg), "{provider:?} {msg:?}");
            }
        }
    }
}
