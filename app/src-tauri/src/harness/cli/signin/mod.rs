//! Signing in to a CLI agent, from the error row that says you are not.
//!
//! [`is_auth_failure`] classifies a provider's error text; its lists stay
//! tight because a false "sign in again" is worse than a miss. [`start`] /
//! [`submit_code`] / [`cancel`] drive the CLI's own login flow as a
//! subprocess (the discovered binary with [`discover::child_env`](super::discover::child_env), never a
//! shell), streamed like `install`; nothing here holds a token. There is
//! no deadline — [`cancel`] ends a flow.
//!
//! `claude auth login` blocks reading a pasted code from stdin; `codex login`
//! finishes on its own loopback listener (port 1455). opencode signs in per
//! provider through its server (`harness_opencode_*`), so it is not here.
//! [`status`] is never cached: it is read where a stale "signed out" would
//! be the one answer that must not be wrong.

mod classify;
mod flow;
mod output;
mod status;
#[cfg(test)]
mod tests;

pub use classify::is_auth_failure;
pub use flow::{cancel, start, submit_code, SignInLine, SIGNIN_EVENT};
pub use status::{status, SignInStatus};
