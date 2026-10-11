use super::{INPUT_WINDOW_SECS, MEDIA_WINDOW_SECS};

/// Why a tick is active. Input wins when both are recent: the page in use is
/// where the time goes, not a video playing beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    Input,
    Media,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tick {
    pub open: bool,
    /// `None` for an idle tick.
    pub active: Option<Presence>,
}

/// Whether one tick counts as open, and whether and why it counts as active.
pub fn classify(
    visible: bool,
    minimized: bool,
    focused: bool,
    now: u64,
    last_input: Option<u64>,
    last_media: Option<u64>,
) -> Tick {
    let within =
        |at: Option<u64>, window: u64| at.is_some_and(|at| now.saturating_sub(at) <= window);
    let open = visible && !minimized;
    let active = if !open {
        None
    } else if focused && within(last_input, INPUT_WINDOW_SECS) {
        Some(Presence::Input)
    } else if within(last_media, MEDIA_WINDOW_SECS) {
        Some(Presence::Media)
    } else {
        None
    };
    Tick { open, active }
}
