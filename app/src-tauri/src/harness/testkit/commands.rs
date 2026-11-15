//! The command path (`harness/core/`): an id gives one result however often
//! it is sent, a refusal records nothing, a rewind holds its chat while the
//! provider is asked, and every envelope says the revision its change was
//! saved at.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use serde_json::Value;
use sqlx::SqlitePool;
use tokio::sync::oneshot::Receiver;

use crate::harness::app::consumer::{consume, ConsumerIo};
use crate::harness::app::{BusMsg, Envelope};
use crate::harness::core::decide::Answer;
use crate::harness::core::effect::{Effect, Start};
use crate::harness::core::{load, Command, Kind, Outcome};
use crate::harness::{jobs, store, HarnessEvent, Provider, Release, SendOptions};

use super::io::{Clock, Feed};
use super::replay::run;
use super::rig::Rig;
use super::scenario::Scenario;

pub(super) type Result = std::result::Result<Outcome, String>;

/// A consumer on a scratch database, alive across the test's scripts, whose
/// effects are answered by the test: a start sends nothing and reports
/// nothing, so its run stays `starting` until the script ends its turn, and
/// a rewind's answer arrives after whatever the script still holds, as a
/// slow provider's would.
pub(super) struct Bench {
    pub(super) runtime: tokio::runtime::Runtime,
    _dir: crate::test_support::Scratch,
    pub(super) pool: SqlitePool,
    pub(super) thread: i64,
    bus: Sender<BusMsg>,
    record: Arc<Mutex<Record>>,
    /// Whether the provider says its context went back, per rewind asked.
    rewinds: Arc<AtomicBool>,
    fed: Arc<AtomicBool>,
    drained: Sender<Sender<()>>,
    consumer: Option<JoinHandle<()>>,
}

/// What the consumer told and asked for.
#[derive(Default)]
struct Record {
    events: Vec<Value>,
    effects: Vec<String>,
    starts: Vec<Start>,
}

struct Io {
    pool: SqlitePool,
    bus: Sender<BusMsg>,
    record: Arc<Mutex<Record>>,
    rewinds: Arc<AtomicBool>,
}

impl ConsumerIo for Io {
    fn open_pool(&mut self, _: &tokio::runtime::Runtime) -> Option<SqlitePool> {
        Some(self.pool.clone())
    }
    fn name(&mut self, _: i64, _: Provider, _: Vec<jobs::JobSelection>, _: store::NamingSeed) {}
    fn perform(&mut self, effect: Effect) {
        let mut record = self.record.lock().unwrap();
        record.effects.push(match &effect {
            Effect::Start(start) => format!("start {}", start.text),
            Effect::Rewind { ask, .. } => format!("rewind {}", ask.anchor),
            Effect::Interrupt { .. } => "interrupt".to_string(),
            Effect::ForceStop { .. } => "force stop".to_string(),
            Effect::Release { reason, .. } => format!("release {reason:?}"),
        });
        match effect {
            Effect::Rewind { command, ask } => {
                let answer = Answer::Rewound {
                    context: self.rewinds.load(Ordering::SeqCst),
                };
                let _ = self.bus.send(BusMsg::Answer {
                    thread_id: ask.thread_id,
                    command,
                    answer,
                });
            }
            Effect::Start(start) => record.starts.push(*start),
            _ => {}
        }
    }
    fn emit(&mut self, envelope: Envelope) {
        let mut record = self.record.lock().unwrap();
        record.events.push(serde_json::to_value(envelope).unwrap());
    }
}

impl Drop for Bench {
    fn drop(&mut self) {
        self.fed.store(true, Ordering::SeqCst);
        if let Some(consumer) = self.consumer.take() {
            let _ = consumer.join();
        }
    }
}

