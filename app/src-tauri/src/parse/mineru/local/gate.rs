//! The local server's one seat, handed out by priority.
//!
//! `mineru-api` serves one request at a time, so parses take turns. A plain
//! mutex would wake an arbitrary waiter; here the next one is the best-ranked
//! (`parse::Priority`), then the one that has waited longest. Rank is read
//! when the seat frees, so a press made while a file waits still counts.
//! Poisoning is stepped over so one panicked parse cannot disable the engine.

use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, MutexGuard};

use crate::parse::priority::sort_key;
use crate::parse::Priority;

pub struct Gate {
    state: Mutex<State>,
    freed: Condvar,
}

struct State {
    busy: bool,
    next_ticket: u64,
    waiting: Vec<(u64, PathBuf)>,
}

/// The seat; dropping it hands the gate to the next waiter.
pub struct Seat<'a> {
    gate: &'a Gate,
}

impl Default for Gate {
    fn default() -> Self {
        Self::new()
    }
}

impl Gate {
    pub const fn new() -> Self {
        Self {
            state: Mutex::new(State {
                busy: false,
                next_ticket: 0,
                waiting: Vec::new(),
            }),
            freed: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Block until the seat is free and `pdf` is the best-ranked waiter.
    /// No timeout: the parse ahead of it takes minutes.
    pub fn acquire(&self, pdf: &Path, priority: &Priority) -> Seat<'_> {
        let mut state = self.lock();
        let ticket = state.next_ticket;
        state.next_ticket += 1;
        state.waiting.push((ticket, pdf.to_path_buf()));
        loop {
            if !state.busy {
                let first = state
                    .waiting
                    .iter()
                    .min_by_key(|(ticket, path)| (sort_key(priority.rank(path)), *ticket))
                    .map(|(ticket, _)| *ticket);
                if first == Some(ticket) {
                    state.waiting.retain(|(other, _)| *other != ticket);
                    state.busy = true;
                    return Seat { gate: self };
                }
            }
            state = self.freed.wait(state).unwrap_or_else(|p| p.into_inner());
        }
    }
}

impl Drop for Seat<'_> {
    fn drop(&mut self) {
        self.gate.lock().busy = false;
        self.gate.freed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// Park three files behind a held seat, release it, and report the order
    /// they were seated in.
    fn seated_order(priority: &Arc<Priority>, names: &[&'static str]) -> Vec<&'static str> {
        let gate = Arc::new(Gate::new());
        let held = gate.acquire(Path::new("/held.pdf"), priority);
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut threads = Vec::new();
        for &name in names {
            let (gate, priority, order) = (gate.clone(), priority.clone(), order.clone());
            threads.push(std::thread::spawn(move || {
                let _seat = gate.acquire(Path::new(&format!("/{name}.pdf")), &priority);
                order.lock().unwrap().push(name);
            }));
            // Arrival order is the tie-break, so make it deterministic.
            std::thread::sleep(Duration::from_millis(60));
        }
        drop(held);
        for thread in threads {
            thread.join().unwrap();
        }
        let order = order.lock().unwrap().clone();
        order
    }

    #[test]
    fn waiters_without_a_rank_go_in_arrival_order() {
        let priority = Arc::new(Priority::new());
        assert_eq!(seated_order(&priority, &["a", "b", "c"]), ["a", "b", "c"]);
    }

    #[test]
    fn a_prioritised_waiter_is_seated_first() {
        let priority = Arc::new(Priority::new());
        priority.prioritise(Path::new("/c.pdf"));
        assert_eq!(seated_order(&priority, &["a", "b", "c"]), ["c", "a", "b"]);
    }

    #[test]
    fn several_prioritised_waiters_keep_the_order_they_were_pressed_in() {
        let priority = Arc::new(Priority::new());
        priority.prioritise(Path::new("/c.pdf"));
        priority.prioritise(Path::new("/b.pdf"));
        assert_eq!(seated_order(&priority, &["a", "b", "c"]), ["c", "b", "a"]);
    }

    #[test]
    fn a_running_parse_is_never_displaced() {
        let gate = Gate::new();
        let priority = Priority::new();
        let held = gate.acquire(Path::new("/running.pdf"), &priority);
        priority.prioritise(Path::new("/urgent.pdf"));
        assert!(gate.lock().busy, "the seat stays with the running parse");
        drop(held);
        let _next = gate.acquire(Path::new("/urgent.pdf"), &priority);
    }
}
