//! Steers on the consumer: a message committed into the running run is
//! told and handed to its effect; what the provider said of it is saved and
//! told; a steer that missed its turn waits first in the queue as its own
//! message; and a restart steer turns its run's terminal into a new attempt
//! instead of a settlement. docs/harness.md, "Steering the running turn".

use crate::harness::core::commit::Steered as Committed;
use crate::harness::core::effect::{Effect, SteerWrite};
use crate::harness::store::steer::{self, delivery, Restart, Written};
use crate::harness::{store, HarnessEvent, Provider, Steered};

use super::super::run_scope::{From, Tags};
use super::{Consumer, ConsumerIo, Stored};

impl Consumer {
    /// A steer was committed: its row is told, then its provider is told.
    /// A restart steer asks for the turn to stop instead; the run carries
    /// on as a new attempt once the turn's terminal comes.
    pub(super) fn steered(
        &mut self,
        thread_id: i64,
        provider: Provider,
        steered: Committed,
        io: &mut impl ConsumerIo,
    ) {
        let Committed {
            written,
            run_id,
            text,
            client,
            restart,
        } = steered;
        let delivery = match restart {
            true => delivery::RESTART,
            false => delivery::SENDING,
        };
        let told = HarnessEvent::Steer {
            text: text.clone(),
            message: written.message,
            client: client.clone(),
            delivery: delivery.into(),
            item: Some(written.item_id),
        };
        self.event(thread_id, provider, From::App, told, None, io);
        if restart {
            // Asked once per run: a second correction joins the first.
            if self.restarting.insert(run_id) {
                io.perform(Effect::Interrupt { thread_id });
            }
            return;
        }
        io.perform(Effect::Steer(Box::new(SteerWrite {
            thread_id,
            provider,
            written,
            text,
            client,
        })));
    }

    /// What a steer's write came to. Taken in: the row says so. Missed,
    /// declined or withdrawn: nothing reached the turn, so the message
    /// waits first in the queue as its own run, and the driver starts it
    /// when nothing is open. A write that broke partway is never resent.
    pub(super) fn steer_reported(
        &mut self,
        thread_id: i64,
        provider: Provider,
        written: Written,
        outcome: Steered,
        io: &mut impl ConsumerIo,
    ) {
        let Some(pool) = self.pool.clone() else {
            return;
        };
        let state = match outcome {
            Steered::Accepted => {
                // The claim wrote `written` with its own revision; an echo
                // read before this report has already moved it on.
                if let Ok(revision) = self.rt.block_on(store::revision(&pool, thread_id)) {
                    self.revisions.insert(thread_id, revision);
                }
                let now = self.rt.block_on(steer::delivery_of(&pool, written.item_id));
                if now.ok().flatten().as_deref() != Some(delivery::WRITTEN) {
                    return;
                }
                delivery::WRITTEN
            }
            Steered::Unknown(_) => {
                let saved = steer::update(&pool, thread_id, written.item_id, delivery::UNKNOWN);
                match self.rt.block_on(saved) {
                    Ok(Some((_, revision))) => {
                        self.revisions.insert(thread_id, revision);
                    }
                    Ok(None) => return,
                    Err(e) => {
                        elog!("[oculus] harness thread {thread_id}: steer: {e}");
                        return;
                    }
                }
                delivery::UNKNOWN
            }
            Steered::Missed | Steered::Declined(_) | Steered::Withdrawn => {
                self.flush_draft(thread_id, io.now());
                match self
                    .rt
                    .block_on(steer::missed(&pool, thread_id, written.item_id))
                {
                    Ok(Some((_, revision))) => {
                        self.revisions.insert(thread_id, revision);
                    }
                    Ok(None) => return,
                    Err(e) => {
                        elog!("[oculus] harness thread {thread_id}: steer: {e}");
                        return;
                    }
                }
                delivery::REQUEUED
            }
        };
        self.tell_steer(thread_id, provider, written, state, io);
        if state == delivery::REQUEUED {
            self.tell_queue(thread_id, provider, io);
            self.drive(thread_id, provider, io);
        }
    }

