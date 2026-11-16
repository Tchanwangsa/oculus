//! The one transaction a command gets: its receipt, every change its
//! decider asked for, the thread's raised revision and the stored result.
//! All of it lands or none of it does, and nobody is told until it has.
//! Nothing else writes for a command.

use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::harness::handoff::store::{self as handoff, HandoffView};
use crate::harness::manager::transition::Selection;
use crate::harness::requests::{self, PermissionMode, RequestView};
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
    /// The chat's selection becomes this: what its next run is sent with.
    Select {
        provider: Provider,
        model: Option<String>,
        effort: Option<String>,
    },
    /// The run of a message going out now: its question row, attempt,
    /// binding and run receipt (`store::begin`). It is sent with the
    /// chat's selection as it stands in this commit.
    Begin {
        text: String,
        options: SendOptions,
        /// The run receipt's id: the command's own for a send, a fresh one
        /// for a run the app starts.
        receipt: String,
        /// The waiting run that starts, when the message waited.
        queued: Option<i64>,
    },
    /// A message, or a card's continuation, joins the back of the queue as
    /// a waiting run (`store/queue.rs`); with `front`, its head.
    Enqueue {
        text: String,
        options: SendOptions,
        front: bool,
    },
    /// A message written into the open run (`store/steer.rs`): a new one,
    /// or with `from` a waiting one taken out of the queue. With `restart`
    /// the run's turn is stopped and started again with it.
    Steer {
        text: String,
        options: SendOptions,
        target: store::steer::Target,
        from: Option<i64>,
        restart: bool,
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
    /// The run is marked as asked to stop.
    RequestStop {
        run_id: i64,
    },
    /// The queue is held: the student's waiting messages stay and none
    /// goes out; a waiting continuation is cancelled and its card says it
    /// was stopped.
    HoldQueue,
    /// The queue's hold is lifted: its runs may start again.
    Unhold,
    /// The rows from `item_id` on leave the chat. `context` is whether the
    /// provider forgot them too.
    Truncate {
        item_id: i64,
        context: bool,
    },
    SetCompleted {
        completed: bool,
    },
    DeleteThread,
    /// The student's answer claims the request: one conditional write, so
    /// of two answers only one lands.
    Respond {
        request_id: i64,
        option_id: String,
        answers: Option<Vec<requests::QuestionAnswer>>,
    },
    /// How the chat asks from its next run on.
    SetMode {
        mode: PermissionMode,
    },
}

/// What a change did, for the consumer to announce and act on.
pub(in crate::harness) enum Applied {
    Began(Box<Began>),
    /// The chat's selection changed.
    Selected {
        provider: Provider,
        model: Option<String>,
        effort: Option<String>,
    },
    /// The queue changed: the webview's list is told again.
    Queue,
    /// A message was written into the open run; its provider is told next.
    Steered(Box<Steered>),
    /// These cards' continuations were cancelled.
    Cards(Vec<String>),
    /// This run was asked to stop.
    Stopping(i64),
    Truncated {
        item_id: i64,
        context: bool,
        /// The providers whose session held rows that went, and was given
        /// up for it (`bindings::rewound`).
        stale: Vec<Provider>,
    },
    Completion,
    Deleted,
    /// A request was answered; its answer goes to the provider next.
    Responded(Box<RequestView>),
    /// The chat's permission mode changed.
    Mode(PermissionMode),
}

/// A steer as committed, for the consumer to tell and to write.
pub(in crate::harness) struct Steered {
    pub written: store::steer::Written,
    pub run_id: i64,
    pub text: String,
    /// The id its provider will know it by and echo back.
    pub client: String,
    /// It waits for the run's turn to stop, and goes in as a new attempt.
    pub restart: bool,
}