impl Bench {
    pub(super) fn new(name: &str) -> Self {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (dir, pool) = runtime.block_on(super::runs::scratch_pool(name));
        let thread = runtime
            .block_on(store::create_thread(
                &pool,
                Provider::Claude,
                None,
                None,
                None,
                "q",
            ))
            .unwrap();
        let (bus, rx) = std::sync::mpsc::channel();
        let (drained, waiters) = std::sync::mpsc::channel();
        let record: Arc<Mutex<Record>> = Arc::default();
        let rewinds = Arc::new(AtomicBool::new(true));
        let fed = Arc::new(AtomicBool::new(false));
        let mut io = Io {
            pool: pool.clone(),
            bus: bus.clone(),
            record: record.clone(),
            rewinds: rewinds.clone(),
        };
        let feed = Feed {
            rx,
            fed: fed.clone(),
            drained: waiters,
            clock: Clock::new(),
            waiting: Vec::new(),
        };
        let consumer = std::thread::spawn(move || consume(feed, Arc::default(), &mut io));
        Bench {
            runtime,
            _dir: dir,
            pool,
            thread,
            bus,
            record,
            rewinds,
            fed,
            drained,
            consumer: Some(consumer),
        }
    }

    /// Put `script` on the bus and wait until the consumer has applied it,
    /// and every answer its effects produced.
    pub(super) fn play(&mut self, script: Vec<BusMsg>) {
        for message in script {
            self.bus.send(message).unwrap();
        }
        let (done, waiting) = std::sync::mpsc::channel();
        self.drained.send(done).unwrap();
        waiting
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the consumer caught up");
    }

    /// Whether the provider will say its context went back with a rewind.
    pub(super) fn rewinds(&self, context: bool) {
        self.rewinds.store(context, Ordering::SeqCst);
    }

    /// The envelopes told so far.
    pub(super) fn events(&self) -> Vec<Value> {
        self.record.lock().unwrap().events.clone()
    }

    /// The effects asked for so far, by name.
    pub(super) fn effects(&self) -> Vec<String> {
        self.record.lock().unwrap().effects.clone()
    }

    /// The runs handed to the start step so far.
    pub(super) fn starts(&self) -> Vec<Start> {
        self.record.lock().unwrap().starts.clone()
    }

    /// Forget what was told and asked for.
    pub(super) fn clear(&self) {
        *self.record.lock().unwrap() = Record::default();
    }

    /// A run of the thread's is going out or being answered.
    pub(super) fn open(&self) -> bool {
        let open = store::queue::open(&self.pool, self.thread);
        self.runtime.block_on(open).unwrap().is_some()
    }

    pub(super) fn command(&self, id: &str, kind: Kind) -> (BusMsg, Receiver<Result>) {
        command(id, Some(self.thread), kind)
    }

    pub(super) fn send(&self, id: &str, text: &str) -> (BusMsg, Receiver<Result>) {
        self.command(id, send(text))
    }

    /// A whole turn: the question `text` under the command `id`, the
    /// session naming itself and the question's anchor, an answer, the end.
    pub(super) fn turn(&self, id: &str, text: &str, anchor: &str) -> Vec<BusMsg> {
        let said = HarnessEvent::AssistantMessage {
            text: format!("answer to {text}"),
            source: None,
        };
        let session = HarnessEvent::SessionStarted {
            provider_session_id: "native-1".into(),
            model: None,
            cwd: String::new(),
        };
        let anchor = HarnessEvent::TurnAnchor {
            anchor: anchor.into(),
        };
        vec![
            self.send(id, text).0,
            self.session(session),
            self.session(anchor),
            self.session(said),
            self.app(ended("completed")),
        ]
    }

    pub(super) fn session(&self, event: HarnessEvent) -> BusMsg {
        BusMsg::Event {
            thread_id: self.thread,
            provider: Provider::Claude,
            generation: Some(1),
            event,
        }
    }

    pub(super) fn app(&self, event: HarnessEvent) -> BusMsg {
        BusMsg::app(self.thread, Provider::Claude, event)
    }

    pub(super) fn got(&self, answer: Receiver<Result>) -> Result {
        self.runtime.block_on(answer).unwrap()
    }

    pub(super) fn count(&self, sql: &str) -> i64 {
        self.runtime
            .block_on(sqlx::query_scalar(sql).fetch_one(&self.pool))
            .unwrap()
    }

    pub(super) fn questions(&self) -> Vec<String> {
        self.runtime
            .block_on(
                sqlx::query_scalar(
                    "SELECT content FROM harness_items WHERE kind = 'user' ORDER BY id",
                )
                .fetch_all(&self.pool),
            )
            .unwrap()
    }

