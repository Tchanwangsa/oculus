//! The schema the move writes into: `lectures.folder_rel` (63) and
//! `path_moves` (64). The app's migrations are applied here exactly as
//! `tauri-plugin-sql` applies them (same versions, descriptions and SQL, so
//! the same checksums), so the plugin finds nothing left to do when the
//! frontend loads the database.

use std::future::Future;
use std::pin::Pin;

use sqlx::error::BoxDynError;
use sqlx::migrate::{Migration, MigrationSource, MigrationType, Migrator};
use sqlx::SqlitePool;

#[derive(Debug)]
struct AppMigrations;

impl MigrationSource<'static> for AppMigrations {
    #[allow(clippy::type_complexity)]
    fn resolve(
        self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Migration>, BoxDynError>> + Send + 'static>> {
        Box::pin(async { Ok(app_migrations()) })
    }
}

/// The plugin's conversion: every `Up` migration as `ReversibleUp`, in a
/// transaction.
fn app_migrations() -> Vec<Migration> {
    crate::db::migrations::all()
        .into_iter()
        .filter(|m| matches!(m.kind, tauri_plugin_sql::MigrationKind::Up))
        .map(|m| {
            Migration::new(
                m.version,
                m.description.into(),
                MigrationType::ReversibleUp,
                m.sql.into(),
                false,
            )
        })
        .collect()
}

/// Apply every migration the database lacks. A database carrying one this
/// build does not know is refused, as the plugin would refuse it.
pub async fn ensure(pool: &SqlitePool) -> Result<(), String> {
    let migrator = Migrator::new(AppMigrations)
        .await
        .map_err(|e| format!("read the migrations: {e}"))?;
    migrator
        .run(pool)
        .await
        .map_err(|e| format!("bring the database's schema up to date: {e}"))
}

/// The migrations up to `version`, for a test's database from before them.
#[cfg(test)]
pub async fn ensure_up_to(pool: &SqlitePool, version: i64) {
    let migrations: Vec<Migration> = app_migrations()
        .into_iter()
        .filter(|m| m.version <= version)
        .collect();
    let migrator = Migrator {
        migrations: std::borrow::Cow::Owned(migrations),
        ..Migrator::DEFAULT
    };
    migrator.run(pool).await.unwrap();
}
