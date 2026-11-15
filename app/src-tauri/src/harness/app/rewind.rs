//! The provider's half of a rewind: its own session forgets the question.

use crate::harness::core::decide::RewindAsk;
use crate::harness::core::effect::Runtime;
use crate::harness::store;
use sqlx::SqlitePool;

use crate::harness::{RewindTo, SendOptions};
use crate::runtime::blocking::run as blocking;

use super::send::{lecture_brief, studio_brief};
use super::sinks_for;

/// Ask the provider to forget a question and everything after it, and
/// answer whether it did. A refusal or an unresumable session answers
/// `false`, which the timeline shows as a note.
pub(in crate::harness) async fn provider(rt: &Runtime, ask: RewindAsk) -> bool {
    let RewindAsk {
        thread_id,
        provider,
        anchor,
        last_seen,
        resume,
        options,
    } = ask;
    let Ok(pool) = rt.pools.open().await else {
        return false;
    };
    let Ok(opts) = spawn_options(&pool, thread_id, options).await else {
        return false;
    };
    let (h, sinks) = (rt.harness.clone(), sinks_for(&rt.bus, thread_id, provider));
    blocking(move || {
        h.rewind(
            thread_id,
            provider,
            Some(&resume),
            &opts,
            RewindTo {
                anchor: &anchor,
                last_seen: last_seen.as_deref(),
            },
            &sinks,
        )
    })
    .await
    .is_ok()
}

/// What a respawn binds, since a rewind may respawn the session: the brief,
/// and the chat's saved mode, so the session is not respawned again for it
/// at the next send.
async fn spawn_options(
    pool: &SqlitePool,
    thread_id: i64,
    options: SendOptions,
) -> Result<SendOptions, String> {
    let row = store::thread(pool, thread_id).await?;
    Ok(SendOptions {
        scope: row.subject_code.clone(),
        lecture: lecture_brief(pool, &row).await,
        studio_brief: studio_brief(pool, thread_id).await,
        permission_mode: crate::harness::requests::store::mode(pool, thread_id).await?,
        ..options
    })
}

#[cfg(test)]
mod tests {
    use crate::harness::requests::PermissionMode;
    use crate::harness::{store, Provider, SendOptions};

    #[tokio::test]
    async fn a_respawn_after_a_rewind_takes_the_chats_saved_mode() {
        let dir = crate::test_support::Scratch::new("rewind-mode");
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(dir.join("oculus.db"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        for migration in crate::db::migrations::all() {
            sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
        }
        let thread = store::create_thread(&pool, Provider::Codex, None, None, None, "q")
            .await
            .unwrap();
        sqlx::query("UPDATE harness_threads SET permission_mode = 'quiet' WHERE id = ?1")
            .bind(thread)
            .execute(&pool)
            .await
            .unwrap();
        let options = SendOptions {
            permission_mode: PermissionMode::Ask,
            ..SendOptions::default()
        };
        let opts = super::spawn_options(&pool, thread, options).await.unwrap();
        assert_eq!(opts.permission_mode, PermissionMode::Quiet);
    }
}