    pub(super) fn kinds(&self) -> Vec<String> {
        self.events()
            .iter()
            .map(|e| e["event"]["type"].as_str().unwrap().to_string())
            .collect()
    }
}

pub(super) fn command(id: &str, thread_id: Option<i64>, kind: Kind) -> (BusMsg, Receiver<Result>) {
    let (reply, answer) = tokio::sync::oneshot::channel();
    let command = Command {
        id: id.into(),
        thread_id,
        kind,
    };
    (BusMsg::Command { command, reply }, answer)
}

pub(super) fn send(text: &str) -> Kind {
    Kind::Send {
        provider: Provider::Claude,
        text: text.into(),
        options: SendOptions::default(),
        mode: Default::default(),
    }
}

pub(super) fn ended(status: &str) -> HarnessEvent {
    HarnessEvent::TurnFinished {
        status: status.into(),
        turn: None,
    }
}

/// A retried send is the same send: one run, one provider write, and the
/// answer the first call got.
#[test]
fn the_same_command_id_twice_gives_one_result() {
    let mut rig = Rig::start(Scenario::recorded("codex-ls"));
    rig.open();
    let first: (i64, i64) = rig
        .runtime
        .block_on(
            sqlx::query_as("SELECT id, initiating_item_id FROM harness_runs").fetch_one(&rig.pool),
        )
        .unwrap();
    // The recording holds one `turn/start`: a second write would fail it.
    let again = rig.send("replay-send").unwrap();
    assert_eq!((again.run_id, again.item_id), first);
    rig.resume();
    rig.close();
    let replay = rig.finish();
    assert_eq!(replay.ledger["runs"].as_array().unwrap().len(), 1);
    let questions = replay.items.iter().filter(|i| i["kind"] == "user").count();
    assert_eq!(questions, 1);
    assert_eq!(replay.ledger["runs"][0]["execution_status"], "completed");
}

#[test]
fn a_command_id_from_another_chat_is_refused() {
    let mut bench = Bench::new("command-other-chat");
    let other = bench
        .runtime
        .block_on(store::create_thread(
            &bench.pool,
            Provider::Claude,
            None,
            None,
            None,
            "other",
        ))
        .unwrap();
    let (first, sent) = bench.send("c1", "hello");
    let (stolen, refused) = command("c1", Some(other), send("hello"));
    bench.play(vec![first, stolen]);
    assert!(bench.got(sent).is_ok());
    let refused = bench.got(refused).unwrap_err();
    assert!(refused.contains("another thread"), "{refused}");
    assert_eq!(bench.count("SELECT COUNT(*) FROM harness_runs"), 1);
    assert_eq!(bench.effects(), ["start hello"]);
}

/// The receipt is read before the thread is made, so a retried first
/// message makes one chat.
#[test]
fn a_repeated_first_message_makes_one_chat() {
    let mut bench = Bench::new("command-first-message");
    let (first, made) = command("new-1", None, send("hello"));
    let (again, repeated) = command("new-1", None, send("hello"));
    bench.play(vec![first, again]);
    let made = bench.got(made).unwrap();
    assert_ne!(made.thread_id, bench.thread, "a chat of its own");
    assert_eq!(bench.got(repeated).unwrap(), made);
    assert_eq!(bench.count("SELECT COUNT(*) FROM harness_threads"), 2);
    assert_eq!(bench.count("SELECT COUNT(*) FROM harness_runs"), 1);
    assert_eq!(bench.effects(), ["start hello"], "sent once");
    let receipt: (i64, String) = bench
        .runtime
        .block_on(
            sqlx::query_as("SELECT thread_id, command FROM harness_receipts WHERE id = 'new-1'")
                .fetch_one(&bench.pool),
        )
        .unwrap();
    assert_eq!(receipt, (made.thread_id, "send".to_string()));
}

