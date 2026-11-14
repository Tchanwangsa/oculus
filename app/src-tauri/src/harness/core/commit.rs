//! The one transaction a command gets: its receipt, every change its
//! decider asked for, the thread's raised revision and the stored result.
//! All of it lands or none of it does, and nobody is told until it has.
//! Nothing else writes for a command.

use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::harness::store::{self, RunStart, SendIntent};
use crate::harness::{Provider, SendOptions};

use super::command::Outcome;
use super::decide::Plan;
use super::receipts;

/// One change a decider asks for.
pub(in crate::harness) enum Change {
    /// A new chat for the send that follows.
    CreateThread {
        provider: Provider,
        /// The send's options: the model, the subject, the lecture, the video.
        options: SendOptions,
        first_message: String,
    },
    /// The run of a message going out now: its question row, attempt,
    /// binding and run receipt (`store::begin`).
    Begin {
        provider: Provider,
        text: String,
        options: SendOptions,
        /// The run receipt's id: the command's own for a send, a fresh one
        /// for a run the app starts.
        receipt: String,
        /// The waiting run that starts, when the message waited.
        queued: Option<i64>,
    },
    /// A message, or a card's continuation, joins the back of the queue as
    /// a waiting run (`store/queue.rs`).
    Enqueue {
        provider: Provider,
        text: String,
        options: SendOptions,
    },
    EditQueued {
        run_id: i64,
        text: String,
    },
    /// A waiting run leaves the queue: cancelled, or `failed` when it could
    /// not be started.
    CloseQueued {
        run_id: i64,
        failed: bool,
    },
    ReorderQueued {
        order: Vec<i64>,
    },
    /// Every waiting run is cancelled; a continuation's card says it was
    /// stopped.
    ClearQueue,
    /// The rows from `item_id` on leave the chat. `context` is whether the
    /// provider forgot them too.
    Truncate {
        item_id: i64,
        context: bool,
    },
    DeleteThread,
}

/// What a change did, for the consumer to announce and act on.
pub(in crate::harness) enum Applied {
    Began(Box<Began>),
    /// The queue changed: the webview's list is told again.
    Queue,
    /// These cards' continuations were cancelled.
    Cards(Vec<String>),
    Truncated {
        item_id: i64,
        context: bool,
    },
    Deleted,
}

/// A run written and ready to be sent.
pub(in crate::harness) struct Began {
    pub provider: Provider,
    pub run: RunStart,
    /// The row's text: what the student typed, or a continuation's line.
    pub text: String,
    pub options: SendOptions,
    /// The run waited in the queue before this.
    pub waited: bool,
    /// The send changed the chat's model, which a CLI that fixes its model
    /// at spawn can only take as a new session.
    pub respawn: bool,
}

pub(in crate::harness) struct Committed {
    pub outcome: Outcome,
    /// The thread's revision with everything written; 0 for a deleted one.
    pub revision: i64,
    pub applied: Vec<Applied>,
}

/// A command's identity, for its receipt.
pub(in crate::harness) struct Receipt<'a> {
    pub id: &'a str,
    pub command: &'static str,
}

