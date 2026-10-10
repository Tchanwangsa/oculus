//! One memory budget for every PDF render in the process, so the viewer and
//! the embedder together stay near 1 GB however many pages are asked for.
//!
//! A render reserves an estimate of its peak memory ([`estimate`]) before it
//! starts and gives it back when its [`Reservation`] drops — on success, error
//! or panic alike. A render estimated past the whole budget runs alone rather
//! than failing. There is no timeout: a reservation waits for one.
//!
//! The viewer goes first. The embedder never holds more than
//! [`EMBED_SHARE`] of the bytes or all of the slots, and waits while a viewer
//! render is queued, so scrolling stays live under a long index run.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock, PoisonError};

use crate::providers::ratelimit::hold;

/// Bytes all renders together may reserve. The rest of ~1 GB is what the
/// estimate cannot see: the open documents' bytes, hayro's caches, allocator
/// slack. Calibrated by `examples/render_memory.rs`.
pub const BUDGET_BYTES: u64 = 640 << 20;

/// Renders at once, at most: half the cores, so a small laptop stays
/// responsive, and never more than this.
pub const MAX_SLOTS: usize = 4;

/// The embedder's share of [`BUDGET_BYTES`], in quarters: the rest is the
/// viewer's, so a page in view never waits behind an index run's bytes.
pub const EMBED_SHARE: u64 = 3;

/// A render's fixed cost before its pixels: hayro's interpreter state and
/// the page's decoded fonts and images. Calibrated by
/// `examples/render_memory.rs`.
pub const BASE_BYTES: u64 = 16 << 20;

/// Bytes per output pixel: the RGBA pixmap (4), one more full-page layer per
/// soft mask or transparency group, and the PNG. Most pages peak near 5;
/// 16 covers 99% of the library's, so the estimate rarely runs short.
pub const BYTES_PER_PIXEL: u64 = 16;

/// Pixels past which a render runs alone: at 65 bytes per pixel, the worst
/// page measured, it alone could fill the budget. Only a zoomed-in viewer
/// render gets here; the embedder renders at most ~4M.
pub const ALONE_PIXELS: u64 = 8_000_000;

/// The peak memory a `width` × `height` render is reserved for; past
/// [`ALONE_PIXELS`], all of it.
pub fn estimate(width: u32, height: u32) -> u64 {
    let pixels = u64::from(width) * u64::from(height);
    if pixels > ALONE_PIXELS {
        return u64::MAX;
    }
    BASE_BYTES + BYTES_PER_PIXEL * pixels
}

/// Who is asking: the viewer is served first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    Viewer,
    Embedder,
}

impl Lane {
    fn index(self) -> usize {
        match self {
            Lane::Viewer => 0,
            Lane::Embedder => 1,
        }
    }
}

pub struct Budget {
    bytes: u64,
    slots: usize,
    embed_bytes: u64,
    embed_slots: usize,
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Default)]
struct State {
    held: u64,
    running: usize,
    embed_held: u64,
    embed_running: usize,
    /// Waiting tickets per lane, served in order so a large render is not
    /// overtaken forever by small ones.
    queues: [VecDeque<u64>; 2],
    next_ticket: u64,
}

impl Budget {
    /// `bytes` shared by at most `slots` renders at once.
    pub fn new(bytes: u64, slots: usize) -> Self {
        let slots = slots.max(1);
        Self {
            bytes: bytes.max(1),
            slots,
            embed_bytes: (bytes / 4 * EMBED_SHARE).max(1),
            // One slot stays the viewer's whenever there is more than one.
            embed_slots: slots.saturating_sub(1).max(1),
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        }
    }

    /// Renders the embedder may run at once.
    pub fn embed_slots(&self) -> usize {
        self.embed_slots
    }

