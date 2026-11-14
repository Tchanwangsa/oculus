//! Starting a turn: the thread row decides model, subject and lecture, its
//! link decides the Studio video, then the send runs on a blocking thread.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::harness::antigravity_rules;
use crate::harness::store;
use crate::harness::{Harness, HarnessEvent, LectureBrief, Provider, SendOptions, StudioBrief};
use crate::runtime::blocking::run as blocking;

use super::{sink_for, sinks_for, Bus, BusMsg};

/// A send the queue has let go. A failure still closes the turn (an error
/// row, then `TurnFinished`), so nothing queued behind it is stranded. A
/// card's continuation whose claim is gone (a rewind took the card, Stop
/// cancelled it) sends nothing and shows nothing; the thread passes on.
pub(super) async fn dispatch(
    harness: Arc<Harness>,
    bus: Bus,
    thread_id: i64,
    provider: Provider,
    opts: SendOptions,
    text: String,
) -> Result<(), String> {
    let sink = sink_for(&bus, thread_id, provider);
    let continuation = opts.continuation.is_some();
    match start_turn(&harness, &bus, thread_id, provider, opts, &text).await {
        Ok(()) => Ok(()),
        Err(e) if continuation && e == store::NOT_CLAIMED => {
            let _ = bus.send(BusMsg::Release(thread_id, provider));
            Ok(())
        }
        Err(e) => {
            sink(HarnessEvent::error(e.clone()));
            sink(HarnessEvent::TurnFinished {
                status: "failed".into(),
                turn: None,
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

/// The Studio section of a thread's brief, read off the video's link and
/// folder on every send. `None` for a thread that makes no video.
pub(super) async fn studio_brief(pool: &SqlitePool, thread_id: i64) -> Option<StudioBrief> {
    let link = crate::db::store::studio_link_of_thread(pool, thread_id)
        .await
        .ok()
        .flatten()?;
    Some(StudioBrief::read(
        &crate::studio::videos::videos_dir(),
        &link,
    ))
}

async fn start_turn(
    harness: &Arc<Harness>,
    bus: &Bus,
    thread_id: i64,
    provider: Provider,
    opts: SendOptions,
    text: &str,
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
    // The row, not the payload, decides model, subject, lecture and video.
    let lecture = lecture_brief(&pool, &row).await;
    let studio_brief = studio_brief(&pool, thread_id).await;
    // Only a spawn uses these, but whether this send spawns is `ensure`'s call.
    let antigravity_rules = match provider {
        Provider::Antigravity => Some(antigravity_rules::stored(&pool).await?),
        _ => None,
    };
    // A continuation has no composer behind it: it keeps the live session's
    // reasoning level, and the row's model, as the student last chose them.
    let reasoning_effort = match &opts.continuation {
        Some(_) => opts
            .reasoning_effort
            .clone()
            .or_else(|| harness.live_effort(thread_id)),
        None => opts.reasoning_effort.clone(),
    };
    let opts = SendOptions {
        model: opts.model.or(row.model),
        reasoning_effort,
        scope: row.subject_code,
        lecture,
        studio_brief,
        antigravity_rules,
        ..opts
    };
    let resume = row.provider_session_id;
    // The row is what the student typed; what they had open (a lecture's moment,
    // a notebook) rides only the prompt, or the timeline would show it. A
    // continuation's row is its line ("Sync finished — continuing"); the
    // prompt is the card's outcome and the agent's own follow-up.
    let intent = store::SendIntent {
        thread_id,
        provider,
        model: opts.model.clone(),
        native_session: resume.clone(),
        receipt: opts.receipt.clone().unwrap_or_else(store::fresh_receipt),
        text: text.to_string(),
        at: opts.at,
        continuation: opts.continuation.clone(),
    };
    let (run, text, cards) = begin_turn(&pool, bus, intent, opts.context.as_deref()).await?;
    let (h, sinks) = (harness.clone(), sinks_for(bus, thread_id, provider));
    blocking(move || h.send(thread_id, provider, resume.as_deref(), &opts, &text, &sinks)).await?;
    if let Err(e) = store::mark_sent(&pool, &run).await {
        elog!("[oculus] harness run {}: {e}", run.run_id);
    }
    super::proposal_context::accepted(&pool, cards).await
}

/// Write the send's run, then build what the provider is sent: the
/// student's text and what they had open, or a continuation's prompt, then
/// "Cards in this thread", read now (after any click made while it queued).
pub(in crate::harness) async fn begin_turn(
    pool: &SqlitePool,
    bus: &Bus,
    intent: store::SendIntent,
    context: Option<&str>,
) -> Result<(store::RunStart, String, super::proposal_context::Cards), String> {
    let (thread_id, card, typed) = (
        intent.thread_id,
        intent.continuation.clone(),
        intent.text.clone(),
    );
    let run = begin_run(bus, intent).await?;
    let text = match (card, context) {
        (Some(card), _) => super::continuation::prompt(pool, &card).await?,
        (None, Some(c)) => {
            format!("{typed}\n\n---\n\n## What the student had open when this was sent\n\n{c}")
        }
        (None, None) => typed,
    };
    let cards = super::proposal_context::read(pool, thread_id).await?;
    let text = format!("{text}{}", cards.section);
    Ok((run, text, cards))
}

/// Persist the send's run and question on the consumer, before any provider
/// write. An `Err` wrote nothing, and the caller must not send.
pub(in crate::harness) async fn begin_run(
    bus: &Bus,
    intent: store::SendIntent,
) -> Result<store::RunStart, String> {
    let (reply, answer) = tokio::sync::oneshot::channel();
    bus.send(BusMsg::Begin { intent, reply })
        .map_err(|_| "the harness consumer has stopped".to_string())?;
    answer
        .await
        .map_err(|_| "the harness consumer has stopped".to_string())?
}
