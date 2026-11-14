//! A card's follow-up: the agent's optional `then` on a proposal or job card,
//! run as one turn after the card settles (docs/harness.md, "A card can
//! continue the agent"). The student's click on "… and continue" is the
//! consent for that one billed turn, so nothing else starts one.
//!
//! The claim lives in the card's JSON state, `continuation`: `queued` once
//! [`claim`] wins it, `started` with the run's id once [`start`] writes the
//! run, or `skipped` with the reason it never ran. Each step is a conditional
//! update, so a card continues at most once, and never once its row is gone
//! (a rewind past it, or the thread deleted).

use serde_json::Value;
use sqlx::{Row, SqliteConnection, SqlitePool};

/// Startup: a job the app closed during never finished, so its follow-up never ran.
pub(in crate::harness) const SKIPPED_JOB: &str =
    "The app closed before this job finished, so the agent did not continue.";
/// Startup: the card settled, but the app closed before its turn could start.
pub(in crate::harness) const SKIPPED_QUEUED: &str =
    "The app closed before the agent could continue.";
/// Stop cleared the queue the continuation was waiting in.
pub(in crate::harness) const STOPPED: &str = "Stopped before the agent continued.";

/// The card's row is still in its thread's timeline.
const ROW_EXISTS: &str =
    "EXISTS (SELECT 1 FROM harness_items i JOIN harness_threads t ON t.id = i.thread_id
                 WHERE i.id = chat_proposals.item_id AND i.thread_id = chat_proposals.thread_id)";