    /// Waits until `estimate` bytes and a slot are free for `lane`, then holds
    /// them until the reservation drops.
    pub fn reserve(&self, lane: Lane, estimate: u64) -> Reservation<'_> {
        // Past the whole budget it waits for an empty budget and runs alone.
        let bytes = estimate.min(self.bytes);
        let mut state = hold(&self.state);
        let ticket = state.next_ticket;
        state.next_ticket += 1;
        state.queues[lane.index()].push_back(ticket);
        while !self.admits(&state, lane, ticket, bytes) {
            state = self.wait(state);
        }
        state.queues[lane.index()].pop_front();
        state.held += bytes;
        state.running += 1;
        if lane == Lane::Embedder {
            state.embed_held += bytes;
            state.embed_running += 1;
        }
        drop(state);
        // The next ticket in line may fit beside this one.
        self.changed.notify_all();
        Reservation {
            budget: self,
            lane,
            bytes,
        }
    }

    fn admits(&self, state: &State, lane: Lane, ticket: u64, bytes: u64) -> bool {
        let first = state.queues[lane.index()].front() == Some(&ticket);
        let fits = state.running < self.slots && state.held + bytes <= self.bytes;
        match lane {
            Lane::Viewer => first && fits,
            Lane::Embedder => {
                first
                    && fits
                    && state.queues[Lane::Viewer.index()].is_empty()
                    && state.embed_running < self.embed_slots
                    && (state.embed_held == 0 || state.embed_held + bytes <= self.embed_bytes)
            }
        }
    }

    fn wait<'a>(&self, state: MutexGuard<'a, State>) -> MutexGuard<'a, State> {
        self.changed
            .wait(state)
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn release(&self, lane: Lane, bytes: u64) {
        let mut state = hold(&self.state);
        state.held -= bytes;
        state.running -= 1;
        if lane == Lane::Embedder {
            state.embed_held -= bytes;
            state.embed_running -= 1;
        }
        drop(state);
        self.changed.notify_all();
    }

    #[cfg(test)]
    fn held(&self) -> (u64, usize) {
        let state = hold(&self.state);
        (state.held, state.running)
    }
}

/// Memory and a slot held for one render; dropping it gives both back.
pub struct Reservation<'a> {
    budget: &'a Budget,
    lane: Lane,
    bytes: u64,
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        self.budget.release(self.lane, self.bytes);
    }
}

static GLOBAL: OnceLock<Budget> = OnceLock::new();

/// The process's budget: [`BUDGET_BYTES`] over half the cores, at most
/// [`MAX_SLOTS`].
pub fn global() -> &'static Budget {
    GLOBAL.get_or_init(|| {
        let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
        Budget::new(BUDGET_BYTES, (cores / 2).clamp(1, MAX_SLOTS))
    })
}

