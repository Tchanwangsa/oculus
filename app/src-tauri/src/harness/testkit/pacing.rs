//! Work the consumer does on a clock, driven by a scripted one: streamed
//! text reaching the draft table at most once per interval, and a failed
//! settle tried again at growing intervals.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sqlx::SqlitePool;

use crate::harness::app::consumer::{consume, ConsumerIo};
use crate::harness::app::{BusMsg, Envelope};
use crate::harness::event::Source;
use crate::harness::{jobs, store, HarnessEvent, Provider, Queue, QueuedMessage, SendOptions};

/// A consumer whose clock the test sets before each message, recording what
/// the draft table held as each envelope went out.
struct Timed {
    pool: SqlitePool,
    now: Arc<Mutex<Instant>>,
    wakes: Vec<Instant>,
    /// (event type, the stored draft at that moment).
    seen: Vec<(String, Option<String>)>,
    /// (event type, when it went out).
    told: Vec<(String, Instant)>,
    runtime: tokio::runtime::Runtime,
}

impl ConsumerIo for Timed {
    fn now(&self) -> Instant {
        *self.now.lock().unwrap()
    }
    fn wake_at(&mut self, at: Instant) {
        self.wakes.push(at);
    }
    fn open_pool(&mut self, _: &tokio::runtime::Runtime) -> Option<SqlitePool> {
        Some(self.pool.clone())
    }
    fn name(&mut self, _: i64, _: Provider, _: Vec<jobs::JobSelection>, _: store::NamingSeed) {}
    fn dispatch(&mut self, _: i64, _: Provider, _: QueuedMessage, _: SendOptions) {}
    fn emit(&mut self, envelope: Envelope) {
        let kind = serde_json::to_value(&envelope.event).unwrap()["type"]
            .as_str()
            .unwrap()
            .to_string();
        self.told.push((kind.clone(), self.now()));
        self.seen.push((kind, draft(&self.runtime, &self.pool)));
    }
}

fn draft(runtime: &tokio::runtime::Runtime, pool: &SqlitePool) -> Option<String> {
    runtime
        .block_on(sqlx::query_scalar("SELECT text FROM harness_stream_drafts").fetch_optional(pool))
        .unwrap()
}

#[test]
fn streamed_text_reaches_the_draft_once_per_interval_and_leaves_with_its_message() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (_dir, pool) = runtime.block_on(super::runs::scratch_pool("turn-drafts"));
    let thread = runtime
        .block_on(store::create_thread(
            &pool,
            Provider::Codex,
            None,
            None,
            None,
            "q",
        ))
        .unwrap();
    let event = |event| BusMsg::Event {
        thread_id: thread,
        provider: Provider::Codex,
        generation: Some(1),
        event,
    };
    let delta = |item: &str, text: &str| {
        event(HarnessEvent::AssistantDelta {
            text: text.into(),
            item: Some(item.into()),
        })
    };
    let (reply, _begun) = tokio::sync::oneshot::channel();
    // Milliseconds since the start, and what arrives then.
    let script = vec![
        (
            0,
            BusMsg::Begin {
                intent: super::runs::intent(thread, "drafts", None),
                reply,
            },
        ),
        (0, delta("m1", "He")),
        (10, delta("m1", "llo")),
        (20, delta("m2", " and")),
        (60, BusMsg::Wake),
        (61, delta("m2", " more")),
        (
            70,
            event(HarnessEvent::AssistantMessage {
                text: "Hello".into(),
                source: Source::of("m1"),
            }),
        ),
    ];
    let start = Instant::now();
    let now = Arc::new(Mutex::new(start));
    let clock = now.clone();
    let bus = script.into_iter().map(move |(at, msg)| {
        *clock.lock().unwrap() = start + Duration::from_millis(at);
        msg
    });
    let mut io = Timed {
        pool: pool.clone(),
        now,
        wakes: Vec::new(),
        seen: Vec::new(),
        told: Vec::new(),
        runtime: tokio::runtime::Runtime::new().unwrap(),
    };
    consume(bus, Arc::new(Mutex::new(Queue::default())), &mut io);

    let drafts: Vec<_> = io
        .seen
        .iter()
        .filter(|(kind, _)| kind == "assistant_delta" || kind == "assistant_message")
        .map(|(_, draft)| draft.as_deref())
        .collect();
    assert_eq!(
        drafts,
        vec![
            Some("He"),        // the first delta is written at once
            Some("He"),        // 10 ms later: held
            Some("He"),        // 20 ms: held, another message's text
            Some("Hello and"), // the wake at 60 ms wrote both; 61 ms is held
            Some(" and more"), // m1 became a row: only m2's text is a draft
        ]
    );
    assert_eq!(
        io.wakes.first(),
        Some(&(start + Duration::from_millis(50))),
        "the consumer asked to be woken when the interval ends"
    );
    assert_eq!(
        draft(&runtime, &pool).as_deref(),
        Some(" and more"),
        "what was still waiting is written when the bus closes"
    );
    runtime.block_on(pool.close());
}

