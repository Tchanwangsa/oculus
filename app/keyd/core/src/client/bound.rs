//! Which university this process's sessions belong to. keyd sends a session
//! only to its own site, and a keyd that was never told the profile's site,
//! or is too old to be told, would send it to UniMelb's. So a process that
//! knows its profile (the app and the CLI) registers a check with `bind`,
//! and every call that hands keyd a credential or has it use one asks the
//! check first: `session_put`, `session_get`, `okta_save`,
//! `ensure_signed_in`, and `send`/`send_stream` on the `canvas` and `ed`
//! routes. The check answers the Canvas origin keyd has taken, which
//! `session_put` and `forward` carry for keyd to compare with its site
//! (`ops::site::check_claim`), or the error that stops the call.

use std::sync::OnceLock;

use super::KeydError;

/// The Canvas origin keyd has taken this profile's site for, or why not.
pub type Check = fn() -> Result<String, KeydError>;

static CHECK: OnceLock<Check> = OnceLock::new();

/// Registers the process's check. The first registration stands.
pub fn bind(check: Check) {
    CHECK.set(check).ok();
}

/// The origin to name in a request, or `None` when no check is registered
/// (a test, or a caller that holds no profile).
pub(super) fn canvas() -> Result<Option<String>, KeydError> {
    CHECK.get().map(|check| check()).transpose()
}

/// Whether `route` carries a login session.
pub(super) fn is_session_route(route: &str) -> bool {
    matches!(route, "canvas" | "ed")
}
