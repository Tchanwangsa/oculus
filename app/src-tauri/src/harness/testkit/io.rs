//! The replay consumer's side of the bus: its checked external effects, and
//! the feed that hands it messages until the test has fed everything.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use tokio::runtime::Runtime;

use crate::harness::app::consumer::ConsumerIo;
use crate::harness::app::{BusMsg, Envelope};
use crate::harness::core::effect::{self, Effect};
use crate::harness::providers::capabilities::Capabilities;
use crate::harness::{jobs, store, Provider};

use super::{guard::assert_scratch, outbound::Outbound, snapshot};

pub(super) struct Io {
    pub pool: SqlitePool,
    pub database: PathBuf,
    pub scratch: PathBuf,
    pub outbound: Outbound,
    pub envelopes: Vec<Value>,
    /// The same envelopes, for a test to read while the consumer runs.
    pub told: Arc<Mutex<Vec<Value>>>,
    pub snapshots: Vec<Vec<Value>>,
    pub runtime: Option<Runtime>,
    pub epoch: std::time::Instant,
    /// What the consumer's effects run against: the replay's manager, its
    /// bus and the scratch database.
    pub effects: effect::Runtime,
    /// The cards whose continuation the consumer started.
    pub continued: Vec<String>,
    /// Effects kept back for the test to run when it chooses
    /// (`Rig::hold_effects`); `None` runs each as it is asked for.
    pub kept: Arc<Mutex<Option<Vec<Effect>>>>,
    pub clock: Arc<Clock>,
}

/// The consumer's time in a replay. It moves only when the feed, finding
/// the bus empty, jumps to the moment the consumer asked to be woken at: no
/// test waits on a real timer.
pub(super) struct Clock {
    now: Mutex<Instant>,
    wake: Mutex<Option<Instant>>,
    /// How many wakes the feed has delivered.
    pub fired: AtomicU64,
}

impl Clock {
    pub fn new() -> Arc<Self> {
        Arc::new(Clock {
            now: Mutex::new(Instant::now()),
            wake: Mutex::new(None),
            fired: AtomicU64::new(0),
        })
    }

    /// The pending wake, with the clock moved to it.
    fn fire(&self) -> bool {
        let Some(at) = self.wake.lock().unwrap().take() else {
            return false;
        };
        let mut now = self.now.lock().unwrap();
        *now = (*now).max(at);
        self.fired.fetch_add(1, Ordering::SeqCst);
        true
    }
}

impl ConsumerIo for Io {
    fn capabilities(&self, provider: Provider) -> Capabilities {
        self.effects.harness.capabilities(provider)
    }
    fn background(&self, thread_id: i64) -> bool {
        self.effects.harness.has_background_work(thread_id)
    }
    fn library(&self) -> std::path::PathBuf {
        self.scratch.clone()
    }
    fn clock_time(&self) -> Option<std::time::Instant> {
        Some(self.epoch + Duration::from_millis(self.envelopes.len() as u64 * 10))
    }
    fn now(&self) -> Instant {
        *self.clock.now.lock().unwrap()
    }
    fn wake_at(&mut self, at: Instant) {
        *self.clock.wake.lock().unwrap() = Some(at);
    }
    fn open_pool(&mut self, rt: &Runtime) -> Option<SqlitePool> {
        assert_scratch(&self.database, &self.scratch);
        let databases = rt
            .block_on(sqlx::query("PRAGMA database_list").fetch_all(&self.pool))
            .unwrap();
        for database in databases {
            let file: String = database.get("file");
            if !file.is_empty() {
                assert_scratch(Path::new(&file), &self.scratch);
            }
        }
        Some(self.pool.clone())
    }
    fn name(&mut self, _: i64, _: Provider, _: Vec<jobs::JobSelection>, _: store::NamingSeed) {
        self.outbound.exchange(json!({"operation":"thread naming"}));
    }
    /// An effect runs at once, on the consumer's thread, unless the test
    /// keeps them back: the recorded session answers inline, so the turn is
    /// open when the command returns.
    /// Its writes to the provider are checked like any other, so a start the
    /// recording did not make fails the replay.
    fn perform(&mut self, effect: Effect) {
        if let Effect::Start(start) = &effect {
            self.continued.extend(start.options.continuation.clone());
        }
        if let Some(kept) = self.kept.lock().unwrap().as_mut() {
            kept.push(effect);
            return;
        }
        let runtime = self.runtime.as_ref().unwrap();
        runtime.block_on(effect::run(&self.effects, effect));
    }
    fn emit(&mut self, envelope: Envelope) {
        self.snapshots.push(
            self.runtime
                .as_ref()
                .unwrap()
                .block_on(snapshot::items(&self.pool, envelope.thread_id)),
        );
        let envelope = serde_json::to_value(envelope).unwrap();
        self.told.lock().unwrap().push(envelope.clone());
        self.envelopes.push(envelope);
    }
}

/// The bus until the replay has fed everything: the bridges keep sinks alive,
/// so the channel itself never closes. A bus left empty first delivers the
/// wake the consumer asked for, then answers whoever asked on `drained`,
/// which is how a gate knows the consumer has caught up. A message that
/// arrived with the request is handled before the answer.
pub(super) struct Feed {
    pub rx: Receiver<BusMsg>,
    pub fed: Arc<AtomicBool>,
    pub drained: Receiver<Sender<()>>,
    pub clock: Arc<Clock>,
    /// Whoever asked to be told when the consumer has caught up, taken off
    /// `drained` and not answered yet.
    pub waiting: Vec<Sender<()>>,
}

impl Iterator for Feed {
    type Item = BusMsg;

    fn next(&mut self) -> Option<BusMsg> {
        loop {
            match self.rx.recv_timeout(Duration::from_millis(5)) {
                Ok(msg) => return Some(msg),
                Err(RecvTimeoutError::Timeout) => {
                    if self.clock.fire() {
                        return Some(BusMsg::Wake);
                    }
                    // A waiter is answered only once the bus is empty with
                    // the waiter already in hand: whoever asks has sent
                    // their messages first, and one that landed between
                    // the timeout above and here must be handled before
                    // they are told the consumer has caught up.
                    while let Ok(waiter) = self.drained.try_recv() {
                        self.waiting.push(waiter);
                    }
                    if let Ok(msg) = self.rx.try_recv() {
                        return Some(msg);
                    }
                    for waiter in self.waiting.drain(..) {
                        let _ = waiter.send(());
                    }
                    if self.fed.load(Ordering::SeqCst) {
                        return self.rx.try_recv().ok();
                    }
                }
                Err(RecvTimeoutError::Disconnected) => return None,
            }
        }
    }
}
