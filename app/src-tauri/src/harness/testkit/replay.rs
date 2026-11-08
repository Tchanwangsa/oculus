use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use tokio::runtime::Runtime;

use crate::harness::app::consumer::{consume, ConsumerIo};
use crate::harness::app::send::begin_run;
use crate::harness::app::BusMsg;
use crate::harness::app::Envelope;
use crate::harness::manager::Handle;
use crate::harness::{jobs, store, Harness, Provider, Queue, QueuedMessage, SendOptions, Sink};

use super::{guard::assert_scratch, outbound::Outbound, scenario::Scenario, snapshot};

pub(in crate::harness) struct Bridge {
    pub handle: Handle,
    pub inbound: Box<dyn FnMut(&Value)>,
    /// The transport closing, through the bridge's production EOF path.
    pub eof: Box<dyn FnMut()>,
}

pub(super) struct Replay {
    pub envelopes: Vec<Value>,
    pub snapshots: Vec<Vec<Value>>,
    pub items: Vec<Value>,
    pub reopened: Vec<Value>,
    pub outbound: Vec<Value>,
    pub busy: bool,
    pub schema: Vec<String>,
    /// Run, attempt, binding and receipt rows for the thread (`snapshot::ledger`).
    pub ledger: Value,
    pub repeat_envelope: bool,
    /// The app restarted after the stream (startup reconciliation ran).
    pub restarted: bool,
    pub load_race: bool,
    /// Continuations the consumer dispatched (`Scenario::continuation`).
    pub continuations: Vec<SendOptions>,
}

struct Io {
    pool: SqlitePool,
    database: std::path::PathBuf,
    scratch: std::path::PathBuf,
    outbound: Outbound,
    envelopes: Vec<Value>,
    snapshots: Vec<Vec<Value>>,
    runtime: Option<Runtime>,
    epoch: std::time::Instant,
    /// A continuation's dispatch, which the replay starts itself.
    continued: std::sync::mpsc::Sender<(QueuedMessage, SendOptions)>,
}