/// A rewind waits on the provider with its chat held: a message sent in
/// that window waits its turn, and is not among the rows the rewind takes.
#[test]
fn a_message_sent_while_the_provider_rewinds_waits_and_survives_it() {
    let mut bench = Bench::new("command-rewind-race");
    let mut script = bench.turn("c1", "first", "anchor-1");
    script.extend(bench.turn("c2", "second", "anchor-2"));
    bench.play(script);
    assert_eq!(bench.questions(), ["first", "second"]);
    let second: i64 = bench.count("SELECT MAX(id) FROM harness_items WHERE kind = 'user'");

    bench.clear();
    let (rewind, rewound) = bench.command("c3", Kind::Rewind { item_id: second });
    let (typed, sent) = bench.send("c4", "meanwhile");
    bench.play(vec![rewind, typed]);

    assert_eq!(bench.got(rewound).unwrap().text.as_deref(), Some("second"));
    let waited = bench.got(sent).unwrap();
    assert!(waited.run.is_none() && waited.queued.is_some(), "it waited");
    assert_eq!(bench.effects(), ["rewind anchor-2", "start meanwhile"]);
    assert_eq!(bench.questions(), ["first", "meanwhile"]);
    assert_eq!(
        bench.kinds(),
        ["queue", "rewound", "queue", "user_message", "run_selected"],
        "the rows went before the waiting message's question was written"
    );
    let rewound = &bench.events()[1];
    assert_eq!(rewound["event"]["context"], true);
    assert_eq!(rewound["event"]["from_item_id"], second);
}

/// A refusal the student can lift is not a result: nothing is recorded, so
/// the same id works once the turn is over.
#[test]
fn a_rewind_on_a_busy_chat_is_refused_and_its_id_stays_free() {
    let mut bench = Bench::new("command-refusal");
    let (first, _) = bench.send("c1", "first");
    let question = Kind::Rewind { item_id: 1 };
    let (early, refused) = bench.command("c2", question);
    bench.play(vec![first, early]);
    assert_eq!(
        bench.got(refused).unwrap_err(),
        "stop the current turn before rewinding"
    );
    assert_eq!(
        bench.count("SELECT COUNT(*) FROM harness_receipts WHERE id = 'c2'"),
        0
    );
    assert_eq!(bench.questions(), ["first"]);

    let (later, rewound) = bench.command("c2", Kind::Rewind { item_id: 1 });
    bench.play(vec![bench.app(ended("completed")), later]);
    assert_eq!(bench.got(rewound).unwrap().text.as_deref(), Some("first"));
    assert!(bench.questions().is_empty());
    // The question had no anchor, so the provider was never asked.
    assert_eq!(bench.effects(), ["start first"]);
    let events = bench.events();
    let rewound = events.last().unwrap();
    assert_eq!(rewound["event"]["type"], "rewound");
    assert_eq!(rewound["event"]["context"], false);
}

/// An edit takes the question's rows and asks the new text as one commit,
/// under the command's own id; a provider that kept its context is said so.
#[test]
fn an_edit_rewinds_and_sends_in_one_commit() {
    let mut bench = Bench::new("command-edit");
    bench.rewinds(false);
    bench.play(bench.turn("c1", "first", "anchor-1"));
    let question: i64 = bench.count("SELECT MAX(id) FROM harness_items WHERE kind = 'user'");
    bench.clear();

    let edit = || Kind::EditResend {
        item_id: question,
        text: "again".into(),
        options: SendOptions::default(),
        provider: None,
    };
    let (edited, sent) = bench.command("c2", edit());
    bench.play(vec![edited]);
    let (repeat, same) = bench.command("c2", edit());
    bench.play(vec![repeat]);

    let run = bench.got(sent).unwrap().run.expect("the edit's run");
    assert_eq!(
        bench.got(same).unwrap().run,
        Some(run),
        "a retry sends nothing"
    );
    assert_eq!(bench.effects(), ["rewind anchor-1", "start again"]);
    assert_eq!(bench.questions(), ["again"]);
    assert_eq!(bench.kinds(), ["rewound", "user_message", "run_selected"]);
    assert_eq!(bench.events()[0]["event"]["context"], false);
    assert_eq!(bench.count("SELECT COUNT(*) FROM harness_runs"), 1);
    let receipt: i64 = bench.count("SELECT run_id FROM harness_command_receipts WHERE id = 'c2'");
    assert_eq!(receipt, run.run_id, "the run's receipt is the command's id");
    assert!(bench.open(), "the edit's run has the chat");
}

