use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

use crate::shell::browser::MAIN;

use super::classify::{classify, Tick};
use super::store::{credit, hour_key};
use super::{UsageContext, UsageState, TICK_SECS};

/// Start the ticker. Ticks before the database or table exists (the frontend's
/// first load applies the migrations) fail quietly and later ticks retry.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let period = Duration::from_secs(TICK_SECS);
        // The first tick is one period in, so a launch doesn't credit time it hasn't had.
        let mut interval = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
        // A wake from sleep resumes the cadence rather than counting the missed ticks.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut pool: Option<SqlitePool> = None;
        let mut failing = false;
        loop {
            interval.tick().await;
            let Some((tick, context)) = sample(&app).await else {
                continue;
            };
            // Active implies open, so a closed tick has nothing to add.
            if !tick.open {
                continue;
            }
            match record(&mut pool, tick, context.as_ref()).await {
                Ok(()) => failing = false,
                Err(e) => {
                    if !failing {
                        eprintln!("[oculus] usage: {e}");
                    }
                    failing = true;
                }
            }
        }
    });
}

/// Classify this tick from the main window and the latest pings, with the
/// context an active tick credits; `None` when the window is gone or won't
/// answer.
async fn sample(app: &AppHandle) -> Option<(Tick, Option<UsageContext>)> {
    let window = app.get_window(MAIN)?;
    // Each getter blocks on a round trip to the main thread, so keep them off
    // the async workers.
    let (visible, minimized, focused) = tauri::async_runtime::spawn_blocking(move || {
        Some((
            window.is_visible().ok()?,
            window.is_minimized().ok()?,
            window.is_focused().ok()?,
        ))
    })
    .await
    .ok()??;
    let state = app.state::<UsageState>();
    let ping = |slot: &AtomicU64| Some(slot.load(Ordering::Relaxed)).filter(|&at| at > 0);
    let tick = classify(
        visible,
        minimized,
        focused,
        crate::runtime::clock::now_secs(),
        ping(&state.last_input),
        ping(&state.last_media),
    );
    Some((tick, tick.active.map(|presence| state.credited(presence))))
}

async fn record(
    pool: &mut Option<SqlitePool>,
    tick: Tick,
    context: Option<&UsageContext>,
) -> Result<(), String> {
    if pool.is_none() {
        *pool = Some(crate::db::store::open_pool().await?);
    }
    let Some(pool) = pool.as_ref() else {
        return Ok(());
    };
    let hour = hour_key(chrono::Local::now().naive_local());
    credit(pool, &hour, tick, context).await
}
