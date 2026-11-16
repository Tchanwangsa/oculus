//! A steer's effect: a message already saved inside the running run goes
//! to its provider's turn, and the consumer is told what became of it
//! (`BusMsg::Steered`). The write is claimed first (`store::steer::claim`),
//! so a run stopped or ended meanwhile is not written to, and a restart
//! never sends the same steer twice.

use crate::harness::core::effect::{Runtime, SteerWrite};
use crate::harness::{store, Steered};
use crate::runtime::blocking::run as blocking;

use super::BusMsg;

pub(in crate::harness) async fn write(rt: &Runtime, steer: SteerWrite) {
    let (thread_id, provider, written) = (steer.thread_id, steer.provider, steer.written);
    let outcome = send(rt, steer).await;
    if let Steered::Declined(why) | Steered::Unknown(why) = &outcome {
        elog!("[oculus] harness thread {thread_id}: steer: {why}");
    }
    let _ = rt.bus.send(BusMsg::Steered {
        thread_id,
        provider,
        written,
        outcome,
    });
}

async fn send(rt: &Runtime, steer: SteerWrite) -> Steered {
    let pool = match rt.pools.open().await {
        Ok(pool) => pool,
        Err(e) => return Steered::Declined(e),
    };
    match store::steer::claim(&pool, steer.thread_id, steer.written.item_id).await {
        Ok(true) => {}
        Ok(false) => return Steered::Withdrawn,
        Err(e) => return Steered::Declined(e),
    }
    let harness = rt.harness.clone();
    let SteerWrite {
        thread_id,
        text,
        client,
        ..
    } = steer;
    let written = blocking(move || Ok(harness.steer(thread_id, &text, &client)));
    written.await.unwrap_or_else(Steered::Unknown)
}