/// Save `plan` for the chat `thread_id` (`None` when the plan creates it).
/// `receipt` is `None` for what the app does itself: a waiting message or a
/// card's continuation going out.
pub(in crate::harness) async fn run(
    pool: &SqlitePool,
    receipt: Option<Receipt<'_>>,
    thread_id: Option<i64>,
    plan: Plan,
) -> Result<Committed, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    if let Some(receipt) = &receipt {
        receipts::reserve(&mut tx, receipt.id, receipt.command, thread_id).await?;
    }
    let mut thread = thread_id;
    let mut deleted = false;
    let mut applied = Vec::new();
    let mut outcome = Outcome {
        text: plan.text,
        ..Outcome::default()
    };
    for change in plan.changes {
        match change {
            Change::CreateThread {
                provider,
                options,
                first_message,
            } => {
                thread = Some(create(&mut tx, provider, &options, &first_message).await?);
            }
            Change::Begin {
                provider,
                text,
                options,
                receipt,
                queued,
            } => {
                let thread_id = thread.ok_or("a send with no thread")?;
                let send = (text, options, receipt, queued);
                let began = begin(&mut tx, thread_id, provider, send).await?;
                outcome.run = Some(began.run);
                applied.push(Applied::Began(Box::new(began)));
            }
            Change::Enqueue {
                provider,
                text,
                options,
            } => {
                let thread_id = thread.ok_or("a send with no thread")?;
                let run_id =
                    store::queue::enqueue(&mut tx, thread_id, provider, &text, &options).await?;
                outcome.queued = Some(run_id);
                applied.push(Applied::Queue);
            }
            Change::EditQueued { run_id, text } => {
                let thread_id = thread.ok_or("a queue with no thread")?;
                if store::queue::edit(&mut tx, thread_id, run_id, &text).await? {
                    applied.push(Applied::Queue);
                }
            }
            Change::CloseQueued { run_id, failed } => {
                let thread_id = thread.ok_or("a queue with no thread")?;
                let status = match failed {
                    true => "failed",
                    false => "cancelled",
                };
                if store::queue::close(&mut tx, thread_id, run_id, status).await? {
                    applied.push(Applied::Queue);
                }
            }
            Change::ReorderQueued { order } => {
                let thread_id = thread.ok_or("a queue with no thread")?;
                store::queue::reorder(&mut tx, thread_id, &order).await?;
                applied.push(Applied::Queue);
            }
            Change::ClearQueue => {
                let thread_id = thread.ok_or("a queue with no thread")?;
                let cleared = store::queue::clear(&mut tx, thread_id).await?;
                let mut stopped = Vec::new();
                for card in cleared.cards {
                    let reason = crate::harness::app::continuation::STOPPED;
                    if crate::harness::app::continuation::cancel(&mut *tx, &card, reason).await? {
                        stopped.push(card);
                    }
                }
                outcome.dropped = cleared.texts;
                applied.push(Applied::Queue);
                applied.push(Applied::Cards(stopped));
            }
            Change::Truncate { item_id, context } => {
                let thread_id = thread.ok_or("a rewind with no thread")?;
                store::truncate(&mut tx, thread_id, item_id).await?;
                applied.push(Applied::Truncated { item_id, context });
            }
            Change::DeleteThread => {
                let thread_id = thread.ok_or("a delete with no thread")?;
                store::threads::delete(&mut tx, thread_id).await?;
                receipts::forget(&mut tx, thread_id).await?;
                deleted = true;
                applied.push(Applied::Deleted);
            }
        }
    }
    outcome.thread_id = thread.ok_or("a command with no thread")?;
    let revision = match deleted {
        true => 0,
        false => store::raise_revision(&mut tx, outcome.thread_id).await?,
    };
    if let Some(receipt) = &receipt {
        receipts::store(&mut tx, receipt.id, &outcome).await?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(Committed {
        outcome,
        revision,
        applied,
    })
}

/// The chat a send starts, and the Studio video it makes, linked.
async fn create(
    tx: &mut SqliteConnection,
    provider: Provider,
    options: &SendOptions,
    first_message: &str,
) -> Result<i64, String> {
    let id = store::threads::create(
        &mut *tx,
        provider,
        options.model.as_deref(),
        options.subject_id,
        options.lecture_id.as_deref(),
        first_message,
    )
    .await?;
    if let Some(video) = &options.studio {
        let length = video.length_secs.map(i64::from);
        crate::db::store::link_studio_video(&mut *tx, &video.slug, id, length).await?;
    }
    Ok(id)
}

/// Write a run that goes out now. The row, not the send, decides what it
/// resumes; a send that names another model changes the chat's from here on.
async fn begin(
    tx: &mut SqliteConnection,
    thread_id: i64,
    provider: Provider,
    (text, options, receipt, queued): (String, SendOptions, String, Option<i64>),
) -> Result<Began, String> {
    let row = sqlx::query("SELECT model, provider_session_id FROM harness_threads WHERE id = ?1")
        .bind(thread_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no thread {thread_id}"))?;
    let model: Option<String> = row.get("model");
    let respawn = options.model.is_some() && options.model != model;
    if respawn {
        sqlx::query("UPDATE harness_threads SET model = ?2 WHERE id = ?1")
            .bind(thread_id)
            .bind(&options.model)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    let intent = SendIntent {
        thread_id,
        provider,
        model: options.model.clone().or(model),
        native_session: row.get("provider_session_id"),
        receipt,
        text,
        at: options.at,
        continuation: options.continuation.clone(),
        queued,
    };
    let run = store::begin(&mut *tx, &intent).await?;
    Ok(Began {
        provider,
        run,
        text: intent.text,
        options,
        waited: queued.is_some(),
        respawn,
    })
}
