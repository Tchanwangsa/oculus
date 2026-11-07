//! One-off turns the app can stop by an id it chose: a chat widget's `ask`
//! has a Stop pill (`widget_ask_cancel`). Each id has a slot for as long as
//! its [`StopScope`] lives; stopping an id with no slot does nothing, so a
//! late Stop never leaks state.

use std::collections::HashMap;

use crate::harness::jobs::JobSelection;

use super::handle::Handle;
use super::Harness;

/// The running turn under each id, and whether it was stopped.
#[derive(Default)]
pub(in crate::harness) struct Stoppable {
    slots: HashMap<u64, Slot>,
}

#[derive(Default)]
struct Slot {
    handle: Option<Handle>,
    stopped: bool,
}

/// An open slot; dropping it frees the id.
pub struct StopScope<'a> {
    harness: &'a Harness,
    id: u64,
}

impl Harness {
    /// Opens `id`'s slot for the turns that follow. An id already open is an
    /// error rather than a second owner of its Stop.
    pub fn stop_scope(&self, id: u64) -> Result<StopScope<'_>, String> {
        let mut s = self.stoppable.lock().unwrap();
        if s.slots.contains_key(&id) {
            return Err(format!("request {id} is already running"));
        }
        s.slots.insert(id, Slot::default());
        Ok(StopScope { harness: self, id })
    }

    /// Stops `id`'s turn as fast as the provider allows (`Handle::cancel`)
    /// and keeps any later turn under it from starting. The handle stays in
    /// its slot, so a second Stop cancels again.
    pub fn stop_one_off(&self, id: u64) {
        let running = {
            let mut s = self.stoppable.lock().unwrap();
            let Some(slot) = s.slots.get_mut(&id) else {
                return;
            };
            slot.stopped = true;
            slot.handle.clone()
        };
        if let Some(h) = running {
            let _ = h.cancel();
        }
    }

    /// Every stoppable turn, on quit.
    pub(in crate::harness) fn stop_all_one_offs(&self) {
        let ids: Vec<u64> = self
            .stoppable
            .lock()
            .unwrap()
            .slots
            .keys()
            .copied()
            .collect();
        for id in ids {
            self.stop_one_off(id);
        }
    }
}

impl StopScope<'_> {
    pub fn stopped(&self) -> bool {
        self.harness
            .stoppable
            .lock()
            .unwrap()
            .slots
            .get(&self.id)
            .is_none_or(|s| s.stopped)
    }

    /// [`Harness::one_turn`] that this scope's Stop can end: None once
    /// stopped, before or during the turn.
    pub fn one_turn(
        &self,
        sel: &JobSelection,
        instructions: &str,
        agent: &'static str,
        prompt: &str,
    ) -> Result<Option<String>, String> {
        if self.stopped() {
            return Ok(None);
        }
        let turn = self.harness.one_off(sel, instructions, agent)?;
        let started = {
            let mut s = self.harness.stoppable.lock().unwrap();
            match s.slots.get_mut(&self.id) {
                Some(slot) if !slot.stopped => {
                    slot.handle = Some(turn.handle.clone());
                    true
                }
                _ => false,
            }
        };
        if !started {
            turn.close();
            return Ok(None);
        }
        let sent = turn.handle.send(prompt);
        // A server ignores an interrupt for a turn it has not started, so a
        // Stop that landed while the prompt went out is repeated here.
        if self.stopped() {
            let _ = turn.handle.cancel();
        }
        let reply = sent.map(|()| turn.wait(None, "answering"));
        turn.close();
        if let Some(slot) = self
            .harness
            .stoppable
            .lock()
            .unwrap()
            .slots
            .get_mut(&self.id)
        {
            slot.handle = None;
        }
        if self.stopped() {
            return Ok(None);
        }
        Ok(Some(reply?.answered()?.to_string()))
    }
}

impl Drop for StopScope<'_> {
    fn drop(&mut self) {
        self.harness
            .stoppable
            .lock()
            .unwrap()
            .slots
            .remove(&self.id);
    }
}
