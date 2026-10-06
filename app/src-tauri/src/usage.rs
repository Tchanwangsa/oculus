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

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, State};

use crate::browser::MAIN;

/// Seconds between ticks, and the seconds each counted tick adds.
const TICK_SECS: u64 = 30;
/// Input counts while the latest ping is this recent; the frontend throttles
/// input pings to one per 30 s, so this tolerates a few missed ones.
const INPUT_WINDOW_SECS: u64 = 120;
/// Media pings arrive every 30 s while playing; two missed ones ends it.
const MEDIA_WINDOW_SECS: u64 = 60;

/// The `UsageKind`s of `app/src/lib/usageContext.ts`, stored as written.
const KINDS: [&str; 8] = ["lecture", "file", "document", "course", "chat", "browser", "planning", "other"];

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
        UsageContext { kind: "other".into(), subject_id: None }
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
        held.lock().unwrap().clone().unwrap_or_else(UsageContext::other)
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
    state.ping(&kind, context, crate::clock::now_secs())
}

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
    let within = |at: Option<u64>, window: u64| at.is_some_and(|at| now.saturating_sub(at) <= window);
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

/// The `usage_hours` key for a local time: `YYYY-MM-DD HH`.
pub fn hour_key(local: chrono::NaiveDateTime) -> String {
    local.format("%Y-%m-%d %H").to_string()
}

/// Add seconds to an hour's row, creating it if needed.
pub async fn add<'e, E>(executor: E, hour: &str, open_seconds: i64, active_seconds: i64) -> Result<(), String>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "INSERT INTO usage_hours (hour, open_seconds, active_seconds) VALUES (?1, ?2, ?3)
         ON CONFLICT(hour) DO UPDATE SET
             open_seconds = open_seconds + excluded.open_seconds,
             active_seconds = active_seconds + excluded.active_seconds",
    )
    .bind(hour)
    .bind(open_seconds)
    .bind(active_seconds)
    .execute(executor)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Add active seconds to an hour's row for `context`, creating it if needed.
pub async fn add_context<'e, E>(
    executor: E,
    hour: &str,
    context: &UsageContext,
    active_seconds: i64,
) -> Result<(), String>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "INSERT INTO usage_context_hours (hour, kind, subject_id, active_seconds) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(hour, kind, subject_id) DO UPDATE SET
             active_seconds = active_seconds + excluded.active_seconds",
    )
    .bind(hour)
    .bind(&context.kind)
    .bind(context.subject_id.unwrap_or(0))
    .bind(active_seconds)
    .execute(executor)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Credit one tick to its hour and, when active, to `context`, in one
