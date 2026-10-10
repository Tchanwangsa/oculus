//! The adapter contract: everything keyd and its clients need from an OS,
//! and the only place OS calls are made. The adapter is picked by target;
//! each delivers the same contract and never varies it.
//!
//! 1. Endpoint: `Listener::bind`, `accept_any` and `connect` over a `Conn`
//!    only this user can reach; `connect` is `Absent` when nothing is there.
//! 2. Activation: `activated()`, the listeners the OS made when the first
//!    connect started keyd, outside the caller's sandbox.
//! 3. PeerCheck: `inspect` (same user, which executable, its `Role`) and
//!    `admit`, both before a byte of the request is read.
//! 4, 5. `master_key()` and `legacy_items()`: the OS secret store.
//! 6. `registrar()`: installs, reports and removes what starts keyd, and
//!    retires a registration an earlier Oculus made.
//! 7. Paths are `crate::paths`; 8. the build step is the adapter's part of
//!    `app/scripts/build-keyd.mjs`. `files` holds the file helpers.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;
#[cfg(not(target_os = "macos"))]
mod unsupported;
#[cfg(not(target_os = "macos"))]
use unsupported as imp;

/// Exclusive locks, owner-only files, durable renames: the POSIX ones on any
/// Unix, whatever its adapter.
#[cfg(unix)]
#[path = "unix.rs"]
pub mod files;
#[cfg(not(unix))]
pub use unsupported::files;

// Without a feature nothing serves or connects, so the listener goes unused.
#[cfg(test)]
#[cfg_attr(not(any(feature = "server", feature = "client")), allow(dead_code))]
pub(crate) mod memory;

mod caller;
mod endpoint;
#[cfg(feature = "client")]
mod registration;
#[cfg(feature = "server")]
mod secret_store;

#[cfg(feature = "server")]
pub use caller::peer_check;
pub use caller::{Caller, PeerCheck, Policy, Role};
#[cfg(test)]
pub(crate) use endpoint::Accept;
#[cfg(any(test, all(feature = "server", target_os = "macos")))]
pub(crate) use endpoint::Stream;
pub use endpoint::{accept_any, activated, connect, Conn, ConnectError, Listener};
#[cfg(feature = "client")]
pub use registration::{registrar, Registrar, Registration};
#[cfg(feature = "server")]
pub use secret_store::{legacy_items, master_key};