/// A settle the database refuses is tried again after 100 ms, then 200 ms,
/// and so on, and its terminal is told to nobody until the write lands.
#[test]
fn a_failed_settle_is_retried_at_growing_intervals_until_it_lands() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (_dir, pool) = runtime.block_on(super::runs::scratch_pool("turn-settle-retry"));
    let thread = runtime
        .block_on(store::create_thread(
            &pool,
            Provider::Codex,
            None,
            None,
            None,
            "q",
        ))
        .unwrap();
    let (reply, _begun) = tokio::sync::oneshot::channel();
    let ended = BusMsg::Event {
        thread_id: thread,
        provider: Provider::Codex,
        generation: Some(1),
        event: HarnessEvent::TurnFinished {
            status: "completed".into(),
            turn: Some("t".into()),
        },
    };
    // Milliseconds since the start, whether the database then refuses
    // writes, and what arrives.
    let script = vec![
        (
            0,
            false,
            BusMsg::Begin {
                intent: super::runs::intent(thread, "retry", None),
                reply,
            },
        ),
        (
            0,
            false,
            BusMsg::Bound {
                thread_id: thread,
                generation: 1,
                turn: "t".into(),
            },
        ),
        (0, true, ended),
        (50, true, BusMsg::Wake),
        (100, true, BusMsg::Wake),
        (300, false, BusMsg::Wake),
    ];
    let start = Instant::now();
    let now = Arc::new(Mutex::new(start));
    let (clock, pragmas, db) = (
        now.clone(),
        tokio::runtime::Runtime::new().unwrap(),
        pool.clone(),
    );
    let bus = script.into_iter().map(move |(at, refuses, msg)| {
        *clock.lock().unwrap() = start + Duration::from_millis(at);
        let pragma = match refuses {
            true => "PRAGMA query_only = ON",
            false => "PRAGMA query_only = OFF",
        };
        pragmas.block_on(sqlx::query(pragma).execute(&db)).unwrap();
        msg
    });
    let queue = Arc::new(Mutex::new(Queue::default()));
    assert!(queue.lock().unwrap().try_claim(thread));
    let mut io = Timed {
        pool: pool.clone(),
        now,
        wakes: Vec::new(),
        seen: Vec::new(),
        told: Vec::new(),
        runtime: tokio::runtime::Runtime::new().unwrap(),
    };
    consume(bus, queue.clone(), &mut io);

    let at = |ms| start + Duration::from_millis(ms);
    assert_eq!(
        io.wakes,
        vec![at(100), at(100), at(300)],
        "first after 100 ms, asked again by the early wake, then 200 ms more"
    );
    let told: Vec<_> = io
        .told
        .iter()
        .map(|(kind, when)| (kind.as_str(), *when))
        .collect();
    assert_eq!(
        told,
        vec![
            ("user_message", at(0)),
            // No answer arrived in this script, which settlement says.
            ("delivery_notice", at(300)),
            ("run_settled", at(300)),
            ("turn_finished", at(300)),
        ]
    );
    let status: String = runtime
        .block_on(sqlx::query_scalar("SELECT execution_status FROM harness_runs").fetch_one(&pool))
        .unwrap();
    assert_eq!(status, "completed");
    assert!(!queue.lock().unwrap().is_busy(thread));
    runtime.block_on(pool.close());
}
