//! App-usage tracking: open and active seconds per local hour in `usage_hours`,
//! and active seconds per hour, page kind and subject in `usage_context_hours`.
//!
//! Rust owns the clock. Every 30 s a ticker reads the main window's state and
//! credits the current hour; the frontend only reports activity through
//! `usage_activity`, which records a timestamp and the page it came from, and
//! never touches the database. Open = the window is visible and not minimized.
//! Active = open, and either focused with recent input or playing media; the
//! active seconds go to the context of whichever made the tick active. See
//! `docs/architecture.md`.

mod classify;
mod store;
#[cfg(test)]
mod tests;
mod ticker;

pub use ticker::start;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use tauri::State;

use classify::Presence;

/// Seconds between ticks, and the seconds each counted tick adds.
const TICK_SECS: u64 = 30;
/// Input counts while the latest ping is this recent; the frontend throttles
/// input pings to one per 30 s, so this tolerates a few missed ones.
const INPUT_WINDOW_SECS: u64 = 120;
/// Media pings arrive every 30 s while playing; two missed ones ends it.
const MEDIA_WINDOW_SECS: u64 = 60;

/// The `UsageKind`s of `app/src/lib/activity/usageContext.ts`, stored as written.
const KINDS: [&str; 8] = [
    "lecture", "file", "document", "course", "chat", "browser", "planning", "other",
];

/// The page a ping came from: its usage kind and the subject it belongs to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageContext {
    pub kind: String,
    /// `None` outside a subject; stored as 0.
    pub subject_id: Option<i64>,
}

impl UsageContext {
    /// Where active time goes before any ping has carried a context.
    fn other() -> Self {
        UsageContext {
            kind: "other".into(),
            subject_id: None,
        }
    }
}

/// Managed state: wall-clock seconds of the latest ping of each kind, 0 for
/// none, and the latest context each kind carried. Wall clock, not `Instant`,
/// because `Instant` stops during macOS sleep and a pre-sleep ping would look
/// recent on wake.
#[derive(Default)]
pub struct UsageState {
    last_input: AtomicU64,
    last_media: AtomicU64,
    input_context: Mutex<Option<UsageContext>>,
    media_context: Mutex<Option<UsageContext>>,
}

impl UsageState {
    /// Store a ping at `now`. A ping without a context keeps the one before.
    fn ping(&self, kind: &str, context: Option<UsageContext>, now: u64) -> Result<(), String> {
        let (slot, held) = match kind {
            "input" => (&self.last_input, &self.input_context),
            "media" => (&self.last_media, &self.media_context),
            other => return Err(format!("unknown activity kind: {other}")),
        };
        if let Some(context) = context {
            if !KINDS.contains(&context.kind.as_str()) {
                return Err(format!("unknown usage kind: {}", context.kind));
            }
            *held.lock().unwrap() = Some(context);
        }
        slot.store(now, Ordering::Relaxed);
        Ok(())
    }

    /// The context an active tick credits: the latest one carried by the pings
    /// that made it active, else `other` outside any subject.
    fn credited(&self, presence: Presence) -> UsageContext {
        let held = match presence {
            Presence::Input => &self.input_context,
            Presence::Media => &self.media_context,
        };
        held.lock()
            .unwrap()
            .clone()
            .unwrap_or_else(UsageContext::other)
    }
}

/// Record that the user is present: `input` (mouse or keyboard) or `media`
/// (a video is playing), from the page `context` describes.
#[tauri::command]
pub fn usage_activity(
    state: State<'_, UsageState>,
    kind: String,
    context: Option<UsageContext>,
) -> Result<(), String> {
    state.ping(&kind, context, crate::runtime::clock::now_secs())
}