/// Replaces the default before the first render, for measuring other
/// settings (`examples/render_memory.rs`). False once a render has run.
pub fn install(budget: Budget) -> bool {
    GLOBAL.set(budget).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    /// Spawns `work` and reports whether it is still blocked after a pause.
    fn blocked<F: FnOnce() + Send + 'static>(work: F) -> (std::thread::JoinHandle<()>, bool) {
        let done = Arc::new(AtomicUsize::new(0));
        let flag = done.clone();
        let handle = std::thread::spawn(move || {
            work();
            flag.store(1, Ordering::SeqCst);
        });
        std::thread::sleep(Duration::from_millis(80));
        (handle, done.load(Ordering::SeqCst) == 0)
    }

    #[test]
    fn a_reservation_is_given_back_when_it_drops() {
        let budget = Budget::new(1_000, 4);
        {
            let _a = budget.reserve(Lane::Viewer, 300);
            let _b = budget.reserve(Lane::Embedder, 200);
            assert_eq!(budget.held(), (500, 2));
        }
        assert_eq!(budget.held(), (0, 0));
    }

    #[test]
    fn a_panicking_render_still_gives_its_bytes_back() {
        let budget = Arc::new(Budget::new(1_000, 2));
        let inside = budget.clone();
        let result = std::thread::spawn(move || {
            let _held = inside.reserve(Lane::Embedder, 600);
            panic!("hayro fell over");
        })
        .join();
        assert!(result.is_err());
        assert_eq!(budget.held(), (0, 0));
        // And the next render is not stuck behind it.
        let _next = budget.reserve(Lane::Viewer, 1_000);
    }

    #[test]
    fn bytes_bound_what_runs_at_once() {
        let budget = Arc::new(Budget::new(1_000, 4));
        let first = budget.reserve(Lane::Viewer, 700);
        let waiting = budget.clone();
        let (handle, was_blocked) = blocked(move || {
            let _second = waiting.reserve(Lane::Viewer, 400);
        });
        assert!(was_blocked, "700 + 400 went over 1000");
        drop(first);
        handle.join().unwrap();
        assert_eq!(budget.held(), (0, 0));
    }

    #[test]
    fn slots_bound_what_runs_at_once() {
        let budget = Arc::new(Budget::new(1_000_000, 2));
        let a = budget.reserve(Lane::Viewer, 1);
        let b = budget.reserve(Lane::Viewer, 1);
        let waiting = budget.clone();
        let (handle, was_blocked) = blocked(move || {
            let _c = waiting.reserve(Lane::Viewer, 1);
        });
        assert!(was_blocked, "a third render ran in two slots");
        drop(a);
        handle.join().unwrap();
        drop(b);
    }

    #[test]
    fn an_oversized_render_runs_alone_instead_of_failing() {
        let budget = Arc::new(Budget::new(1_000, 4));
        let small = budget.reserve(Lane::Viewer, 10);
        let waiting = budget.clone();
        let (handle, was_blocked) = blocked(move || {
            let _huge = waiting.reserve(Lane::Viewer, 50_000);
            // Alone: it holds the whole budget.
            assert_eq!(waiting.held(), (1_000, 1));
        });
        assert!(was_blocked, "it ran beside another render");
        drop(small);
        handle.join().unwrap();
        assert_eq!(budget.held(), (0, 0));
    }

    #[test]
    fn a_large_render_is_not_overtaken_by_later_small_ones() {
        let budget = Arc::new(Budget::new(1_000, 4));
        let first = budget.reserve(Lane::Viewer, 600);
        let large = budget.clone();
        let (large_handle, large_blocked) = blocked(move || {
            let _large = large.reserve(Lane::Viewer, 600);
        });
        assert!(large_blocked);
        // Fits beside `first`, but the large one is ahead of it.
        let small = budget.clone();
        let (small_handle, small_blocked) = blocked(move || {
            let _small = small.reserve(Lane::Viewer, 100);
        });
        assert!(small_blocked, "a later small render jumped the queue");
        drop(first);
        large_handle.join().unwrap();
        small_handle.join().unwrap();
    }

    #[test]
    fn the_embedder_leaves_the_viewer_a_slot_and_a_quarter_of_the_bytes() {
        let budget = Arc::new(Budget::new(1_000, 3));
        assert_eq!(budget.embed_slots(), 2);
        let a = budget.reserve(Lane::Embedder, 300);
        let b = budget.reserve(Lane::Embedder, 300);
        // A third embed render has neither slot nor share.
        let embed = budget.clone();
        let (embed_handle, embed_blocked) = blocked(move || {
            let _c = embed.reserve(Lane::Embedder, 100);
        });
        assert!(embed_blocked);
        // The viewer still gets the remaining slot at once.
        let viewer = budget.reserve(Lane::Viewer, 300);
        drop(viewer);
        drop(a);
        embed_handle.join().unwrap();
        drop(b);

        // Bytes: past 3/4 the embedder waits even with slots free.
        let budget = Arc::new(Budget::new(1_000, 4));
        let a = budget.reserve(Lane::Embedder, 700);
        let embed = budget.clone();
        let (handle, embed_blocked) = blocked(move || {
            let _b = embed.reserve(Lane::Embedder, 100);
        });
        assert!(embed_blocked, "the embedder went past its share");
        let viewer = budget.reserve(Lane::Viewer, 250);
        drop(viewer);
        drop(a);
        handle.join().unwrap();
    }

    #[test]
    fn a_waiting_viewer_render_goes_before_the_embedder() {
        let budget = Arc::new(Budget::new(1_000, 1));
        assert_eq!(budget.embed_slots(), 1, "a one-slot machine still embeds");
        let running = budget.reserve(Lane::Embedder, 100);
        let order = Arc::new(Mutex::new(Vec::new()));

        let (viewer_budget, viewer_order) = (budget.clone(), order.clone());
        let (viewer, viewer_blocked) = blocked(move || {
            let _held = viewer_budget.reserve(Lane::Viewer, 100);
            hold(&viewer_order).push("viewer");
        });
        let (embed_budget, embed_order) = (budget.clone(), order.clone());
        let (embed, embed_blocked) = blocked(move || {
            let _held = embed_budget.reserve(Lane::Embedder, 100);
            hold(&embed_order).push("embedder");
        });
        assert!(viewer_blocked && embed_blocked);
        drop(running);
        viewer.join().unwrap();
        embed.join().unwrap();
        assert_eq!(*hold(&order), ["viewer", "embedder"]);
    }

    #[test]
    fn an_estimate_grows_with_the_pixels() {
        assert!(estimate(1, 1) >= BASE_BYTES);
        // A 200-DPI 16:9 slide, the embedder's largest, sits well inside the
        // embedder's share; a 16M-pixel viewer render runs alone.
        assert!(estimate(2667, 1500) < BUDGET_BYTES / 4);
        assert!(estimate(2828, 2828) < BUDGET_BYTES / 4 * EMBED_SHARE);
        assert_eq!(estimate(4000, 4000), u64::MAX);
        let budget = Budget::new(BUDGET_BYTES, 4);
        let _alone = budget.reserve(Lane::Viewer, estimate(4000, 4000));
        assert_eq!(budget.held(), (BUDGET_BYTES, 1));
    }
}