/// The bus until the replay has fed everything: the bridges keep sinks alive,
/// so the channel itself never closes.
fn until_fed(rx: Receiver<BusMsg>, fed: Arc<AtomicBool>) -> impl Iterator<Item = BusMsg> {
    std::iter::from_fn(move || loop {
        match rx.recv_timeout(std::time::Duration::from_millis(5)) {
            Ok(msg) => return Some(msg),
            Err(RecvTimeoutError::Timeout) if fed.load(Ordering::SeqCst) => {
                return rx.try_recv().ok();
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return None,
        }
    })
}

impl ConsumerIo for Io {
    fn clock_time(&self) -> Option<std::time::Instant> {
        Some(self.epoch + std::time::Duration::from_millis(self.envelopes.len() as u64 * 10))
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

pub(super) fn run(scenario: Scenario) -> Replay {
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
    let events = bus.clone();
    let sink: Sink = Arc::new(move |ev| {
        let _ = events.send(BusMsg::Event(thread, provider, ev));
    });
    let queue = Arc::new(Mutex::new(Queue::default()));
    let fed = Arc::new(AtomicBool::new(false));
    let (continued, dispatched) = std::sync::mpsc::channel();
    let consumer = {
        let (queue, fed) = (queue.clone(), fed.clone());
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
        };
        std::thread::spawn(move || {
            io.runtime = Some(Runtime::new().unwrap());
            consume(until_fed(rx, fed), queue, &mut io);
            io
        })
    };
    let mut bridge = match provider {
        Provider::Claude => crate::harness::claude::replay::bridge(outbound.wire(), sink.clone()),
        Provider::Codex => crate::harness::codex::replay::bridge(
            outbound.clone(),
            sink.clone(),
            &scenario.fixture.native_session,
            &scratch,
        ),
        Provider::Opencode => crate::harness::opencode::replay::bridge(
            outbound.clone(),
            sink.clone(),
            &scenario.fixture.native_session,
            &scratch,
        ),
        Provider::Antigravity => {
            crate::harness::antigravity::replay::bridge(outbound.wire(), sink.clone())
        }
    };
    harness.replay_handle(thread, bridge.handle.clone());
    let mut continuations = Vec::new();
    if let Some(card) = &card {
        // The card's click settled it: the consumer claims the idle thread
        // and dispatches, and the replay runs the production start steps.
        bus.send(BusMsg::Event(
            thread,
            provider,
            crate::harness::HarnessEvent::CardSettled {
                proposal: card.clone(),
            },
        ))
        .unwrap();
        let (msg, opts) = dispatched
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the settled card dispatched its continuation");
        let started = runtime.block_on(crate::harness::app::send::begin_turn(
            &pool,
            &bus,
            store::SendIntent {
                thread_id: thread,
                provider,
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
        continuations.push(opts);
        harness
            .send(
                thread,
                provider,
                None,
                &SendOptions::default(),
                &prompt,
                sink.clone(),
            )
            .unwrap();
        runtime.block_on(store::mark_sent(&pool, &run)).unwrap();
        runtime
            .block_on(crate::harness::app::proposal_context::accepted(
                &pool, cards,
            ))
            .unwrap();
    }
    let student = card.is_none();
    assert!(!student || queue.lock().unwrap().try_claim(thread));
    if scenario.fail_intent {
        runtime.block_on(sqlx::raw_sql("CREATE TRIGGER replay_intent_failure BEFORE INSERT ON harness_items WHEN NEW.kind = 'user' BEGIN SELECT RAISE(FAIL, 'replay intent failure'); END;").execute(&pool)).unwrap();
    }
    if scenario.fail_write {
        runtime.block_on(sqlx::raw_sql("CREATE TRIGGER replay_write_failure BEFORE INSERT ON harness_items WHEN NEW.kind = 'assistant' BEGIN SELECT RAISE(FAIL, 'replay storage failure'); END;").execute(&pool)).unwrap();
    }
    let begun = if !student {
        Err("a continuation, not a send".to_string())
    } else {
        runtime.block_on(begin_run(
            &bus,
            store::SendIntent {
                thread_id: thread,
                provider,
                model: None,
                native_session: None,
                receipt: "replay-send".into(),
                text: "Replay request".into(),
                at: None,
                continuation: None,
            },
        ))
    };
    let sent = if !student {
        Ok(())
    } else {
        begun.and_then(|run| {
            harness.send(
                thread,
                provider,
                None,
                &SendOptions::default(),
                "Replay request",
                sink.clone(),
            )?;
            runtime.block_on(store::mark_sent(&pool, &run))
        })
    };
    if let Err(error) = &sent {
        crate::harness::child::fail_turn(&sink, provider, error.clone());
    }
    for frame in &scenario.frames {
        (bridge.inbound)(frame);
    }
    // The same card settling again (a second view's event) starts nothing.
    if let Some(card) = &card {
        bus.send(BusMsg::Event(
            thread,
            provider,
            crate::harness::HarnessEvent::CardSettled {
                proposal: card.clone(),
            },
        ))
        .unwrap();
    }
    // A recording ends where its transport closed; a restart never sees it.
    if !scenario.restart {
        (bridge.eof)();
    }
    fed.store(true, Ordering::SeqCst);
    let io = consumer.join().unwrap();
    if scenario.restart {
        runtime.block_on(store::reconcile(&pool)).unwrap();
    }
    let items = runtime.block_on(snapshot::items(&pool, thread));
    let schema = runtime
        .block_on(
            sqlx::query_scalar::<_, String>("SELECT name FROM sqlite_master WHERE type = 'table'")
                .fetch_all(&pool),
        )
        .unwrap();
    let ledger = runtime.block_on(snapshot::ledger(&pool, thread));
    let busy = queue.lock().unwrap().is_busy(thread);
    runtime.block_on(pool.close());
    let reopened_pool = runtime
        .block_on(crate::db::store::pool(&assert_scratch(&database, &scratch)))
        .unwrap();
    let reopened = runtime.block_on(snapshot::items(&reopened_pool, thread));
    runtime.block_on(reopened_pool.close());
    harness.shutdown();
    drop(bridge);
    Replay {
        envelopes: io.envelopes,
        snapshots: io.snapshots,
        items,
        reopened,
        // A send refused before its write leaves the recorded write unused.
        outbound: outbound.finish(!scenario.fail_intent),
        busy,
        schema,
        ledger,
        repeat_envelope: scenario.repeat_envelope,
        restarted: scenario.restart,
        load_race: scenario.load_race,
        continuations: {
            continuations.extend(dispatched.try_iter().map(|(_, opts)| opts));
            continuations
        },
    }
}

#[test]
#[should_panic(expected = "unrecorded provider spawn in replay")]
fn replay_manager_never_spawns_an_unrecorded_handle() {
    let scratch = super::guard::scratch("turn-no-spawn");
    let harness = Harness::replay(assert_scratch(&scratch, &scratch));
    let _ = harness.send(
        1,
        Provider::Claude,
        None,
        &SendOptions::default(),
        "unexpected",
        Arc::new(|_| {}),
    );
}
