//! The replay's runtime, driven a step at a time: production handles, the
//! production consumer on its own thread and a migrated scratch database.
//! [`super::replay::run`] plays a whole recording through it; a test that has
//! to act mid-turn pauses at a gate ([`Scenario::gate`], [`Rig::until`]).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use tokio::runtime::Runtime;

use crate::harness::app::consumer::consume;
use crate::harness::app::send::begin_run;
use crate::harness::app::BusMsg;
use crate::harness::{
    jobs, store, Harness, HarnessEvent, Provider, Queue, QueuedMessage, SendOptions, Sink, Sinks,
};

use super::io::{Clock, Feed, Io};
use super::replay::{Bridge, Replay};
use super::{guard::assert_scratch, outbound::Outbound, scenario::Scenario, snapshot};

pub(super) struct Rig {
    pub scenario: Scenario,
    scratch: crate::test_support::Scratch,
    database: PathBuf,
    pub runtime: Runtime,
    pub pool: SqlitePool,
    pub thread: i64,
    pub provider: Provider,
    pub harness: Harness,
    pub bus: Sender<BusMsg>,
    /// Where the app's own events go: a send that failed before its turn.
    pub sink: Sink,
    /// Where the thread's sessions report, as in the app.
    pub sinks: Arc<dyn Sinks>,
    pub queue: Arc<Mutex<Queue>>,
    outbound: Outbound,
    bridge: Bridge,
    fed: Arc<AtomicBool>,
    clock: Arc<Clock>,
    drained: Sender<Sender<()>>,
    consumer: JoinHandle<Io>,
    /// Continuations the consumer dispatched, which the rig starts itself.
    dispatched: Receiver<(QueuedMessage, SendOptions)>,
    card: Option<String>,
    continuations: Vec<SendOptions>,
    /// How many of the scenario's frames have gone in.
    fed_frames: usize,
}

impl Rig {
    /// The scratch database, the consumer and the recording's bridge, with
    /// nothing sent yet.
    pub fn start(scenario: Scenario) -> Self {
        let scratch = super::guard::scratch("turn-replay");
        let database = assert_scratch(&scratch.join("oculus.db"), &scratch);
        let runtime = Runtime::new().unwrap();
        let pool = runtime.block_on(async {
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(
                    SqliteConnectOptions::new()
                        .filename(&database)
                        .create_if_missing(true),
                )
                .await
                .unwrap();
            for migration in crate::db::migrations::all() {
                sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
            }
            crate::db::store::set_setting(
                &pool,
                jobs::SETTINGS_KEY,
                r#"{"threadNaming":[],"studioResearch":[],"studioScene":[],"widgetAsk":[]}"#,
            )
            .await
            .unwrap();
            pool
        });
        let provider = scenario.fixture.provider;
        let thread = runtime
            .block_on(store::create_thread(
                &pool,
                provider,
                None,
                None,
                None,
                "Replay request",
            ))
            .unwrap();
        let harness = Harness::replay(assert_scratch(&scratch, &scratch));
        assert_scratch(&crate::harness::thread_cwd(&scratch), &scratch);
        // A continuation sends its card's prompt where the recording sent the request.
        let card = scenario
            .continuation
            .then(|| runtime.block_on(super::continuation::seed_card(&pool, thread)));
        let mut expected = scenario.fixture.expected_outbound.clone();
        if let Some(card) = &card {
            let prompt = runtime.block_on(super::continuation::prompt_for(&pool, thread, card));
            for e in &mut expected {
                e.frame = super::continuation::resend_as(e.frame.take(), &prompt);
            }
        }
        let outbound = Outbound::new(expected, &scratch);
        // The consumer runs beside the replay, as in the app: a send's run is
        // written on it before the provider write.
        let (bus, rx) = std::sync::mpsc::channel::<BusMsg>();
        let sink = crate::harness::app::sink_for(&bus, thread, provider);
        let sinks = crate::harness::app::sinks_for(&bus, thread, provider);
        let queue = Arc::new(Mutex::new(Queue::default()));
        let fed = Arc::new(AtomicBool::new(false));
        let (continued, dispatched) = std::sync::mpsc::channel();
        let (drained, waiters) = std::sync::mpsc::channel();
        let clock = Clock::new();
        let consumer = {
            let (queue, fed, clock) = (queue.clone(), fed.clone(), clock.clone());
            let mut io = Io {
                pool: pool.clone(),
                database: database.clone(),
                scratch: scratch.to_path_buf(),
                outbound: outbound.clone(),
                envelopes: Vec::new(),
                snapshots: Vec::new(),
                runtime: None,
                epoch: std::time::Instant::now(),
                continued,
                clock: clock.clone(),
            };
            std::thread::spawn(move || {
                io.runtime = Some(Runtime::new().unwrap());
                let feed = Feed {
                    rx,
                    fed,
                    drained: waiters,
                    clock,
                };
                consume(feed, queue, &mut io);
                io
            })
        };
        let bridge = session(
            &harness,
            thread,
            provider,
            &outbound,
            &scenario.fixture.native_session,
            &scratch,
        );
        Rig {
            scenario,
            scratch,
            database,
            runtime,
            pool,
            thread,
            provider,
            harness,
            bus,
            sink,
            sinks,
            queue,
            outbound,
            bridge,
            fed,
            clock,
            drained,
            consumer,
            dispatched,
            card,
            continuations: Vec::new(),
            fed_frames: 0,
        }
    }

