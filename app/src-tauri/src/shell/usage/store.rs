use sqlx::SqlitePool;

use super::classify::Tick;
use super::{UsageContext, TICK_SECS};

/// The `usage_hours` key for a local time: `YYYY-MM-DD HH`.
pub fn hour_key(local: chrono::NaiveDateTime) -> String {
    local.format("%Y-%m-%d %H").to_string()
}

/// Add seconds to an hour's row, creating it if needed.
pub async fn add<'e, E>(
    executor: E,
    hour: &str,
    open_seconds: i64,
    active_seconds: i64,
) -> Result<(), String>
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
    add(
        &mut *tx,
        hour,
        seconds(tick.open),
        seconds(tick.active.is_some()),
    )
    .await?;
    if let (Some(_), Some(context)) = (tick.active, context) {
        add_context(&mut *tx, hour, context, TICK_SECS as i64).await?;
    }
    tx.commit().await.map_err(|e| e.to_string())
}
