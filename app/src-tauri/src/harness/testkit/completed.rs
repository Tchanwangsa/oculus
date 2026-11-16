use super::commands::Bench;
use crate::harness::core::Kind;

#[test]
fn completing_and_reopening_use_receipts_without_provider_effects() {
    let mut bench = Bench::new("completion-receipts");
    bench.runtime.block_on(async {
        sqlx::query("INSERT INTO harness_bindings(id, thread_id, provider) VALUES (90, ?1, 'claude')")
            .bind(bench.thread).execute(&bench.pool).await.unwrap();
        sqlx::query("INSERT INTO harness_runs(id, thread_id, ordinal, origin, binding_id, execution_status, queue_position, queued_text, queued_options) VALUES (90, ?1, 1, 'student', 90, 'queued', 1, 'waiting', '{}')")
            .bind(bench.thread).execute(&bench.pool).await.unwrap();
    });
    let (complete, first) = bench.command("complete", Kind::CompleteThread);
    let (again, duplicate) = bench.command("complete", Kind::CompleteThread);
    bench.play(vec![complete, again]);
    assert_eq!(bench.got(first).unwrap().thread_id, bench.thread);
    assert_eq!(bench.got(duplicate).unwrap().thread_id, bench.thread);
    assert_eq!(
        bench.count("SELECT COUNT(*) FROM harness_threads WHERE completed_at IS NOT NULL"),
        1
    );
    assert_eq!(
        bench.count("SELECT COUNT(*) FROM harness_receipts WHERE command = 'complete_thread'"),
        1
    );
    assert!(bench.effects().is_empty());
    let (reopen, reopened) = bench.command("reopen", Kind::ReopenThread);
    bench.play(vec![reopen]);
    assert!(bench.got(reopened).is_ok());
    assert_eq!(
        bench.count("SELECT COUNT(*) FROM harness_threads WHERE completed_at IS NOT NULL"),
        0
    );
    assert!(bench.effects().is_empty());
    assert_eq!(
        bench.count("SELECT COUNT(*) FROM harness_runs WHERE execution_status = 'queued'"),
        1
    );
}