    /// The recording's request: a student's send, or the continuation of the
    /// card the scenario seeded, then the scenario's storage faults.
    pub fn open(&mut self) {
        if let Some(card) = self.card.clone() {
            self.continue_card(&card);
        }
        let student = self.card.is_none();
        if self.scenario.fail_intent {
            self.runtime.block_on(sqlx::raw_sql("CREATE TRIGGER replay_intent_failure BEFORE INSERT ON harness_items WHEN NEW.kind = 'user' BEGIN SELECT RAISE(FAIL, 'replay intent failure'); END;").execute(&self.pool)).unwrap();
        }
        if self.scenario.fail_write {
            self.runtime.block_on(sqlx::raw_sql("CREATE TRIGGER replay_write_failure BEFORE INSERT ON harness_items WHEN NEW.kind = 'assistant' BEGIN SELECT RAISE(FAIL, 'replay storage failure'); END;").execute(&self.pool)).unwrap();
        }
        if student {
            let _ = self.send("replay-send");
        }
    }

    /// The card's click settled it: the consumer claims the idle thread and
    /// dispatches, and the rig runs the production start steps.
    fn continue_card(&mut self, card: &str) {
        self.bus
            .send(BusMsg::app(
                self.thread,
                self.provider,
                HarnessEvent::CardSettled {
                    proposal: card.into(),
                },
            ))
            .unwrap();
        let (msg, opts) = self
            .dispatched
            .recv_timeout(Duration::from_secs(10))
            .expect("the settled card dispatched its continuation");
        let started = self.runtime.block_on(crate::harness::app::send::begin_turn(
            &self.pool,
            &self.bus,
            store::SendIntent {
                thread_id: self.thread,
                provider: self.provider,
                model: None,
                native_session: None,
                receipt: "replay-continuation".into(),
                text: msg.text,
                at: None,
                continuation: opts.continuation.clone(),
            },
            None,
        ));
        let (run, prompt, cards) = started.unwrap();
        self.continuations.push(opts);
        self.harness
            .send(
                self.thread,
                self.provider,
                None,
                &SendOptions::default(),
                &prompt,
                &self.sinks,
            )
            .unwrap();
        self.runtime
            .block_on(store::mark_sent(&self.pool, &run))
            .unwrap();
        self.runtime
            .block_on(crate::harness::app::proposal_context::accepted(
                &self.pool, cards,
            ))
            .unwrap();
    }