/// A run written as `starting`: its row is in the timeline, and the driver
/// will hand it to its start step.
pub(in crate::harness) struct Began {
    pub provider: Provider,
    /// The model and reasoning level it is sent with.
    pub model: Option<String>,
    pub effort: Option<String>,
    /// The earlier conversation its prompt carries, if any.
    pub handoff: Option<HandoffView>,
    pub run: RunStart,
    /// The row's text: what the student typed, or a continuation's line.
    pub text: String,
    /// The playhead's second, for a message from the lecture dock.
    pub at: Option<i64>,
    /// The card this run continues.
    pub continuation: Option<String>,
    /// The run waited in the queue before this.
    pub waited: bool,
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
            Change::Select {
                provider,
                model,
                effort,
            } => {
                let thread_id = thread.ok_or("a selection with no thread")?;
                let (m, e) = (model.as_deref(), effort.as_deref());
                store::threads::select(&mut tx, thread_id, provider, m, e).await?;
                applied.push(Applied::Selected {
                    provider,
                    model,
                    effort,
                });
            }
            Change::Begin {
                text,
                options,
                receipt,
                queued,
            } => {
                let thread_id = thread.ok_or("a send with no thread")?;
                if reopen_on_send(&mut tx, thread_id).await? {
                    applied.push(Applied::Completion);
                }
                let send = (text, options, receipt, queued);
                let began = begin(&mut tx, thread_id, send).await?;
                outcome.run = Some(began.run);
                applied.push(Applied::Began(Box::new(began)));
            }
            Change::Enqueue {
                text,
                options,
                front,
            } => {
                let thread_id = thread.ok_or("a send with no thread")?;
                if reopen_on_send(&mut tx, thread_id).await? {
                    applied.push(Applied::Completion);
                }
                let provider = selection(&mut tx, thread_id, &options).await?.provider;
                let run_id =
                    store::queue::enqueue(&mut tx, thread_id, provider, &text, &options).await?;
                if front {
                    store::queue::to_front(&mut tx, thread_id, run_id).await?;
                }
                outcome.queued = Some(run_id);
                applied.push(Applied::Queue);
            }
            Change::Steer {
                text,
                options,
                target,
                from,
                restart,
            } => {
                let thread_id = thread.ok_or("a steer with no thread")?;
                if reopen_on_send(&mut tx, thread_id).await? {
                    applied.push(Applied::Completion);
                }
                let client = store::steer::client_id();
                let state = match restart {
                    true => store::steer::delivery::RESTART,
                    false => store::steer::delivery::SENDING,
                };
                let send = (text.as_str(), &options);
                let written =
                    store::steer::write(&mut tx, thread_id, target, send, &client, from, state)
                        .await?;
                // "Send now" on a message that has just gone out on its own.
                let Some(written) = written else { continue };
                if from.is_some() {
                    applied.push(Applied::Queue);
                }
                outcome.steered = Some(written);
                applied.push(Applied::Steered(Box::new(Steered {
                    written,
                    run_id: target.run_id,
                    text,
                    client,
                    restart,
                })));
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
            Change::RequestStop { run_id } => {
                let thread_id = thread.ok_or("a stop with no thread")?;
                if store::queue::request_stop(&mut tx, thread_id, run_id).await? {
                    outcome.stopped = Some(run_id);
                    applied.push(Applied::Stopping(run_id));
                }
            }
            Change::HoldQueue => {
                let thread_id = thread.ok_or("a queue with no thread")?;
                let reason = crate::harness::app::continuation::STOPPED;
                let stopped =
                    crate::harness::app::continuation::hold_queue(&mut tx, thread_id, reason)
                        .await?;
                applied.push(Applied::Queue);
                applied.push(Applied::Cards(stopped));
            }
            Change::Unhold => {
                let thread_id = thread.ok_or("a queue with no thread")?;
                store::queue::unhold(&mut tx, thread_id).await?;
                applied.push(Applied::Queue);
            }
            Change::Truncate { item_id, context } => {
                let thread_id = thread.ok_or("a rewind with no thread")?;
                let stale = store::bindings::rewound(&mut tx, thread_id, item_id).await?;
                store::truncate(&mut tx, thread_id, item_id).await?;
                applied.push(Applied::Truncated {
                    item_id,
                    context,
                    stale,
                });
            }
            Change::Respond {
                request_id,
                option_id,
                answers,
            } => {
                let thread_id = thread.ok_or("an answer with no thread")?;
                let claimed = requests::store::respond(
                    &mut tx,
                    thread_id,
                    request_id,
                    &option_id,
                    answers.as_deref(),
                )
                .await?;
                let request = claimed.ok_or(crate::harness::core::decide::respond::ANSWERED)?;
                outcome.answered = Some(request_id);
                applied.push(Applied::Responded(Box::new(request)));
            }
            Change::SetMode { mode } => {
                let thread_id = thread.ok_or("a mode with no thread")?;
                requests::store::set_mode(&mut tx, thread_id, mode).await?;
                applied.push(Applied::Mode(mode));
            }
            Change::SetCompleted { completed } => {
                let thread_id = thread.ok_or("completion with no thread")?;
                store::completed::set(&mut tx, thread_id, completed).await?;
                applied.push(Applied::Completion);
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

/// Recheck in the write transaction: the hourly sweep may have completed a
/// chat after its send decision was read but before the commit got the lock.
async fn reopen_on_send(conn: &mut SqliteConnection, thread: i64) -> Result<bool, String> {
    sqlx::query("UPDATE harness_threads SET completed_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1 AND completed_at IS NOT NULL")
        .bind(thread).execute(conn).await.map(|r| r.rows_affected() > 0).map_err(|e| e.to_string())
}

/// The chat a send starts, and the Studio video it makes, linked.
async fn create(
    tx: &mut SqliteConnection,
    provider: Provider,
    options: &SendOptions,
    first_message: &str,
) -> Result<i64, String> {
    let chosen = (
        provider,
        options.model.as_deref(),
        options.reasoning_effort.as_deref(),
    );
    let id = store::threads::create(
        &mut *tx,
        chosen,
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

/// What a run on this chat is sent with: the chat's selection, which a
/// student's message takes as it stands when the run starts, however long
/// it waited. A card's continuation has no composer behind it and never
/// switches the chat: it carries on under what the newest run was sent
/// with.
async fn selection(
    tx: &mut SqliteConnection,
    thread_id: i64,
    options: &SendOptions,
) -> Result<Selection, String> {
    if options.continuation.is_some() {
        if let Some((provider, model, effort)) = store::last_selection(&mut *tx, thread_id).await? {
            return Ok(Selection {
                provider,
                model,
                effort,
            });
        }
    }
    let row = sqlx::query("SELECT provider, model, effort FROM harness_threads WHERE id = ?1")
        .bind(thread_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no thread {thread_id}"))?;
    let provider: String = row.get("provider");
    Ok(Selection {
        provider: Provider::parse(&provider)
            .ok_or_else(|| format!("unknown provider {provider}"))?,
        model: row.get("model"),
        effort: row.get("effort"),
    })
}

/// Write a run that goes out now, under the chat's selection.
async fn begin(
    tx: &mut SqliteConnection,
    thread_id: i64,
    (text, options, receipt, queued): (String, SendOptions, String, Option<i64>),
) -> Result<Began, String> {
    let chosen = selection(&mut *tx, thread_id, &options).await?;
    // Which session it goes out on, and what that session has to be told
    // of the chat first: planned here, so the run, its binding and its
    // handoff are saved together.
    let planned = handoff::plan(&mut *tx, thread_id, &chosen, &text).await?;
    // The send's options stay on the run: its start step reads them there.
    let intent = SendIntent {
        thread_id,
        provider: chosen.provider,
        model: chosen.model,
        effort: chosen.effort,
        binding: Some(planned.binding.id),
        receipt,
        text,
        at: options.at,
        continuation: options.continuation.clone(),
        queued,
        options: Some(store::queue::stored(&options)?),
    };
    let run = store::begin(&mut *tx, &intent).await?;
    let handoff = match &planned.handoff {
        Some(draft) => {
            handoff::save(&mut *tx, thread_id, run.run_id, run.binding_id, draft).await?;
            handoff::view_of(&mut *tx, run.run_id).await?
        }
        None => None,
    };
    Ok(Began {
        provider: intent.provider,
        model: intent.model,
        effort: intent.effort,
        handoff,
        run,
        text: intent.text,
        at: options.at,
        continuation: options.continuation,
        waited: queued.is_some(),
    })
}
