//! The provider's half of a rewind: its own session forgets the question.

use crate::harness::core::decide::RewindAsk;
use crate::harness::core::effect::Runtime;
use crate::harness::store;
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
    let Ok(row) = store::thread(&pool, thread_id).await else {
        return false;
    };
    // A rewind may respawn the session, which binds the brief.
    let opts = SendOptions {
        scope: row.subject_code.clone(),
        lecture: lecture_brief(&pool, &row).await,
        studio_brief: studio_brief(&pool, thread_id).await,
        ..options
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
