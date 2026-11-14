//! Discarding idle background tabs, as Safari and Chrome's Memory Saver do.
//!
//! A tab no pane or side panel has shown for `IDLE` loses its page through the
//! same teardown as closing it; on macOS, memory pressure discards every
//! backgrounded tab at once. The tab itself stays in `browser-state` (id, URL,
//! title, zoom), so the strip is unchanged, and showing it again
//! (`browser_place`) loads its URL in a new page made the way a new tab's is.
//! A page playing media or capturing the camera or microphone is kept.
//! One thread ticks every `TICK` for all tabs.

use std::time::{Duration, Instant};

use tauri::AppHandle;

use super::layout::{create_page, page, parse};
use super::seed::wants_sessions;
use super::teardown::{destroy, in_use};
use super::{broadcast, seed_sessions, with_state, Inner};

/// How long a tab stays backgrounded before its page is discarded.
pub(super) const IDLE: Duration = Duration::from_secs(15 * 60);
/// How often the tick looks; a tab goes at most this long past `IDLE`.
const TICK: Duration = Duration::from_secs(60);

/// Whether tab `id` may lose its page at `now`: it exists, is not shown
/// anywhere, still has one, is not downloading, and has been backgrounded
/// for at least `after`.
pub(super) fn is_due(s: &Inner, id: u32, now: Instant, after: Duration) -> bool {
    s.tabs.iter().any(|t| t.id == id)
        && !s.shown.contains(&id)
        && !s.discarded.contains(&id)
        && !s.downloads.unfinished_in(id)
        && s.backgrounded
            .get(&id)
            .is_some_and(|&at| now.saturating_duration_since(at) >= after)
}

/// Every tab `is_due` at `now`.
pub(super) fn due(s: &Inner, now: Instant, after: Duration) -> Vec<u32> {
    s.tabs
        .iter()
        .map(|t| t.id)
        .filter(|&id| is_due(s, id, now, after))
        .collect()
}

/// Starts the tick and, on macOS, the memory-pressure source. From `init`.
pub(super) fn start(app: &AppHandle) {
    let tick_app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("browser-discard".into())
        .spawn(move || loop {
            std::thread::sleep(TICK);
            sweep(&tick_app, IDLE);
        });
    if let Err(e) = spawned {
        elog!("[oculus] browser: no discard tick: {e}");
    }
    #[cfg(target_os = "macos")]
    on_memory_pressure(app);
}

/// Asks each due page whether it is in use, and discards the ones that are not.
fn sweep(app: &AppHandle, after: Duration) {
    let ids = with_state(app, |s| due(s, Instant::now(), after));
    for id in ids {
        // Still being made (seeding the jar, or restoring).
        let Some(webview) = page(app, id) else {
            continue;
        };
        let app = app.clone();
        in_use(&webview, move |busy| {
            if busy {
                return;
            }
            // macOS answers inside a WebKit callback; teardown runs on the
            // main thread, after it, so `browser_place` cannot interleave.
            std::thread::spawn(move || {
                let main = app.clone();
                main.run_on_main_thread(move || discard(&app, id, after))
                    .ok();
            });
        });
    }
}

/// Main thread. Checks again: the tab may have been shown, closed or
/// already discarded since. A tab with no page is being made; leave it.
fn discard(app: &AppHandle, id: u32, after: Duration) {
    let Some(webview) = page(app, id) else {
        return;
    };
    let due = with_state(app, |s| {
        if !is_due(s, id, Instant::now(), after) {
            return false;
        }
        s.discarded.insert(id);
        if let Some(tab) = s.tabs.iter_mut().find(|t| t.id == id) {
            tab.loading = false;
        }
        true
    });
    if !due {
        return;
    }
    destroy(&webview);
    elog!("[oculus] browser: discarded background tab {id}");
    broadcast(app);
}

/// Gives a discarded tab a page again, at the URL it was on; a no-op for any
/// other tab. The page is made on a worker thread, as `browser_open_url`
/// makes one: Windows deadlocks making a page on the event loop.
pub(super) fn restore(app: &AppHandle, id: u32) {
    let Some(url) = with_state(app, |s| {
        if !s.discarded.remove(&id) {
            return None;
        }
        s.tabs.iter().find(|t| t.id == id).map(|t| t.url.clone())
    }) else {
        return;
    };
    // A page can reach a scheme a tab never opens; that one comes back blank.
    let Some(url) = parse(&url).ok().or_else(|| "about:blank".parse().ok()) else {
        return;
    };
    elog!("[oculus] browser: restoring tab {id} → {url}");
    if wants_sessions(&url) {
        let seeded_app = app.clone();
        seed_sessions(app, move || recreate(&seeded_app, id, url));
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || recreate(&app, id, url));
}

fn recreate(app: &AppHandle, id: u32, url: url::Url) {
    // Closed while the jar was being seeded.
    if !with_state(app, |s| s.tabs.iter().any(|t| t.id == id)) {
        return;
    }
    if let Err(e) = create_page(app, id, url) {
        elog!("[oculus] browser: tab {id}: could not restore: {e}");
        // The next time it is shown tries again.
        with_state(app, |s| s.discarded.insert(id));
    }
}

/// Discards every backgrounded tab, however recently hidden, when the system
/// reports memory pressure (warning or critical).
#[cfg(target_os = "macos")]
fn on_memory_pressure(app: &AppHandle) {
    use block2::RcBlock;
    use dispatch2::{
        dispatch_source_memorypressure_flags_t as Flags, DispatchObject, DispatchQoS,
        DispatchQueue, DispatchSource, GlobalQueueIdentifier,
    };

    let queue = DispatchQueue::global_queue(GlobalQueueIdentifier::QualityOfService(
        DispatchQoS::Utility,
    ));
    let mask = (Flags::DISPATCH_MEMORYPRESSURE_WARN.0 | Flags::DISPATCH_MEMORYPRESSURE_CRITICAL.0)
        as usize;
    // SAFETY: the memory-pressure type takes no handle and a flags mask.
    let source = unsafe {
        DispatchSource::new(
            &raw const dispatch2::_dispatch_source_type_memorypressure as *mut _,
            0,
            mask,
            Some(&queue),
        )
    };
    let app = app.clone();
    let handler = RcBlock::new(move || {
        elog!("[oculus] browser: memory pressure; discarding background tabs");
        sweep(&app, Duration::ZERO);
    });
    // SAFETY: dispatch copies the block.
    unsafe { source.set_event_handler_with_block(RcBlock::as_ptr(&handler)) };
    source.resume();
    // Lives as long as the app.
    std::mem::forget(source);
}
