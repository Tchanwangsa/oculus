//! The driver's start step: a run that is already written goes out to its
//! provider.
//! The thread row decides model, subject and lecture, its link decides the
//! Studio video, then the send runs on a blocking thread.

use sqlx::SqlitePool;

use crate::harness::antigravity_rules;
use crate::harness::core::effect::{Runtime, Start};
use crate::harness::{handoff, store};
use crate::harness::{LectureBrief, Provider, SendError, SendOptions, StudioBrief};
use crate::runtime::blocking::run as blocking;

use super::proposal_context::{self, Cards};
use super::{sinks_for, BusMsg};

/// Send a run the consumer has written as `starting`, and tell the
/// consumer how it went (`BusMsg::Sent`): accepted by the provider,
/// withdrawn because the run was stopped or is gone before the provider
/// heard of it, or failed, which the consumer commits as the run's failure
/// so nothing waiting behind it is stranded.
pub(in crate::harness) async fn start(rt: &Runtime, start: Start) {
    let (thread_id, provider, run_id) = (start.thread_id, start.provider, start.run.run_id);
    let outcome = match send(rt, start).await {
        Ok(true) => Sent::Accepted,
        Ok(false) => Sent::Withdrawn,
        Err(Failure::Unsent(e)) => {
            elog!("[oculus] harness send: {e}");
            Sent::Failed(e)
        }
        Err(Failure::Unknown(e)) => {
            elog!("[oculus] harness send: {e}");
            Sent::Unknown(e)
        }
    };
    let _ = rt.bus.send(BusMsg::Sent {
        thread_id,
        provider,
        run_id,
        outcome,
    });
}

/// How a run's start step ended.
#[derive(Debug)]
pub(in crate::harness) enum Sent {
    /// The provider has the message; its turn will report.
    Accepted,
    /// The run was no longer `starting`, or was asked to stop: the provider
    /// was not called.
    Withdrawn,
    /// The send failed before the provider could hear of it.
    Failed(String),
    /// The write to the provider failed, so whether it has the message is
    /// not known. The run fails the same way; a handoff it carried stays
    /// unconfirmed (`handoff/`).
    Unknown(String),
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

/// Why a start step did not hand its run to the provider.
enum Failure {
    /// The provider heard nothing of it.
    Unsent(String),
    /// The write to the provider failed: it may or may not have the message.
    Unknown(String),
}

impl From<String> for Failure {
    fn from(error: String) -> Self {
        Failure::Unsent(error)
    }
}

/// False when the run was not this start's to send any more.
async fn send(rt: &Runtime, start: Start) -> Result<bool, Failure> {
    let Start {
        thread_id,
        provider,
        run,
        text,
        options: opts,
        model,
        effort,
    } = start;
    let pool = rt.pools.open().await?;
    let row = store::thread(&pool, thread_id).await?;
    // The row, not the payload, decides subject, lecture and video; the
    // run, not the row, decides provider, model and reasoning level, since
    // the chat's selection may have moved on. What that does to a live
    // session is `Harness::ensure`'s call (`manager/transition.rs`).
    let lecture = lecture_brief(&pool, &row).await;
    let studio_brief = studio_brief(&pool, thread_id).await;
    // Only a spawn uses these, but whether this send spawns is `ensure`'s call.
    let antigravity_rules = match provider {
        Provider::Antigravity => Some(antigravity_rules::stored(&pool).await?),
        _ => None,
    };
    let permission_mode = crate::harness::requests::store::mode(&pool, thread_id).await?;
    let opts = SendOptions {
        model,
        reasoning_effort: effort,
        permission_mode,
        scope: row.subject_code,
        lecture,
        studio_brief,
        antigravity_rules,
        ..opts
    };
    // The session to resume is the run's binding's, the provider's current
    // one in this chat when the run was written.
    let resume = store::bindings::of_run(&pool, run.run_id)
        .await?
        .and_then(|binding| binding.native_session);
    let (text, cards) = prompt(
        &pool,
        thread_id,
        &text,
        opts.continuation.as_deref(),
        opts.context.as_deref(),
    )
    .await?;
    let text = super::skills::name_picked_skills(provider, &opts.skills, text);
    // The earlier conversation this session has not seen goes first, in
    // its marked block. One that cannot be sent fails the run here, before
    // the provider hears anything.
    let text = match handoff::store::outgoing(&pool, run.run_id).await? {
        Some(block) => handoff::history::with_block(&block, &text),
        None => text,
    };
    // The last check before the provider hears of it: a run stopped while
    // this step prepared, or one a rewind or a delete took, is not sent.
    if !store::claim_start(&pool, &run).await? {
        return Ok(false);
    }
    let (h, sinks) = (rt.harness.clone(), sinks_for(&rt.bus, thread_id, provider));
    let sent =
        blocking(move || Ok(h.send(thread_id, provider, resume.as_deref(), &opts, &text, &sinks)));
    match sent.await? {
        Ok(()) => {}
        Err(SendError::Unsent(e)) => return Err(Failure::Unsent(e)),
        Err(SendError::Unknown(e)) => return Err(Failure::Unknown(e)),
    }
    if let Err(e) = store::mark_sent(&pool, &run).await {
        elog!("[oculus] harness run {}: {e}", run.run_id);
    }
    proposal_context::accepted(&pool, cards).await?;
    Ok(true)
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
