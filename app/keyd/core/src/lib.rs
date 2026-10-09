//! `oculus-keyd`'s logic, apart from any OS: the vault, the wire format, the
//! ops, `forward`, the server loop and the client. Every OS call sits behind
//! `platform`, whose adapter is picked by target (docs/architecture.md).
//!
//! keyd links it with `server`; the app and CLI link it with `client`, so
//! neither links the other and an app edit never changes keyd's bytes. The
//! framing, the secret names, the paths and the platform's base types are
//! always on.

pub mod framing;
pub mod names;
pub mod paths;
pub mod platform;

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
pub mod forward;
#[cfg(feature = "server")]
pub mod ops;
#[cfg(feature = "server")]
pub mod server;
#[cfg(feature = "server")]
pub mod vault;

#[cfg(all(test, feature = "server"))]
mod test_support;

/// One line of keyd's log (stderr, which the agent sends to a file): the
/// time, the pid and `msg`. Never a value, a header or a body.
#[cfg(feature = "server")]
pub fn log(msg: &str) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    eprintln!("[{t:.3}] pid={} {msg}", std::process::id());
}
