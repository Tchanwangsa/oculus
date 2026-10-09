use std::sync::atomic::Ordering;

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};

use super::classify::{classify, Presence, Tick};
use super::store::{add, credit, hour_key};
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
    assert_eq!(
        active(false, Some(NOW), None),
        None,
        "input in an unfocused window is not presence"
    );
    assert_eq!(
        active(false, None, Some(NOW - 60)),
        Some(Presence::Media),
        "playing media counts without focus"
    );
    assert_eq!(active(true, None, Some(NOW - 61)), None);
    assert_eq!(active(true, None, None), None);
}

#[test]
fn input_wins_over_media_and_media_covers_an_unfocused_window() {
    let active = |focused, input, media| classify(true, false, focused, NOW, input, media).active;
    assert_eq!(active(true, Some(NOW), Some(NOW)), Some(Presence::Input));
    assert_eq!(active(false, Some(NOW), Some(NOW)), Some(Presence::Media));
    assert_eq!(
        active(true, Some(NOW - 121), Some(NOW)),
        Some(Presence::Media)
    );
}

#[test]
fn nothing_is_active_while_closed() {
    assert_eq!(
        classify(false, false, true, NOW, Some(NOW), Some(NOW)),
        Tick {
            open: false,
            active: None
        }
    );
    assert_eq!(
        classify(true, true, false, NOW, None, Some(NOW)),
        Tick {
            open: false,
            active: None
        }
    );
}

fn context(kind: &str, subject_id: Option<i64>) -> UsageContext {
    UsageContext {
        kind: kind.into(),
        subject_id,
    }
}

#[test]
fn each_ping_kind_keeps_its_own_latest_context() {
    let state = UsageState::default();
    assert_eq!(
        state.credited(Presence::Input),
        context("other", None),
        "nothing reported yet"
    );

    state
        .ping("input", Some(context("file", Some(7))), NOW)
        .unwrap();
    state
        .ping("media", Some(context("lecture", Some(3))), NOW)
        .unwrap();
    state.ping("input", None, NOW + 30).unwrap();
    assert_eq!(
        state.credited(Presence::Input),
        context("file", Some(7)),
        "a bare ping keeps the context"
    );
    assert_eq!(state.credited(Presence::Media), context("lecture", Some(3)));
    assert_eq!(state.last_input.load(Ordering::Relaxed), NOW + 30);
}

#[test]
fn unknown_kinds_are_refused_without_recording_the_ping() {
    let state = UsageState::default();
    assert!(state.ping("scroll", None, NOW).is_err());
    assert!(state
        .ping("input", Some(context("game", None)), NOW)
        .is_err());
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
        chrono::NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, 59)
            .unwrap()
    };
    assert_eq!(hour_key(at(2026, 3, 4, 5, 59)), "2026-03-04 05");
    assert_eq!(hour_key(at(2026, 12, 31, 23, 0)), "2026-12-31 23");
}

async fn pool_with(versions: &[i64]) -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    for migration in crate::db::migrations::all()
        .into_iter()
        .filter(|m| versions.contains(&m.version))
    {
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

    let rows =
        sqlx::query("SELECT hour, open_seconds, active_seconds FROM usage_hours ORDER BY hour")
            .fetch_all(&pool)
            .await
            .unwrap();
    let rows: Vec<(String, i64, i64)> = rows
        .iter()
        .map(|r| (r.get(0), r.get(1), r.get(2)))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("2026-10-06 14".into(), 60, 30),
            ("2026-10-06 15".into(), 30, 0)
        ]
    );
}

#[tokio::test]
async fn crediting_sums_active_time_per_hour_kind_and_subject() {
    let pool = pool_with(&[40, 41]).await;
    let active = Tick {
        open: true,
        active: Some(Presence::Input),
    };
    let idle = Tick {
        open: true,
        active: None,
    };
    let lecture = context("lecture", Some(3));

    credit(&pool, "2026-10-06 14", active, Some(&lecture))
        .await
        .unwrap();
    credit(&pool, "2026-10-06 14", active, Some(&lecture))
        .await
        .unwrap();
    credit(&pool, "2026-10-06 14", active, Some(&context("chat", None)))
        .await
        .unwrap();
    credit(&pool, "2026-10-06 14", idle, None).await.unwrap();

    let hours: (i64, i64) = sqlx::query_as("SELECT open_seconds, active_seconds FROM usage_hours")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(hours, (120, 90));
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT kind, subject_id, active_seconds FROM usage_context_hours ORDER BY kind",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![("chat".into(), 0, 30), ("lecture".into(), 3, 60)]
    );
}

#[tokio::test]
async fn crediting_without_the_context_table_writes_nothing() {
    let pool = pool_with(&[40]).await;
    let active = Tick {
        open: true,
        active: Some(Presence::Media),
    };
    assert!(credit(
        &pool,
        "2026-10-06 14",
        active,
        Some(&context("lecture", Some(3)))
    )
    .await
    .is_err());
    let hours: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_hours")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(hours, 0, "the hour's row rolls back with the context's");
}

#[tokio::test]
async fn adding_before_the_table_exists_is_an_error_not_a_panic() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    assert!(add(&pool, "2026-10-06 14", 30, 0).await.is_err());
}
