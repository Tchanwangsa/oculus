//! Starting a turn: the thread row decides model, subject and lecture, then
//! the send runs on a blocking thread.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::harness::antigravity_rules;
use crate::harness::store;
use crate::harness::{Harness, HarnessEvent, LectureBrief, Provider, SendOptions, Sink};
use crate::runtime::blocking::run as blocking;

use super::{sink_for, Bus};

/// A send the queue has let go. A failure still closes the turn (an error
/// row, then `TurnFinished`), so nothing queued behind it is stranded.
pub(super) async fn dispatch(
    harness: Arc<Harness>,
    bus: Bus,
    thread_id: i64,
    provider: Provider,
    opts: SendOptions,
    text: String,
) -> Result<(), String> {
    let sink = sink_for(&bus, thread_id, provider);
    match start_turn(&harness, thread_id, provider, opts, &text, sink.clone()).await {
        Ok(()) => Ok(()),
        Err(e) => {
            sink(HarnessEvent::error(e.clone()));
            sink(HarnessEvent::TurnFinished {
                status: "failed".into(),
            });
            Err(e)
        }
    }
}

/// The lecture section of a thread's brief, read off its row on every
/// send (chapters can land after the conversation started). Needed
/// before *any* spawn, a rewind's included: the brief binds at start.
pub(super) async fn lecture_brief(
    pool: &SqlitePool,
    row: &store::ThreadRow,
) -> Option<LectureBrief> {
    let l = row.lecture.as_ref()?;
    Some(LectureBrief {
        id: l.id.clone(),
        title: l.title.clone(),
        date: l.date.clone(),
        has_transcript: l.has_transcript,
        chapters: crate::db::store::chapters(pool, &l.id)
            .await
            .unwrap_or_default(),
    })
}

async fn start_turn(
    harness: &Arc<Harness>,
    thread_id: i64,
    provider: Provider,
    opts: SendOptions,
    text: &str,
    sink: Sink,
) -> Result<(), String> {
    let pool = crate::db::store::open_pool().await?;
    let row = store::thread(&pool, thread_id).await?;
    if row.provider != provider {
        return Err(format!(
            "thread {thread_id} is a {} thread",
            row.provider.label()
        ));
    }
    if opts.model.is_some() && opts.model != row.model {
        store::set_model(&pool, thread_id, opts.model.as_deref()).await?;
        // Claude and agy fix `--model` at spawn. The thread is between
        // turns here, so nothing is killed mid-answer.
        if matches!(provider, Provider::Claude | Provider::Antigravity) {
            harness.close(thread_id);
        }
    }
    // The row, not the payload, decides model, subject and lecture.
    let lecture = lecture_brief(&pool, &row).await;
    // Only a spawn uses these, but whether this send spawns is `ensure`'s call.
    let antigravity_rules = match provider {
        Provider::Antigravity => Some(antigravity_rules::stored(&pool).await?),
        _ => None,
    };
    let opts = SendOptions {
        model: opts.model.or(row.model),
        scope: row.subject_code,
        lecture,
        antigravity_rules,
        ..opts
    };
    let resume = row.provider_session_id;
    // The row is what the student typed; the moment's text rides only
    // the prompt, or the timeline would show a transcript excerpt.
    sink(HarnessEvent::UserMessage {
        text: text.to_string(),
        at: opts.at,
    });
    let text = match &opts.context {
        Some(c) => format!("{text}\n\n---\n\n## The moment this was sent at\n\n{c}"),
        None => text.to_string(),
    };

    let (h, sink) = (harness.clone(), sink.clone());
    blocking(move || h.send(thread_id, provider, resume.as_deref(), &opts, &text, sink)).await
}