    /// One student send under `receipt`, as the app makes it: the run is
    /// written on the consumer, then the provider hears of it. A refusal
    /// closes the turn the way `send::dispatch` does.
    pub fn send(&mut self, receipt: &str) -> Result<store::RunStart, String> {
        self.send_with(receipt, &SendOptions::default())
    }

    /// [`Self::send`] under the composer's `opts` (a reasoning level).
    pub fn send_with(
        &mut self,
        receipt: &str,
        opts: &SendOptions,
    ) -> Result<store::RunStart, String> {
        assert!(
            self.queue.lock().unwrap().try_claim(self.thread),
            "a send on a busy thread waits in the queue"
        );
        let begun = self.runtime.block_on(begin_run(
            &self.bus,
            store::SendIntent {
                thread_id: self.thread,
                provider: self.provider,
                model: None,
                native_session: None,
                receipt: receipt.into(),
                text: "Replay request".into(),
                at: None,
                continuation: None,
            },
        ));
        let sent = begun.and_then(|run| {
            self.harness.send(
                self.thread,
                self.provider,
                None,
                opts,
                "Replay request",
                &self.sinks,
            )?;
            self.runtime.block_on(store::mark_sent(&self.pool, &run))?;
            Ok(run)
        });
        if let Err(error) = &sent {
            crate::harness::child::fail_turn(&self.sink, self.provider, error.clone(), None);
        }
        sent
    }

    /// A second recorded session for the thread, which the manager's next
    /// spawn opens. Returns the one it replaces, whose reader the test may
    /// still drive: a process the app no longer listens to.
    pub fn next_session(&mut self) -> Bridge {
        let next = session(
            &self.harness,
            self.thread,
            self.provider,
            &self.outbound,
            &self.scenario.fixture.native_session,
            &self.scratch,
        );
        std::mem::replace(&mut self.bridge, next)
    }

    /// Feed one inbound frame through the bridge's production reader.
    pub fn feed(&mut self, frame: &Value) {
        (self.bridge.inbound)(frame);
    }

    /// Feed the recording up to the frame `label` names, leaving that frame
    /// unfed, and wait for the consumer to catch up: the turn is paused there.
    pub fn until(&mut self, label: &str) {
        let (_, at) = self
            .scenario
            .gates
            .iter()
            .find(|(l, _)| l == label)
            .unwrap_or_else(|| panic!("no gate {label}"))
            .clone();
        assert!(at >= self.fed_frames, "gate {label} is already behind");
        self.feed_to(at);
        self.drain();
    }

    /// Feed the rest of the recording.
    pub fn resume(&mut self) {
        self.feed_to(self.scenario.frames.len());
    }

    fn feed_to(&mut self, end: usize) {
        for index in self.fed_frames..end {
            let frame = self.scenario.frames[index].clone();
            self.feed(&frame);
        }
        self.fed_frames = end;
    }

    /// Wait until the consumer has applied everything sent so far.
    pub fn drain(&self) {
        let (done, waiting) = std::sync::mpsc::channel();
        self.drained.send(done).unwrap();
        waiting
            .recv_timeout(Duration::from_secs(10))
            .expect("the consumer caught up");
    }

    /// Make every write to the database fail (`on`), or work again: the
    /// rig's one connection goes read-only, as a full disk would leave it.
    pub fn read_only(&self, on: bool) {
        let pragma = match on {
            true => "PRAGMA query_only = ON",
            false => "PRAGMA query_only = OFF",
        };
        self.runtime
            .block_on(sqlx::query(pragma).execute(&self.pool))
            .unwrap();
    }

    /// How many times the consumer has been woken for work that waits.
    pub fn wakes(&self) -> u64 {
        self.clock.fired.load(Ordering::SeqCst)
    }

