//! The replay's runtime, driven a step at a time: production handles, the
//! production consumer on its own thread and a migrated scratch database.
//! [`super::replay::run`] plays a whole recording through it; a test that has
//! to act mid-turn pauses at a gate ([`Scenario::gate`], [`Rig::until`]).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use tokio::runtime::Runtime;

use crate::harness::app::consumer::consume;
use crate::harness::app::startup::Startup;
use crate::harness::app::BusMsg;
use crate::harness::core::effect::{self, Effect, Pools, Runtime as Effects};
use crate::harness::core::{Command, Kind, Outcome};
use crate::harness::{
    jobs, store, Harness, HarnessEvent, OwnTurns, Provider, SendOptions, Sink, Sinks,
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
    /// The gate a command waits at, as `HarnessState` holds it: already
    /// passed in a fresh rig, not yet after [`Rig::restart`].
    startup: Startup,
    pub harness: Arc<Harness>,
    pub bus: Sender<BusMsg>,
    /// Where the app's own events go: a send that failed before its turn.
    pub sink: Sink,
    /// Where the thread's sessions report, as in the app.
    pub sinks: Arc<dyn Sinks>,
    pub own_turns: Arc<Mutex<OwnTurns>>,
    /// What the consumer's effects run against, for one the test runs.
    effects: Effects,
    kept: Arc<Mutex<Option<Vec<Effect>>>>,
    told: Arc<Mutex<Vec<Value>>>,
    outbound: Outbound,
    bridge: Bridge,
    fed: Arc<AtomicBool>,
    clock: Arc<Clock>,
    drained: Sender<Sender<()>>,
    consumer: Option<JoinHandle<Io>>,
    /// What the consumers of earlier runtimes emitted, before a restart.
    envelopes: Vec<Value>,
    snapshots: Vec<Vec<Value>>,
    card: Option<String>,
    /// The cards whose continuation a consumer started.
    continuations: Vec<String>,
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
        // A continuation sends its card's prompt where the recording sent the request.
        let card = scenario
            .continuation
            .then(|| runtime.block_on(super::continuation::seed_card(&pool, thread)));
        // The card's reply came from a session, which a chat that holds one
        // has: without it the reply would be history a new session is owed.
        if card.is_some() {
            let seeded = sqlx::query(
                "INSERT INTO harness_bindings (thread_id, provider, native_session, current)
                 VALUES (?1, ?2, 'seeded-session', 1)",
            )
            .bind(thread)
            .bind(provider.as_str())
            .execute(&pool);
            runtime.block_on(seeded).unwrap();
        }
        let mut expected = scenario.fixture.expected_outbound.clone();
        if let Some(card) = &card {
            let prompt = runtime.block_on(super::continuation::prompt_for(&pool, thread, card));
            for e in &mut expected {
                e.frame = super::continuation::resend_as(e.frame.take(), &prompt);
            }
        }
        let outbound = Outbound::new(expected, &scratch);
        let running = boot(
            &pool,
            &database,
            &scratch,
            &outbound,
            thread,
            provider,
            &scenario.fixture.native_session,
        );
        Rig {
            scenario,
            scratch,
            database,
            runtime,
            pool,
            thread,
            provider,
            startup: Startup::repaired(),
            harness: running.harness,
            bus: running.bus,
            sink: running.sink,
            sinks: running.sinks,
            own_turns: running.own_turns,
            effects: running.effects,
            kept: running.kept,
            told: running.told,
            outbound,
            bridge: running.bridge,
            fed: running.fed,
            clock: running.clock,
            drained: running.drained,
            consumer: Some(running.consumer),
            envelopes: Vec::new(),
            snapshots: Vec::new(),
            card,
            continuations: Vec::new(),
            fed_frames: 0,
        }
    }

    /// The app is killed and started again on the same database file: the
    /// consumer, the manager and every session go, and new ones come up as
    /// `lifecycle::init` makes them. Nothing has run startup's repair yet;
    /// [`Self::startup`] does, as does the first [`Self::send`], through the
    /// gate the app's commands wait at.
    pub fn restart(mut self) -> Self {
        self.stop();
        self.runtime.block_on(self.pool.close());
        self.harness.shutdown();
        self.pool = self
            .runtime
            .block_on(crate::db::store::pool(&assert_scratch(
                &self.database,
                &self.scratch,
            )))
            .unwrap();
        let running = boot(
            &self.pool,
            &self.database,
            &self.scratch,
            &self.outbound,
            self.thread,
            self.provider,
            &self.scenario.fixture.native_session,
        );
        self.startup = Startup::default();
        self.harness = running.harness;
        self.bus = running.bus;
        self.sink = running.sink;
        self.sinks = running.sinks;
        self.own_turns = running.own_turns;
        self.effects = running.effects;
        self.kept = running.kept;
        self.told = running.told;
        self.bridge = running.bridge;
        self.fed = running.fed;
        self.clock = running.clock;
        self.drained = running.drained;
        self.consumer = Some(running.consumer);
        self
    }

    /// Startup's repair, through the production gate (`app/startup.rs`).
    pub fn startup(&self) -> Result<(), String> {
        let pool = self.pool.clone();
        self.runtime
            .block_on(self.startup.ready(|| async { Ok(pool) }))
    }

    /// Stop the consumer, keeping what it emitted.
    fn stop(&mut self) {
        self.fed.store(true, Ordering::SeqCst);
        if let Some(consumer) = self.consumer.take() {
            let io = consumer.join().unwrap();
            self.envelopes.extend(io.envelopes);
            self.snapshots.extend(io.snapshots);
            self.continuations.extend(io.continued);
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

    /// The card's click settled it: the consumer claims the idle thread,
    /// writes the continuation's run and sends it.
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
        self.drain();
    }

    /// One student send under the command id `receipt`, as `harness_send`
    /// makes it: decided and committed on the consumer, then sent to the
    /// provider by the start step. The run it started, or the refusal.
    pub fn send(&mut self, receipt: &str) -> Result<store::RunStart, String> {
        self.send_with(receipt, &SendOptions::default())
    }

    /// [`Self::send`] under the composer's `opts` (a reasoning level).
    pub fn send_with(
        &mut self,
        receipt: &str,
        opts: &SendOptions,
    ) -> Result<store::RunStart, String> {
        let sent = self.command(Command {
            id: receipt.into(),
            thread_id: Some(self.thread),
            kind: Kind::Send {
                provider: self.provider,
                text: "Replay request".into(),
                options: opts.clone(),
                mode: Default::default(),
            },
        })?;
        sent.run
            .ok_or_else(|| "the send waits behind the running turn".to_string())
    }

    /// One command through the consumer, as a Tauri command sends it: it
    /// waits at startup's gate first, then for the commit. The effects the
    /// commit left have run by the time this returns.
    pub fn command(&mut self, command: Command) -> Result<Outcome, String> {
        self.startup()?;
        let (reply, answer) = tokio::sync::oneshot::channel();
        self.bus
            .send(BusMsg::Command { command, reply })
            .map_err(|_| "the consumer has stopped".to_string())?;
        self.runtime
            .block_on(answer)
            .map_err(|_| "the consumer has stopped".to_string())?
    }

    /// A turn has the thread: a run going out or being answered, or a turn
    /// its CLI started itself.
    pub fn busy(&self) -> bool {
        let open = store::queue::open(&self.pool, self.thread);
        self.runtime.block_on(open).unwrap().is_some()
            || self.own_turns.lock().unwrap().is_held(self.thread)
    }

    /// From now on the consumer's effects wait for [`Self::run_effects`]:
    /// the gap between a commit and its provider call, held open.
    pub fn hold_effects(&self) {
        self.kept.lock().unwrap().get_or_insert_with(Vec::new);
    }

    /// The effects kept back so far, without running them.
    pub fn kept_effects(&self) -> Vec<Effect> {
        let mut kept = self.kept.lock().unwrap();
        kept.as_mut().map(std::mem::take).unwrap_or_default()
    }

    /// Run the effects kept back so far, as the app would have, and wait
    /// for the consumer to hear how they went.
    pub fn run_effects(&self) {
        for effect in self.kept_effects() {
            self.runtime.block_on(effect::run(&self.effects, effect));
        }
        self.drain();
    }

    /// Another chat of the same provider, with a recorded session of its
    /// own for its first send: its thread, and the bridge the test feeds.
    pub fn second_chat(&mut self) -> (i64, Bridge) {
        let created = store::create_thread(&self.pool, self.provider, None, None, None, "Other");
        let thread = self.runtime.block_on(created).unwrap();
        let bridge = session(
            &self.harness,
            thread,
            self.provider,
            &self.outbound,
            &self.scenario.fixture.native_session,
            &self.scratch,
        );
        (thread, bridge)
    }

    /// Another provider's recording, for the same chat: its frames and what
    /// its bridge is expected to be sent. Nothing is registered yet.
    pub fn recording(&self, name: &str) -> Recording {
        let (fixture, frames) = super::fixture::Fixture::load(name);
        Recording {
            provider: fixture.provider,
            frames,
            expected: fixture.expected_outbound,
            native_session: fixture.native_session,
        }
    }

    /// A recorded session of `recording`'s provider, which the manager's
    /// next spawn of that provider for this chat opens, and one more turn
    /// its bridge is expected to be sent, under any prompt
    /// ([`Self::prompts`]). The bridge is the test's to feed.
    pub fn session_of(&mut self, recording: &Recording) -> Bridge {
        self.expect_turn(recording);
        self.register_session(recording)
    }

    /// Register a recording without relaxing its outbound prompt checks.
    pub fn session_checked(&mut self, recording: &Recording) -> Bridge {
        self.outbound.expect(recording.expected.iter().cloned());
        self.register_session(recording)
    }

    fn register_session(&self, recording: &Recording) -> Bridge {
        session(
            &self.harness,
            self.thread,
            recording.provider,
            &self.outbound,
            &recording.native_session,
            &self.scratch,
        )
    }

    /// One more turn `recording`'s bridge is expected to be sent, under
    /// any prompt.
    pub fn expect_turn(&self, recording: &Recording) {
        self.outbound
            .expect(recording.expected.iter().cloned().map(|mut turn| {
                turn.any_prompt = true;
                turn
            }));
    }

    /// The prompts that went out where any was accepted, in order.
    pub fn prompts(&self) -> Vec<String> {
        self.outbound.prompts()
    }

    /// A student's message under the composer's `provider` and `opts`.
    pub fn say(
        &mut self,
        receipt: &str,
        provider: Provider,
        text: &str,
        opts: &SendOptions,
    ) -> Result<Outcome, String> {
        self.command(Command {
            id: receipt.into(),
            thread_id: Some(self.thread),
            kind: Kind::Send {
                provider,
                text: text.into(),
                options: opts.clone(),
                mode: Default::default(),
            },
        })
    }

    /// The student picks what the chat's next send uses.
    pub fn select(
        &mut self,
        receipt: &str,
        provider: Provider,
        model: Option<&str>,
        effort: Option<&str>,
    ) -> Result<Outcome, String> {
        self.command(Command {
            id: receipt.into(),
            thread_id: Some(self.thread),
            kind: Kind::SelectModel {
                provider,
                model: model.map(String::from),
                effort: effort.map(String::from),
            },
        })
    }

    /// What this runtime's consumer has told so far.
    pub fn envelopes(&self) -> Vec<Value> {
        self.told.lock().unwrap().clone()
    }

    /// Whether the thread's recorded process is still up: a replayed
    /// process records its kill.
    pub fn alive(&self) -> bool {
        self.bridge.handle.is_alive()
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
        self.stop();
        let (runtime, pool, thread) = (&self.runtime, &self.pool, self.thread);
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
        let busy = self.busy();
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
        Replay {
            envelopes: self.envelopes,
            snapshots: self.snapshots,
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

/// A recording joined to a rig's chat ([`Rig::recording`]).
pub(super) struct Recording {
    pub provider: Provider,
    pub frames: Vec<Value>,
    expected: Vec<super::outbound::Expected>,
    native_session: String,
}

impl Recording {
    /// Feed the whole recording to `bridge`.
    pub fn play(&self, bridge: &mut Bridge) {
        for frame in &self.frames {
            (bridge.inbound)(frame);
        }
    }
}

/// What a restart tears down and brings up again: the manager, the consumer
/// on its bus, and a recorded session for the thread's next spawn.
struct Running {
    harness: Arc<Harness>,
    bus: Sender<BusMsg>,
    sink: Sink,
    sinks: Arc<dyn Sinks>,
    own_turns: Arc<Mutex<OwnTurns>>,
    effects: Effects,
    kept: Arc<Mutex<Option<Vec<Effect>>>>,
    told: Arc<Mutex<Vec<Value>>>,
    bridge: Bridge,
    fed: Arc<AtomicBool>,
    clock: Arc<Clock>,
    drained: Sender<Sender<()>>,
    consumer: JoinHandle<Io>,
}

fn boot(
    pool: &SqlitePool,
    database: &std::path::Path,
    scratch: &std::path::Path,
    outbound: &Outbound,
    thread: i64,
    provider: Provider,
    native_session: &str,
) -> Running {
    let harness = Arc::new(Harness::replay(assert_scratch(scratch, scratch)));
    assert_scratch(&crate::harness::thread_cwd(scratch), scratch);
    // The consumer runs beside the replay, as in the app: a send's run is
    // written on it before the provider write.
    let (bus, rx) = std::sync::mpsc::channel::<BusMsg>();
    let sink = crate::harness::app::sink_for(&bus, thread, provider);
    let sinks = crate::harness::app::sinks_for(&bus, thread, provider);
    let own_turns: Arc<Mutex<OwnTurns>> = Arc::default();
    let effects = Effects {
        harness: harness.clone(),
        bus: bus.clone(),
        pools: Pools::Given(pool.clone()),
    };
    let kept: Arc<Mutex<Option<Vec<Effect>>>> = Arc::default();
    let told: Arc<Mutex<Vec<Value>>> = Arc::default();
    let fed = Arc::new(AtomicBool::new(false));
    let (drained, waiters) = std::sync::mpsc::channel();
    let clock = Clock::new();
    let consumer = {
        let (held, fed, clock) = (own_turns.clone(), fed.clone(), clock.clone());
        let mut io = Io {
            pool: pool.clone(),
            database: database.to_path_buf(),
            scratch: scratch.to_path_buf(),
            outbound: outbound.clone(),
            envelopes: Vec::new(),
            told: told.clone(),
            snapshots: Vec::new(),
            runtime: None,
            epoch: std::time::Instant::now(),
            effects: effects.clone(),
            continued: Vec::new(),
            kept: kept.clone(),
            clock: clock.clone(),
        };
        std::thread::spawn(move || {
            io.runtime = Some(Runtime::new().unwrap());
            let feed = Feed {
                rx,
                fed,
                drained: waiters,
                clock,
                waiting: Vec::new(),
            };
            consume(feed, held, &mut io);
            io
        })
    };
    let bridge = session(
        &harness,
        thread,
        provider,
        outbound,
        native_session,
        scratch,
    );
    Running {
        harness,
        bus,
        sink,
        sinks,
        own_turns,
        effects,
        kept,
        told,
        bridge,
        fed,
        clock,
        drained,
        consumer,
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
    // Codex and opencode name their session as it is started or resumed
    // (`start_thread`, `attach_session`), before any turn; a recorded
    // bridge is built already open, so the replay says it as they would.
    let named = matches!(provider, Provider::Codex | Provider::Opencode)
        .then(|| native_session.to_string())
        .filter(|id| !id.is_empty());
    harness.replay_spawn(thread, provider, move |sink| {
        if let Some(provider_session_id) = named {
            sink(HarnessEvent::SessionStarted {
                provider_session_id,
                model: None,
                cwd: String::new(),
            });
        }
        *opened.lock().unwrap() = Some(sink);
        handle
    });
    bridge
}
