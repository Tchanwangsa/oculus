//! The suggestion entry points on [`Harness`]: start, supersede, cancel and
//! the warm spare.

use std::sync::Arc;

use crate::harness::jobs::JobSelection;
use crate::harness::manager::{Harness, OneOff};
use crate::harness::opencode;

use super::prompt::prompt;
use super::reply::clean;
use super::{warmable, Key, Warm, INSTRUCTIONS};

impl Harness {
    /// One completion for the caret between `before` and `after` in the note
    /// at `path` (library-relative). Empty when there is nothing to add, or
    /// when a newer request or a cancel superseded this one.
    pub fn suggest(
        self: &Arc<Self>,
        request_id: u64,
        sel: &JobSelection,
        path: &str,
        before: &str,
        after: &str,
    ) -> Result<String, String> {
        let stale = {
            let mut s = self.suggest.lock().unwrap();
            // An older request arriving after a newer one is already stale.
            if s.wanted.is_some_and(|w| w > request_id) {
                return Ok(String::new());
            }
            s.wanted = Some(request_id);
            s.running.take()
        };
        // Outside the lock: on Codex this is a request to the server.
        if let Some((_, h)) = stale {
            let _ = h.cancel();
        }
        let Some(prompt) = prompt(path, before, after) else {
            self.release(request_id);
            return Ok(String::new());
        };

        let key = Key::of(sel);
        let turn = match self.take_warm(&key) {
            Some(t) => t,
            None => match self.one_off(sel, INSTRUCTIONS, opencode::WRITER_AGENT) {
                Ok(t) => t,
                Err(e) => {
                    self.release(request_id);
                    return Err(e);
                }
            },
        };
        {
            let mut s = self.suggest.lock().unwrap();
            if s.wanted != Some(request_id) {
                // Never prompted, so it is still a spare.
                drop(s);
                self.keep_warm(key, turn);
                return Ok(String::new());
            }
            s.running = Some((request_id, turn.handle.clone()));
        }

        let sent = turn.handle.send(&prompt);
        // A cancel that landed while the prompt was going out can miss a
        // turn the server had not started yet, so it is repeated here.
        if self.suggest.lock().unwrap().wanted != Some(request_id) {
            let _ = turn.handle.cancel();
        }
        let reply = sent.map(|()| turn.wait(None, "suggesting"));
        turn.close();
        let current = {
            let mut s = self.suggest.lock().unwrap();
            if s.running.as_ref().is_some_and(|(id, _)| *id == request_id) {
                s.running = None;
            }
            s.wanted == Some(request_id)
        };
        self.release(request_id);
        self.refill_warm(sel);
        if !current {
            return Ok(String::new());
        }
        let reply = reply?;
        if let Some(e) = reply.failed {
            return Err(e);
        }
        let raw = if reply.streamed.trim().is_empty() {
            &reply.message
        } else {
            &reply.streamed
        };
        Ok(clean(raw, before, after))
    }

    /// Stop whatever suggestion is in flight; its call answers empty. The
    /// warm spare stays.
    pub fn cancel_suggestion(&self) {
        let running = {
            let mut s = self.suggest.lock().unwrap();
            s.wanted = None;
            s.running.take()
        };
        if let Some((_, h)) = running {
            let _ = h.cancel();
        }
    }

    /// The turn in flight and the spare, on quit.
    pub(in crate::harness) fn drop_suggestions(&self) {
        self.cancel_suggestion();
        let warm = self.suggest.lock().unwrap().warm.take();
        if let Some(w) = warm {
            w.turn.close();
        }
    }

    /// This request is over; a later one with a lower id (a reloaded page
    /// counts from 1 again) is no longer refused.
    fn release(&self, request_id: u64) {
        let mut s = self.suggest.lock().unwrap();
        if s.wanted == Some(request_id) {
            s.wanted = None;
        }
    }

    /// The spare, if it was started under this selection and is still alive.
    /// Any other spare is closed.
    fn take_warm(&self, key: &Key) -> Option<OneOff> {
        let warm = self.suggest.lock().unwrap().warm.take()?;
        if warm.key == *key && warm.turn.handle.is_alive() {
            return Some(warm.turn);
        }
        warm.turn.close();
        None
    }

    /// Keep `turn` as the spare when there is none, else close it.
    fn keep_warm(&self, key: Key, turn: OneOff) {
        let extra = {
            let mut s = self.suggest.lock().unwrap();
            let have = s.warm.as_ref().is_some_and(|w| w.turn.handle.is_alive());
            if warmable(key.provider) && !have {
                s.warm.replace(Warm { key, turn }).map(|w| w.turn)
            } else {
                Some(turn)
            }
        };
        if let Some(t) = extra {
            t.close();
        }
    }

    /// Start the next request's process now, off the caller's thread.
    fn refill_warm(self: &Arc<Self>, sel: &JobSelection) {
        if !warmable(sel.provider) {
            return;
        }
        let key = Key::of(sel);
        {
            let s = self.suggest.lock().unwrap();
            if s.warm
                .as_ref()
                .is_some_and(|w| w.key == key && w.turn.handle.is_alive())
            {
                return;
            }
        }
        let (h, sel) = (self.clone(), sel.clone());
        std::thread::spawn(
            move || match h.one_off(&sel, INSTRUCTIONS, opencode::WRITER_AGENT) {
                Ok(turn) => h.keep_warm(key, turn),
                Err(e) => eprintln!("[oculus] suggestion warm-up: {e}"),
            },
        );
    }
}
