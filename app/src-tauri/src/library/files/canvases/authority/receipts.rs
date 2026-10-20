//! Per-client operation receipts: the last `WINDOW` accepted `seq`s and the
//! revision each produced, so a retried op is answered without re-applying it.
//! A client keeps at most one unacknowledged op, so its retry always falls
//! inside the window; anything older is `stale-op` and the client resyncs.

use std::collections::{HashMap, VecDeque};

pub const WINDOW: usize = 64;
/// Clients remembered per session; the least recently active is forgotten.
const CLIENTS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub seq: u64,
    pub op_id: String,
    pub revision: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Lookup {
    /// Above every seq seen from this client: apply it.
    Fresh,
    /// Already applied at this revision.
    Replay(u64),
    /// The seq was used for a different op.
    Reused,
    /// Below the retained window, or never accepted.
    Stale,
}

#[derive(Default)]
struct Log {
    last: Option<u64>,
    recent: VecDeque<Receipt>,
    touched: u64,
}

#[derive(Default)]
pub struct Receipts {
    clients: HashMap<String, Log>,
    clock: u64,
}

impl Receipts {
    pub fn lookup(&self, client: &str, seq: u64, op_id: &str) -> Lookup {
        let Some(log) = self.clients.get(client) else {
            return Lookup::Fresh;
        };
        if log.last.is_none_or(|last| seq > last) {
            return Lookup::Fresh;
        }
        match log.recent.iter().find(|r| r.seq == seq) {
            Some(r) if r.op_id == op_id => Lookup::Replay(r.revision),
            Some(_) => Lookup::Reused,
            None => Lookup::Stale,
        }
    }

    pub fn record(&mut self, client: &str, receipt: Receipt) {
        self.clock += 1;
        if !self.clients.contains_key(client) && self.clients.len() >= CLIENTS {
            let oldest = self
                .clients
                .iter()
                .min_by_key(|(_, log)| log.touched)
                .map(|(id, _)| id.clone());
            if let Some(oldest) = oldest {
                self.clients.remove(&oldest);
            }
        }
        let log = self.clients.entry(client.to_string()).or_default();
        log.touched = self.clock;
        log.last = Some(receipt.seq);
        log.recent.push_back(receipt);
        while log.recent.len() > WINDOW {
            log.recent.pop_front();
        }
    }
}
