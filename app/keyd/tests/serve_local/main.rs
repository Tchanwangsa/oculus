//! The built binary end to end through its debug-only `serve-local` hook:
//! a private socket, data dir and master key, never launchd or the keychain.
//! Without the `dev` feature this test process is refused, because it is not
//! inside keyd's app bundle; with it, the ops run.

mod forward;
mod harness;
mod lifecycle;
mod okta;
