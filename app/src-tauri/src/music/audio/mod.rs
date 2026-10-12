//! Where Spotify's audio goes on this Mac: the system default, or one output
//! device by name. The choice is kept here (`Choice`) and in the `music`
//! settings row; the player's sink (`sink.rs`) reads it on every write, so a
//! change takes effect within a packet without rebuilding the player, and the
//! queue and position stay as they were.
//!
//! "System default" needs no watching: cpal opens the default device as
//! CoreAudio's default-output unit, which follows the Mac's default output by
//! itself. A named device that disappears falls back to the default until it
//! comes back. See `docs/music.md`.

mod device;
mod sink;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::Serialize;

pub(super) use device::list;
pub(super) use sink::{Note, Report, Switching};

/// The output the user chose (`None` is the system default), with a
/// generation the sink compares against to notice a change.
#[derive(Default)]
pub struct Choice {
    name: Mutex<Option<String>>,
    gen: AtomicU64,
}

impl Choice {
    pub(super) fn get(&self) -> (Option<String>, u64) {
        let name = self.name.lock().unwrap_or_else(|e| e.into_inner()).clone();
        (name, self.gen())
    }

    pub(super) fn gen(&self) -> u64 {
        self.gen.load(Ordering::Acquire)
    }

    /// True when it changed.
    pub(super) fn set(&self, name: Option<String>) -> bool {
        let name = name.filter(|n| !n.trim().is_empty());
        let mut held = self.name.lock().unwrap_or_else(|e| e.into_inner());
        if *held == name {
            return false;
        }
        *held = name;
        self.gen.fetch_add(1, Ordering::AcqRel);
        true
    }
}

/// One CoreAudio output device (`music_outputs`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OutputDevice {
    pub name: String,
    /// The Mac's default output right now.
    pub is_default: bool,
}

/// What the sink binds to: the chosen device when it is present, else the
/// system default (`None`). The flag is true when a chosen device is missing.
pub(super) fn resolve(chosen: Option<&str>, present: &[String]) -> (Option<String>, bool) {
    match chosen {
        Some(name) if present.iter().any(|p| p == name) => (Some(name.to_string()), false),
        Some(_) => (None, true),
        None => (None, false),
    }
}

/// The snapshot error while a chosen device is missing.
pub(super) fn fallback_message(name: &str) -> String {
    format!("{name} is not available; playing on the system default")
}
