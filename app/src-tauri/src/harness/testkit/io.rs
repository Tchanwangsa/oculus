//! The replay consumer's side of the bus: its checked external effects, and
//! the feed that hands it messages until the test has fed everything.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use tokio::runtime::Runtime;

use crate::harness::app::consumer::ConsumerIo;
use crate::harness::app::{BusMsg, Envelope};
use crate::harness::{jobs, store, Provider, QueuedMessage, SendOptions};

use super::{guard::assert_scratch, outbound::Outbound, snapshot};

pub(super) struct Io {
    pub pool: SqlitePool,
    pub database: PathBuf,
    pub scratch: PathBuf,
    pub outbound: Outbound,
    pub envelopes: Vec<Value>,
    pub snapshots: Vec<Vec<Value>>,
    pub runtime: Option<Runtime>,
    pub epoch: std::time::Instant,
    /// A continuation's dispatch, which the replay starts itself.
    pub continued: Sender<(QueuedMessage, SendOptions)>,
}

impl ConsumerIo for Io {
    fn clock_time(&self) -> Option<std::time::Instant> {
        Some(self.epoch + Duration::from_millis(self.envelopes.len() as u64 * 10))
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
    fn dispatch(&mut self, _: i64, _: Provider, msg: QueuedMessage, opts: SendOptions) {
        if opts.continuation.is_some() {
            let _ = self.continued.send((msg, opts));
            return;
        }
        self.outbound
            .exchange(json!({"operation":"queued dispatch", "text":msg.text}));
    }
    fn emit(&mut self, envelope: Envelope) {
        self.snapshots.push(
            self.runtime
                .as_ref()
                .unwrap()
                .block_on(snapshot::items(&self.pool, envelope.thread_id)),
        );
        self.envelopes.push(serde_json::to_value(envelope).unwrap());
    }
}

/// The bus until the replay has fed everything: the bridges keep sinks alive,
/// so the channel itself never closes. A bus left empty answers whoever asked
/// on `drained`, which is how a gate knows the consumer has caught up.
pub(super) struct Feed {
    pub rx: Receiver<BusMsg>,
    pub fed: Arc<AtomicBool>,
    pub drained: Receiver<Sender<()>>,
}

impl Iterator for Feed {
    type Item = BusMsg;

    fn next(&mut self) -> Option<BusMsg> {
        loop {
            match self.rx.recv_timeout(Duration::from_millis(5)) {
                Ok(msg) => return Some(msg),
                Err(RecvTimeoutError::Timeout) => {
                    while let Ok(waiter) = self.drained.try_recv() {
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
