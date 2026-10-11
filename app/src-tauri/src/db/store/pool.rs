use std::path::{Path, PathBuf};
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// Our own pool over the plugin's file. WAL makes a second reader harmless and
/// our writes are rare, so a busy timeout keeps out of the plugin's way.
pub async fn pool(path: &Path) -> Result<SqlitePool, String> {
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .busy_timeout(Duration::from_secs(15));
    SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .map_err(|e| format!("open {}: {e}", path.display()))
}

/// The shared database file.
pub fn db_path() -> PathBuf {
    crate::library::paths::db_path(&crate::library::paths::data_dir())
}

/// Open the shared database, reporting (never creating) a missing one.
pub async fn open_pool() -> Result<SqlitePool, String> {
    let path = db_path();
    if !path.exists() {
        return Err(format!("no database at {}", path.display()));
    }
    pool(&path).await
}

/// Read a settings row through the caller's pool, connection or transaction.
pub async fn setting<'e, E>(executor: E, key: &str) -> Result<Option<String>, String>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_scalar("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(executor)
        .await
        .map_err(|e| e.to_string())
}

/// Upsert one settings row; callers own its format and validation.
pub async fn set_setting<'e, E>(executor: E, key: &str, value: &str) -> Result<(), String>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(executor)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Edit an object settings row, retaining unknown keys. Missing or malformed
/// objects use the same empty default as the parse and embed config readers.
pub async fn edit_setting(
    pool: &SqlitePool,
    key: &str,
    edit: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>),
) -> Result<(), String> {
    let stored = setting(pool, key).await?;
    let mut object = stored
        .as_deref()
        .and_then(|raw| {
            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(raw).ok()
        })
        .unwrap_or_default();
    edit(&mut object);
    let value = serde_json::to_string(&object).map_err(|e| e.to_string())?;
    set_setting(pool, key, &value).await
}

/// Read one `settings` row from a **synchronous** caller, from any context.
///
/// The seams read their backend setting from plain threads and non-async
/// clients. Never use `tauri::async_runtime::block_on` here: it panics on a
/// runtime worker thread, which is where an `async` Tauri command runs. So the
/// read always happens on a thread of its own with a current-thread runtime —
/// one code path, whoever calls. `None` covers no database, no row, or a
/// failed read; every caller defaults the same way.
pub fn setting_blocking(key: &str) -> Option<String> {
    let database = db_path();
    if !database.is_file() {
        return None;
    }
    let key = key.to_string();

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .ok()?;
        runtime.block_on(async move {
            use sqlx::Connection;
            let options = SqliteConnectOptions::new()
                .filename(&database)
                .create_if_missing(false)
                .busy_timeout(Duration::from_secs(15));
            let mut connection = sqlx::SqliteConnection::connect_with(&options).await.ok()?;
            let value = setting(&mut connection, &key).await.ok().flatten();
            connection.close().await.ok();
            value
        })
    })
    .join()
    .ok()
    .flatten()
}

pub async fn open(data_dir: &Path) -> Result<SqlitePool, String> {
    let path = crate::library::paths::db_path(data_dir);
    if !path.exists() {
        return Err(format!(
            "no database at {} — open the Oculus app once to create it",
            path.display()
        ));
    }
    pool(&path).await
}