    /// Wait until the consumer has been woken `count` times in all.
    pub fn await_wakes(&self, count: u64) {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while self.wakes() < count {
            assert!(
                std::time::Instant::now() < deadline,
                "the consumer was not woken {count} times"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// The transport closing, through the bridge's production EOF path.
    pub fn eof(&mut self) {
        (self.bridge.eof)();
    }

    /// The recording's end: a second view's event for the same card, then the
    /// transport closing. A restart never sees the transport close.
    pub fn close(&mut self) {
        // The same card settling again (a second view's event) starts nothing.
        if let Some(card) = &self.card {
            self.bus
                .send(BusMsg::app(
                    self.thread,
                    self.provider,
                    HarnessEvent::CardSettled {
                        proposal: card.clone(),
                    },
                ))
                .unwrap();
        }
        if !self.scenario.restart {
            self.eof();
        }
    }

    /// Stop the consumer and read what it left.
    pub fn finish(mut self) -> Replay {
        self.fed.store(true, Ordering::SeqCst);
        let io = self.consumer.join().unwrap();
        let (runtime, pool, thread) = (&self.runtime, &self.pool, self.thread);
        if self.scenario.restart {
            runtime.block_on(store::reconcile(pool)).unwrap();
        }
        let items = runtime.block_on(snapshot::items(pool, thread));
        let schema = runtime
            .block_on(
                sqlx::query_scalar::<_, String>(
                    "SELECT name FROM sqlite_master WHERE type = 'table'",
                )
                .fetch_all(pool),
            )
            .unwrap();
        let ledger = runtime.block_on(snapshot::ledger(pool, thread));
        let busy = self.queue.lock().unwrap().is_busy(thread);
        runtime.block_on(pool.close());
        let reopened_pool = runtime
            .block_on(crate::db::store::pool(&assert_scratch(
                &self.database,
                &self.scratch,
            )))
            .unwrap();
        let reopened = runtime.block_on(snapshot::items(&reopened_pool, thread));
        runtime.block_on(reopened_pool.close());
        self.harness.shutdown();
        drop(self.bridge);
        self.continuations
            .extend(self.dispatched.try_iter().map(|(_, opts)| opts));
        Replay {
            envelopes: io.envelopes,
            snapshots: io.snapshots,
            items,
            reopened,
            // A send refused before its write leaves the recorded write unused.
            outbound: self.outbound.finish(!self.scenario.fail_intent),
            busy,
            schema,
            ledger,
            repeat_envelope: self.scenario.repeat_envelope,
            restarted: self.scenario.restart,
            load_race: self.scenario.load_race,
            continuations: self.continuations,
        }
    }
}

/// A recorded session of `provider`, registered as the thread's next spawn.
/// Its bridge is built now, so the test can feed it, on a sink that goes
/// nowhere until the manager opens the session and mints its generation: a
/// session never opened (its send was refused) has no reader to hear it.
fn session(
    harness: &Harness,
    thread: i64,
    provider: Provider,
    outbound: &Outbound,
    native_session: &str,
    scratch: &std::path::Path,
) -> Bridge {
    let opened: Arc<Mutex<Option<Sink>>> = Arc::default();
    let sink: Sink = {
        let opened = opened.clone();
        Arc::new(move |ev| {
            let sink = opened.lock().unwrap().clone();
            if let Some(sink) = sink {
                sink(ev);
            }
        })
    };
    let bridge = match provider {
        Provider::Claude => crate::harness::claude::replay::bridge(outbound.wire(), sink),
        Provider::Codex => {
            crate::harness::codex::replay::bridge(outbound.clone(), sink, native_session, scratch)
        }
        Provider::Opencode => crate::harness::opencode::replay::bridge(
            outbound.clone(),
            sink,
            native_session,
            scratch,
        ),
        Provider::Antigravity => crate::harness::antigravity::replay::bridge(outbound.wire(), sink),
    };
    let handle = bridge.handle.clone();
    harness.replay_spawn(thread, move |sink| {
        *opened.lock().unwrap() = Some(sink);
        handle
    });
    bridge
}