/// Deleting is a command too: its receipt outlives the rows, and whatever
/// the dying session still says writes nothing.
#[test]
fn deleting_a_chat_twice_under_one_id_answers_the_same_and_its_session_is_heard_no_more() {
    let mut bench = Bench::new("command-delete");
    let (first, _) = bench.send("c1", "first");
    let (delete, deleted) = bench.command("c2", Kind::DeleteThread);
    let late = bench.session(HarnessEvent::AssistantMessage {
        text: "late".into(),
        source: None,
    });
    let (again, repeated) = bench.command("c2", Kind::DeleteThread);
    let session = bench.session(HarnessEvent::TurnStarted { turn: None });
    bench.play(vec![first, session, delete, late, again]);

    let deleted = bench.got(deleted).unwrap();
    assert_eq!(deleted.thread_id, bench.thread);
    assert_eq!(bench.got(repeated).unwrap(), deleted);
    assert_eq!(bench.effects(), ["start first", "release Deleted"]);
    for table in ["harness_threads", "harness_items", "harness_runs"] {
        assert_eq!(
            bench.count(&format!("SELECT COUNT(*) FROM {table}")),
            0,
            "{table}"
        );
    }
    let receipts: Vec<String> = bench
        .runtime
        .block_on(sqlx::query_scalar("SELECT id FROM harness_receipts").fetch_all(&bench.pool))
        .unwrap();
    assert_eq!(receipts, ["c2"], "the thread's other receipts went with it");
    assert!(!bench.open());
    // `Release::Deleted` reaching the consumer afterwards changes nothing.
    let released = BusMsg::Released {
        thread_id: bench.thread,
        provider: Provider::Claude,
        generation: 1,
        reason: Release::Deleted,
    };
    bench.play(vec![released]);
    assert_eq!(bench.count("SELECT COUNT(*) FROM harness_items"), 0);
}

/// A reload reads rows and revision together, and every envelope carries
/// the revision of the thread once its change was saved: what a snapshot at
/// revision N holds is exactly what the envelopes up to N told.
#[test]
fn every_envelope_carries_a_revision_a_reload_can_compare() {
    for name in ["claude-ls", "codex-ls", "opencode-v1-ls", "antigravity-ls"] {
        let scenario = Scenario::recorded(name);
        let mut rig = Rig::start(scenario);
        rig.open();
        rig.resume();
        rig.drain();
        let snapshot = rig
            .runtime
            .block_on(load::read(&rig.pool, rig.thread))
            .unwrap();
        rig.close();
        let replay = rig.finish();

        let revisions: Vec<i64> = replay
            .envelopes
            .iter()
            .filter(|e| e["threadId"] == replay.items[0]["thread_id"])
            .map(|e| e["revision"].as_i64().unwrap())
            .collect();
        assert!(
            revisions.windows(2).all(|w| w[0] <= w[1]),
            "{name}: {revisions:?}"
        );
        assert!(revisions[0] >= 1, "{name}: the question's commit raised it");
        // Every row the stream told of is in the snapshot by its revision.
        for envelope in &replay.envelopes {
            let (Some(item), Some(revision)) =
                (envelope["itemId"].as_i64(), envelope["revision"].as_i64())
            else {
                continue;
            };
            if revision <= snapshot.revision {
                assert!(
                    snapshot.items.iter().any(|i| i.id == item),
                    "{name}: row {item} at revision {revision} is missing from the snapshot"
                );
            }
        }
        assert_eq!(snapshot.runs.len(), 1, "{name}");
        assert_eq!(snapshot.items.len(), replay.items.len(), "{name}");
    }
}

/// The run's question is written by its commit, on the consumer, before the
/// provider hears of it: with the write refused, nothing is sent and the
/// chat says why.
#[test]
fn a_send_that_cannot_be_saved_is_an_error_row_and_no_provider_write() {
    let mut scenario = Scenario::recorded("codex-ls");
    scenario.fail_intent = true;
    let replay = run(scenario);
    assert!(replay.outbound.is_empty());
    let kinds: Vec<_> = replay
        .envelopes
        .iter()
        .map(|e| e["event"]["type"].as_str().unwrap())
        .collect();
    assert_eq!(&kinds[..2], ["error", "turn_finished"], "{kinds:?}");
    assert!(replay.ledger["runs"].as_array().unwrap().is_empty());
    assert!(!replay.busy);
}
