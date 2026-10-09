//! Background session keep-alive via a macOS LaunchAgent.
//!
//! The Canvas session is extended on use, so a periodic ping keeps it alive.
//! The in-app timer in `lib.rs` covers "Oculus is open"; launchd covers the
//! rest by running `oculus auth tick`, which shares the probe, cookie merge and
//! headless sign-in with the app. See `docs/auth.md`.

pub(crate) mod commands;
mod install;
mod launchctl;
mod plist;
mod status;

#[cfg(test)]
mod tests;

pub use install::{ensure_installed, repair_path};
pub use status::KeepaliveStatus;

pub const LABEL: &str = "com.tchan.oculus.session-keepalive";
const DEFAULT_INTERVAL_HOURS: u32 = 6;
