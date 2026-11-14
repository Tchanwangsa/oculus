//! Startup's repair of what the last process left open, finished before any
//! command is accepted: a send let in earlier would be cut by the repair
//! meant for the process before it.

use std::future::Future;

use sqlx::SqlitePool;

use crate::harness::store;

/// The gate every thread command waits at. The first caller runs the repair;
/// the rest wait for it.
#[derive(Default)]
pub(in crate::harness) struct Startup {
    repaired: tokio::sync::Mutex<bool>,
}

impl Startup {
    /// A gate with nothing to wait for: the database was made by this process.
    #[cfg(test)]
    pub(in crate::harness) fn repaired() -> Self {
        Startup {
            repaired: tokio::sync::Mutex::new(true),
        }
    }

    /// Returns once the repair has succeeded, running it first when it has
    /// not. A failure is logged and is the caller's error, so it reaches the
    /// student; the next caller tries again.
    pub(in crate::harness) async fn ready<F>(&self, pool: impl FnOnce() -> F) -> Result<(), String>
    where
        F: Future<Output = Result<SqlitePool, String>>,
    {
        let mut repaired = self.repaired.lock().await;
        if *repaired {
            return Ok(());
        }
        let outcome = match pool().await {
            Ok(pool) => repair(&pool).await,
            Err(e) => Err(e),
        };
        match outcome {
            Ok(()) => {
                *repaired = true;
                Ok(())
            }
            Err(e) => {
                elog!("[oculus] harness startup: {e}");
                Err(format!(
                    "Oculus could not finish checking the chats its last session left open, \
                     so nothing was started: {e}"
                ))
            }
        }
    }
}

/// Nothing survives a restart as running: proposal jobs, queued card
/// continuations, then threads, open tool rows and runs. Every step runs
/// even when an earlier one failed; the first failure is returned.
async fn repair(pool: &SqlitePool) -> Result<(), String> {
    let jobs = super::proposal_jobs::reconcile(pool).await;
    let continuations = super::continuation::reconcile(pool).await;
    let rows = store::reconcile(pool).await.map(|r| {
        if r.threads > 0 || r.tools > 0 {
            elog!(
                "[oculus] harness: marked {} interrupted thread(s) idle, closed {} open tool row(s)",
                r.threads,
                r.tools
            );
        }
    });
    jobs.map_err(|e| format!("proposal jobs: {e}"))?;
    continuations.map_err(|e| format!("card continuations: {e}"))?;
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    async fn scratch_pool(name: &str) -> (crate::test_support::Scratch, SqlitePool) {
        let dir = crate::test_support::Scratch::new(name);
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
        (dir, pool)
    }

    #[tokio::test]
    async fn the_repair_runs_once_and_a_command_waits_for_it() {
        let (_dir, pool) = scratch_pool("startup-gate").await;
        let (startup, opened) = (Startup::default(), AtomicU32::new(0));
        let (begin, begun) = tokio::sync::oneshot::channel::<()>();
        let (finish, finished) = tokio::sync::oneshot::channel::<()>();
        let slow = startup.ready(|| async {
            opened.fetch_add(1, Ordering::SeqCst);
            begin.send(()).unwrap();
            finished.await.unwrap();
            Ok(pool.clone())
        });
        let command = startup.ready(|| async {
            opened.fetch_add(1, Ordering::SeqCst);
            Ok(pool.clone())
        });
        tokio::pin!(slow, command);
        // The repair is under way and the command is held at the gate.
        tokio::select! {
            biased;
            _ = &mut slow => panic!("the repair cannot finish yet"),
            _ = begun => {}
        }
        tokio::select! {
            biased;
            _ = &mut command => panic!("a command got past a repair still running"),
            _ = std::future::ready(()) => {}
        }
        finish.send(()).unwrap();
        slow.await.unwrap();
        command.await.unwrap();
        assert_eq!(opened.load(Ordering::SeqCst), 1, "repaired once");
    }

    #[tokio::test]
    async fn a_failed_repair_is_the_callers_error_and_the_next_caller_tries_again() {
        let (_dir, pool) = scratch_pool("startup-failed").await;
        let startup = Startup::default();
        let refused = startup
            .ready(|| async { Err("database or disk is full".to_string()) })
            .await
            .unwrap_err();
        assert!(refused.contains("database or disk is full"), "{refused}");
        assert!(refused.contains("nothing was started"), "{refused}");

        let opened = AtomicU32::new(0);
        for _ in 0..2 {
            startup
                .ready(|| async {
                    opened.fetch_add(1, Ordering::SeqCst);
                    Ok(pool.clone())
                })
                .await
                .unwrap();
        }
        assert_eq!(opened.load(Ordering::SeqCst), 1, "retried, then done");
    }
}