/// transaction so the two tables never disagree.
pub async fn credit(
    pool: &SqlitePool,
    hour: &str,
    tick: Tick,
    context: Option<&UsageContext>,
) -> Result<(), String> {
    let seconds = |counted: bool| if counted { TICK_SECS as i64 } else { 0 };
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    add(&mut *tx, hour, seconds(tick.open), seconds(tick.active.is_some())).await?;
    if let (Some(_), Some(context)) = (tick.active, context) {
        add_context(&mut *tx, hour, context, TICK_SECS as i64).await?;
    }
    tx.commit().await.map_err(|e| e.to_string())
}

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
            let Some((tick, context)) = sample(&app).await else { continue };
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
        Some((window.is_visible().ok()?, window.is_minimized().ok()?, window.is_focused().ok()?))
    })
    .await
    .ok()??;
    let state = app.state::<UsageState>();
    let ping = |slot: &AtomicU64| Some(slot.load(Ordering::Relaxed)).filter(|&at| at > 0);
    let tick = classify(
        visible,
        minimized,
        focused,
        crate::clock::now_secs(),
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
        *pool = Some(crate::store::open_pool().await?);
    }
    let Some(pool) = pool.as_ref() else { return Ok(()) };
    let hour = hour_key(chrono::Local::now().naive_local());
    credit(pool, &hour, tick, context).await
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::Row;

    use super::*;

    const NOW: u64 = 1_800_000_000;

    #[test]
    fn open_needs_a_visible_unminimized_window() {
        assert!(classify(true, false, false, NOW, None, None).open);
        assert!(!classify(false, false, true, NOW, Some(NOW), Some(NOW)).open);
        assert!(!classify(true, true, true, NOW, Some(NOW), Some(NOW)).open);
    }

    #[test]
    fn active_needs_focus_with_recent_input_or_recent_media() {
        let active = |focused, input, media| classify(true, false, focused, NOW, input, media).active;
        assert_eq!(active(true, Some(NOW - 120), None), Some(Presence::Input));
        assert_eq!(active(true, Some(NOW - 121), None), None);
        assert_eq!(active(false, Some(NOW), None), None, "input in an unfocused window is not presence");
        assert_eq!(active(false, None, Some(NOW - 60)), Some(Presence::Media), "playing media counts without focus");
        assert_eq!(active(true, None, Some(NOW - 61)), None);
        assert_eq!(active(true, None, None), None);
    }

    #[test]
    fn input_wins_over_media_and_media_covers_an_unfocused_window() {
        let active = |focused, input, media| classify(true, false, focused, NOW, input, media).active;
        assert_eq!(active(true, Some(NOW), Some(NOW)), Some(Presence::Input));
        assert_eq!(active(false, Some(NOW), Some(NOW)), Some(Presence::Media));
        assert_eq!(active(true, Some(NOW - 121), Some(NOW)), Some(Presence::Media));
    }

    #[test]
    fn nothing_is_active_while_closed() {
        assert_eq!(classify(false, false, true, NOW, Some(NOW), Some(NOW)), Tick { open: false, active: None });
        assert_eq!(classify(true, true, false, NOW, None, Some(NOW)), Tick { open: false, active: None });
    }

    fn context(kind: &str, subject_id: Option<i64>) -> UsageContext {
        UsageContext { kind: kind.into(), subject_id }
    }

    #[test]
    fn each_ping_kind_keeps_its_own_latest_context() {
        let state = UsageState::default();
        assert_eq!(state.credited(Presence::Input), context("other", None), "nothing reported yet");

        state.ping("input", Some(context("file", Some(7))), NOW).unwrap();
        state.ping("media", Some(context("lecture", Some(3))), NOW).unwrap();
        state.ping("input", None, NOW + 30).unwrap();
        assert_eq!(state.credited(Presence::Input), context("file", Some(7)), "a bare ping keeps the context");
        assert_eq!(state.credited(Presence::Media), context("lecture", Some(3)));
        assert_eq!(state.last_input.load(Ordering::Relaxed), NOW + 30);
    }

    #[test]
    fn unknown_kinds_are_refused_without_recording_the_ping() {
        let state = UsageState::default();
        assert!(state.ping("scroll", None, NOW).is_err());
        assert!(state.ping("input", Some(context("game", None)), NOW).is_err());
        assert_eq!(state.last_input.load(Ordering::Relaxed), 0);
        assert_eq!(state.credited(Presence::Input), context("other", None));
    }

    #[test]
    fn contexts_deserialize_from_the_frontend_shape() {
        let parsed: UsageContext = serde_json::from_str(r#"{"kind":"course","subjectId":12}"#).unwrap();
        assert_eq!(parsed, context("course", Some(12)));
        let parsed: UsageContext = serde_json::from_str(r#"{"kind":"chat","subjectId":null}"#).unwrap();
        assert_eq!(parsed, context("chat", None));
    }

    #[test]
    fn hour_keys_are_zero_padded_local_hours() {
        let at = |y, m, d, h, min| {
            chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, min, 59).unwrap()
        };
        assert_eq!(hour_key(at(2026, 3, 4, 5, 59)), "2026-03-04 05");
        assert_eq!(hour_key(at(2026, 12, 31, 23, 0)), "2026-12-31 23");
    }

    async fn pool_with(versions: &[i64]) -> SqlitePool {
        let pool = SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        for migration in crate::migrations::all().into_iter().filter(|m| versions.contains(&m.version)) {
            sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn adding_accumulates_into_the_hours_row() {
        let pool = pool_with(&[40]).await;

        add(&pool, "2026-10-06 14", 30, 30).await.unwrap();
        add(&pool, "2026-10-06 14", 30, 0).await.unwrap();
        add(&pool, "2026-10-06 15", 30, 0).await.unwrap();

        let rows = sqlx::query("SELECT hour, open_seconds, active_seconds FROM usage_hours ORDER BY hour")
            .fetch_all(&pool).await.unwrap();
        let rows: Vec<(String, i64, i64)> =
            rows.iter().map(|r| (r.get(0), r.get(1), r.get(2))).collect();
        assert_eq!(rows, vec![("2026-10-06 14".into(), 60, 30), ("2026-10-06 15".into(), 30, 0)]);
    }

    #[tokio::test]
    async fn crediting_sums_active_time_per_hour_kind_and_subject() {
        let pool = pool_with(&[40, 41]).await;
        let active = Tick { open: true, active: Some(Presence::Input) };
        let idle = Tick { open: true, active: None };
        let lecture = context("lecture", Some(3));

        credit(&pool, "2026-10-06 14", active, Some(&lecture)).await.unwrap();
        credit(&pool, "2026-10-06 14", active, Some(&lecture)).await.unwrap();
        credit(&pool, "2026-10-06 14", active, Some(&context("chat", None))).await.unwrap();
        credit(&pool, "2026-10-06 14", idle, None).await.unwrap();

        let hours: (i64, i64) = sqlx::query_as("SELECT open_seconds, active_seconds FROM usage_hours")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(hours, (120, 90));
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT kind, subject_id, active_seconds FROM usage_context_hours ORDER BY kind",
        )
        .fetch_all(&pool).await.unwrap();
        assert_eq!(rows, vec![("chat".into(), 0, 30), ("lecture".into(), 3, 60)]);
    }

    #[tokio::test]
    async fn crediting_without_the_context_table_writes_nothing() {
        let pool = pool_with(&[40]).await;
        let active = Tick { open: true, active: Some(Presence::Media) };
        assert!(credit(&pool, "2026-10-06 14", active, Some(&context("lecture", Some(3)))).await.is_err());
        let hours: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_hours").fetch_one(&pool).await.unwrap();
        assert_eq!(hours, 0, "the hour's row rolls back with the context's");
    }

    #[tokio::test]
    async fn adding_before_the_table_exists_is_an_error_not_a_panic() {
        let pool = SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        assert!(add(&pool, "2026-10-06 14", 30, 0).await.is_err());
    }
}