/// The agent's follow-up on a card, when it wrote one.
pub fn follow_up(state: &Value) -> Option<&str> {
    state
        .get("then")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn key(card: &str) -> Result<(i64, i64), String> {
    crate::db::proposals::key(card)
}

/// Take the card's one continuation, if it has a follow-up and has settled:
/// applied, discarded, or its job finished (failed included). A job the
/// app closed during does not count. Returns the timeline line for its run.
pub(in crate::harness) async fn claim(
    pool: &SqlitePool,
    thread_id: i64,
    card: &str,
) -> Result<Option<String>, String> {
    let (item, block) = key(card)?;
    let claimed = sqlx::query(&format!(
        "UPDATE chat_proposals SET state = json_set(state, '$.continuation', json('{{\"phase\":\"queued\"}}'))
          WHERE item_id = ?1 AND block = ?2 AND thread_id = ?3
            AND json_type(state, '$.then') = 'text' AND trim(json_extract(state, '$.then')) != ''
            AND json_type(state, '$.continuation') IS NULL
            AND (status = 'discarded'
                 OR (status = 'created'
                     AND COALESCE(json_extract(state, '$.job.phase'), 'complete') IN ('complete', 'error')
                     AND json_type(state, '$.job.interrupted') IS NULL))
            AND {ROW_EXISTS}"
    ))
    .bind(item)
    .bind(block)
    .bind(thread_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?
    .rows_affected();
    if claimed == 0 {
        return Ok(None);
    }
    let row = sqlx::query(
        "SELECT kind, status, state FROM chat_proposals WHERE item_id = ?1 AND block = ?2",
    )
    .bind(item)
    .bind(block)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;
    let state: Value = serde_json::from_str(&row.get::<String, _>("state")).unwrap_or_default();
    Ok(Some(line(
        &row.get::<String, _>("kind"),
        &row.get::<String, _>("status"),
        &state,
    )))
}

/// The queued claim becomes the run: false when the card no longer holds
/// one (a rewind took it, Stop cancelled it), and the caller must not send.
pub(in crate::harness) async fn start(
    tx: &mut SqliteConnection,
    thread_id: i64,
    card: &str,
) -> Result<bool, String> {
    let (item, block) = key(card)?;
    let n = sqlx::query(&format!(
        "UPDATE chat_proposals SET state = json_set(state, '$.continuation.phase', 'started')
          WHERE item_id = ?1 AND block = ?2 AND thread_id = ?3
            AND json_extract(state, '$.continuation.phase') = 'queued' AND {ROW_EXISTS}"
    ))
    .bind(item)
    .bind(block)
    .bind(thread_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?
    .rows_affected();
    Ok(n == 1)
}

/// The started continuation names its run, for the card.
pub(in crate::harness) async fn started(
    tx: &mut SqliteConnection,
    card: &str,
    run_id: i64,
) -> Result<(), String> {
    let (item, block) = key(card)?;
    sqlx::query("UPDATE chat_proposals SET state = json_set(state, '$.continuation.run_id', ?3) WHERE item_id = ?1 AND block = ?2")
        .bind(item)
        .bind(block)
        .bind(run_id)
        .execute(&mut *tx)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// A queued continuation that will not run, and why. False when it was not queued.
pub(in crate::harness) async fn cancel(
    db: impl sqlx::SqliteExecutor<'_>,
    card: &str,
    reason: &str,
) -> Result<bool, String> {
    let (item, block) = key(card)?;
    let n = sqlx::query(
        "UPDATE chat_proposals SET state = json_set(state, '$.continuation', json_object('phase', 'skipped', 'reason', ?3))
          WHERE item_id = ?1 AND block = ?2 AND json_extract(state, '$.continuation.phase') = 'queued'",
    )
    .bind(item)
    .bind(block)
    .bind(reason)
    .execute(db)
    .await
    .map_err(|e| e.to_string())?
    .rows_affected();
    Ok(n == 1)
}

/// Startup, after `proposal_jobs::reconcile`: the click that consented to a
/// continuation was made in a session that is over, so one still waiting is
/// not started later (`store::reconcile` cancels its run), and a job the app
/// closed during never settled. Both cards say so.
pub(in crate::harness) async fn reconcile(pool: &SqlitePool) -> Result<(), String> {
    for (sql, reason) in [
        (
            "UPDATE chat_proposals SET state = json_set(state, '$.continuation', json_object('phase', 'skipped', 'reason', ?1))
              WHERE json_extract(state, '$.continuation.phase') = 'queued'",
            SKIPPED_QUEUED,
        ),
        (
            "UPDATE chat_proposals SET state = json_set(state, '$.continuation', json_object('phase', 'skipped', 'reason', ?1))
              WHERE json_type(state, '$.then') = 'text' AND json_type(state, '$.continuation') IS NULL
                AND json_type(state, '$.job.interrupted') = 'true'",
            SKIPPED_JOB,
        ),
    ] {
        sqlx::query(sql)
            .bind(reason)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// What the continuation's run tells the agent, above "Cards in this thread"
/// (which carries the card's outcome). Marked as Oculus's, not the student's.
pub(in crate::harness) async fn prompt(pool: &SqlitePool, card: &str) -> Result<String, String> {
    let proposal = crate::db::proposals::get(pool, card).await?;
    let then = follow_up(&proposal.state).unwrap_or_default();
    let outcome = match (
        proposal.status.as_str(),
        proposal.state.pointer("/job/phase").and_then(Value::as_str),
    ) {
        ("discarded", _) => "was discarded by the student",
        (_, Some("error")) => "failed",
        (_, Some(_)) => "finished",
        _ => "was applied by the student",
    };
    Ok(format!(
        "(From Oculus, not typed by the student.) Your {} card {card} {outcome}; its outcome is under Cards in this thread. When you proposed it, you asked to continue afterwards with:\n\n{then}",
        proposal.kind
    ))
}

/// The timeline's line for the run: what settled, then "continuing".
fn line(kind: &str, status: &str, state: &Value) -> String {
    let what = if status == "discarded" {
        "Card discarded".to_string()
    } else if let Some(phase) = state.pointer("/job/phase").and_then(Value::as_str) {
        let job = match kind {
            "Sync" => "Sync",
            "Reparse" => "Re-parse",
            "Transcribe" => "Transcription",
            "LectureChapters" => "Chapter search",
            "LectureEnd" => "Lecture end search",
            "Download" => "Download",
            _ => "Job",
        };
        format!(
            "{job} {}",
            if phase == "error" {
                "failed"
            } else {
                "finished"
            }
        )
    } else {
        match kind {
            "NewTasks" => "Tasks created",
            "NewProject" => "Project created",
            "NewEvents" => "Events created",
            "NewNote" => "Note created",
            "NewCheatNotes" => "Cheat sheet entries added",
            "DeleteTasks" => "Tasks deleted",
            "DeleteEvents" => "Events deleted",
            _ => "Changes applied",
        }
        .to_string()
    };
    format!("{what} — continuing")
}
