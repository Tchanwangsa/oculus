//! The start step: a run that is already written goes out to its provider.
//! The thread row decides model, subject and lecture, its link decides the
//! Studio video, then the send runs on a blocking thread.

use sqlx::SqlitePool;

use crate::harness::antigravity_rules;
use crate::harness::core::effect::{Runtime, Start};
use crate::harness::store;
use crate::harness::{HarnessEvent, LectureBrief, Provider, Release, SendOptions, StudioBrief};
use crate::runtime::blocking::run as blocking;

use super::proposal_context::{self, Cards};
use super::{sink_for, sinks_for};

/// Send a run the consumer has written. A failure still closes the turn (an
/// error row, then `TurnFinished`), which settles the run as failed, so
/// nothing waiting behind it is stranded.
pub(in crate::harness) async fn start(rt: &Runtime, start: Start) {
    let (thread_id, provider) = (start.thread_id, start.provider);
    if let Err(e) = send(rt, start).await {
        elog!("[oculus] harness send: {e}");
        let sink = sink_for(&rt.bus, thread_id, provider);
        sink(HarnessEvent::error(e));
        sink(HarnessEvent::TurnFinished {
            status: "failed".into(),
            turn: None,
        });
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

async fn send(rt: &Runtime, start: Start) -> Result<(), String> {
    let Start {
        thread_id,
        provider,
        run,
        text,
        options: opts,
        respawn,
    } = start;
    let pool = rt.pools.open().await?;
    let row = store::thread(&pool, thread_id).await?;
    // Claude and agy fix `--model` at spawn. The run is not bound to a turn
    // yet, so replacing the session interrupts nothing.
    if respawn && matches!(provider, Provider::Claude | Provider::Antigravity) {
        rt.harness.release(thread_id, Release::Replaced);
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
            .or_else(|| rt.harness.live_effort(thread_id)),
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
    let (text, cards) = prompt(
        &pool,
        thread_id,
        &text,
        opts.continuation.as_deref(),
        opts.context.as_deref(),
    )
    .await?;
    let (h, sinks) = (rt.harness.clone(), sinks_for(&rt.bus, thread_id, provider));
    blocking(move || h.send(thread_id, provider, resume.as_deref(), &opts, &text, &sinks)).await?;
    if let Err(e) = store::mark_sent(&pool, &run).await {
        elog!("[oculus] harness run {}: {e}", run.run_id);
    }
    proposal_context::accepted(&pool, cards).await
}

/// What the provider is sent for a run, and the one place its text is
/// finalised. The row keeps only what the student typed; what they had open
/// (a lecture's moment, a notebook) rides the prompt, or the timeline would
/// show it. A continuation's row is its line ("Sync finished — continuing");
/// its prompt is the card's outcome and the agent's own follow-up. "Cards in
/// this thread" comes last, read now (after any click made while it waited).
pub(in crate::harness) async fn prompt(
    pool: &SqlitePool,
    thread_id: i64,
    typed: &str,
    card: Option<&str>,
    context: Option<&str>,
) -> Result<(String, Cards), String> {
    let text = match (card, context) {
        (Some(card), _) => super::continuation::prompt(pool, card).await?,
        (None, Some(c)) => {
            format!("{typed}\n\n---\n\n## What the student had open when this was sent\n\n{c}")
        }
        (None, None) => typed.to_string(),
    };
    let cards = proposal_context::read(pool, thread_id).await?;
    let text = format!("{text}{}", cards.section);
    Ok((text, cards))
}