    /// The provider echoed a steer: it has the message.
    pub(super) fn steer_echoed(
        &mut self,
        thread_id: i64,
        provider: Provider,
        client: &str,
        io: &mut impl ConsumerIo,
    ) {
        let Some(pool) = self.pool.clone() else {
            return;
        };
        let Ok(Some(found)) = self.rt.block_on(steer::by_client(&pool, thread_id, client)) else {
            return;
        };
        self.flush_draft(thread_id, io.now());
        let saved = steer::update(&pool, thread_id, found.item_id, delivery::DELIVERED);
        match self.rt.block_on(saved) {
            Ok(Some((written, revision))) => {
                self.revisions.insert(thread_id, revision);
                self.tell_steer(thread_id, provider, written, delivery::DELIVERED, io);
            }
            Ok(None) => {}
            Err(e) => elog!("[oculus] harness thread {thread_id}: steer echo: {e}"),
        }
    }

    fn tell_steer(
        &mut self,
        thread_id: i64,
        provider: Provider,
        written: Written,
        state: &str,
        io: &mut impl ConsumerIo,
    ) {
        let updated = HarnessEvent::SteerUpdated {
            item_id: written.item_id,
            message: written.message,
            delivery: state.into(),
        };
        self.event(thread_id, provider, From::App, updated, None, io);
    }

    /// The open run's turn ended with restart steers waiting on it: what it
    /// streamed is kept as partial, its attempt is superseded, and a new
    /// one carries the run on with the corrections. The run stays open and
    /// its claim on the queue stays held. False when no restart waited,
    /// and the terminal settles the run as usual.
    pub(super) fn restart(
        &mut self,
        thread_id: i64,
        provider: Provider,
        (run_id, attempt_id): (i64, i64),
        status: &str,
        io: &mut impl ConsumerIo,
    ) -> bool {
        let Some(pool) = self.pool.clone() else {
            return false;
        };
        // Saved state decides: a restart steer's row waits on this run.
        let waiting = self.rt.block_on(async {
            let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;
            steer::restarts(&mut conn, run_id).await
        });
        if waiting.map_or(true, |rows| rows.is_empty()) {
            return false;
        }
        // What streamed under the old attempt is its partial reply.
        for event in self.cut_short(run_id, "interrupted") {
            self.event(thread_id, provider, From::App, event, None, io);
        }
        let restarted = steer::restart(&pool, thread_id, run_id, attempt_id, status);
        self.restarting.remove(&run_id);
        let (attempt, revision, work, delivered) = match self.rt.block_on(restarted) {
            Ok(Restart::Restarted {
                attempt,
                revision,
                work,
                delivered,
            }) => (attempt, revision, work, delivered),
            // Stopped meanwhile: the corrections wait, held, and the run
            // settles on this terminal.
            Ok(Restart::Requeued { revision }) => {
                self.revisions.insert(thread_id, revision);
                self.tell_queue(thread_id, provider, io);
                return false;
            }
            Ok(Restart::None) => return false,
            Err(e) => {
                elog!("[oculus] harness run {run_id}: restart: {e}");
                return false;
            }
        };
        self.revisions.insert(thread_id, revision);
        self.runs.restart(thread_id, attempt);
        self.started.remove(&run_id);
        self.accepted.remove(&run_id);
        // Requests the old attempt still waited on end with it, and its
        // corrections went out with the new one.
        let mut ended = self.requests_left_by(thread_id, Some(run_id), "interrupted");
        for item_id in delivered {
            let message = self.rt.block_on(steer::message_of(&pool, item_id));
            if let Ok(Some(message)) = message {
                let updated = HarnessEvent::SteerUpdated {
                    item_id,
                    message,
                    delivery: delivery::DELIVERED.into(),
                };
                ended.push((updated, None));
            }
        }
        let tags = Tags {
            run_id: Some(run_id),
            attempt_id: Some(attempt),
        };
        let told = HarnessEvent::AttemptRestarted {
            run_id,
            attempt_id: attempt,
            superseded: attempt_id,
            work_item_ids: work,
        };
        let stored = Stored {
            thread_id,
            provider,
            from: From::App,
            ev: told,
            card: None,
            tags,
            item_id: None,
            entity_id: None,
            terminal: None,
        };
        self.publish(stored, ended, io);
        true
    }
}
