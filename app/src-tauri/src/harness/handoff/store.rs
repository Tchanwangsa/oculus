//! One handoff per run, in `harness_handoffs`. It is planned and saved in
//! the commit that writes its run ([`plan`], [`save`]), rendered by the
//! run's start step ([`outgoing`]) and settled once the provider is known
//! to have the prompt, or known not to ([`delivered`], [`undelivered`]).
//! A row left `pending` means nobody knows what that session holds.

use serde::{Deserialize, Serialize};
use sqlx::{Row as _, SqliteConnection, SqliteExecutor};

use crate::harness::manager::transition::Selection;
use crate::harness::store::bindings::{self, Binding};
use crate::harness::Provider;

use super::budget::{handoff_budget, usage_for, Budget, Usage, DEFAULT_CAP};
use super::history::{self, collect, select_history, Kind, Message, Role, Row, RunOf};

/// The handoff a run's first prompt will carry, before its run exists.
pub(in crate::harness) struct Draft {
    pub kind: Kind,
    /// The binding that answered the chat's newest run, when it is not the
    /// one taking over.
    pub from_binding: Option<i64>,
    /// The run ordinals it stands for.
    pub covered_from: i64,
    pub covered_to: i64,
    /// The timeline rows it carries, in order.
    pub items: Vec<i64>,
    pub omitted: usize,
    pub bytes: usize,
    /// Why it cannot be sent: it does not fit the target's window.
    pub error: Option<String>,
}

/// Where a run goes out, and what it takes along.
pub(in crate::harness) struct Planned {
    pub binding: Binding,
    pub handoff: Option<Draft>,
}

/// The run fails with this when even a new session has no room for the
/// block's opening lines beside the student's message.
pub(in crate::harness) const OVER_BUDGET: &str =
    "The earlier conversation does not fit beside this message in the chosen model's context. Shorten the message, or choose a model with a larger context.";

/// Decide which session a run of `selection`'s provider goes out on, and
/// which earlier conversation rides its prompt. In order:
///
/// 1. A handoff into the provider's current session that was sent and
///    never confirmed makes that session's contents unknown: a new one
///    starts, with the whole chat.
/// 2. A command for the CLI itself (`/compact`) must stay the first thing
///    in its prompt, so it carries nothing; the next message does.
/// 3. A session that has seen none of the chat gets all of it (`full`);
///    one that was here before gets what it missed (`delta`).
/// 4. When that does not fit beside what a resumed session already holds,
///    a new session with the whole chat is tried; when that does not fit
///    either, the resumed session is kept and the handoff is saved as
///    failed, which fails the send where the student sees it.
pub(in crate::harness) async fn plan(
    tx: &mut SqliteConnection,
    thread_id: i64,
    selection: &Selection,
    text: &str,
) -> Result<Planned, String> {
    let provider = selection.provider;
    let mut binding = bindings::current(&mut *tx, thread_id, provider).await?;
    if uncertain(&mut *tx, binding.id).await? {
        binding = bindings::fresh(&mut *tx, thread_id, provider).await?;
    }
    if text.trim_start().starts_with('/') {
        return Ok(Planned {
            binding,
            handoff: None,
        });
    }
    let rows = rows(&mut *tx, thread_id).await?;
    let newest = newest_run(&mut *tx, thread_id).await?;
    let resumed = Target::of(&mut *tx, &binding, selection).await?;
    let first = draft(&rows, newest, &resumed, text);
    let over = first.as_ref().is_some_and(|draft| draft.error.is_some());
    if over && binding.native_session.is_some() {
        let anew = Target {
            binding_id: None,
            covered_through: 0,
            full: true,
            usage: None,
            window: binding.context_window.map(|w| w as u64),
        };
        let second = draft(&rows, newest, &anew, text);
        if second.as_ref().is_some_and(|draft| draft.error.is_none()) {
            return Ok(Planned {
                binding: bindings::fresh(&mut *tx, thread_id, provider).await?,
                handoff: second,
            });
        }
    }
    Ok(Planned {
        binding,
        handoff: first,
    })
}

/// The session a draft is made for.
struct Target {
    /// `None` for a session that does not exist yet.
    binding_id: Option<i64>,
    covered_through: i64,
    /// It has seen none of the chat.
    full: bool,
    usage: Option<Usage>,
    window: Option<u64>,
}

impl Target {
    /// The provider's current session, as its binding describes it. What
    /// it reported under another model keeps its occupancy.
    async fn of(
        tx: &mut SqliteConnection,
        binding: &Binding,
        selection: &Selection,
    ) -> Result<Self, String> {
        let last_model: Option<Option<String>> = sqlx::query_scalar(
            "SELECT model FROM harness_runs WHERE binding_id = ?1 AND ordinal > 0
              ORDER BY ordinal DESC LIMIT 1",
        )
        .bind(binding.id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        let same_model = last_model.is_some_and(|model| model == selection.model);
        let reported = binding.context_tokens.map(|used| Usage {
            used: used.max(0) as u64,
            max: binding.context_window.map(|w| w.max(0) as u64),
        });
        // A session with no native id and no coverage has seen nothing. One
        // that covers runs is alive in this process, named or not: when it
        // is lost its coverage goes with it (`bindings::forget_unnamed`).
        let resumes = binding.native_session.is_some();
        Ok(Target {
            binding_id: Some(binding.id),
            covered_through: binding.covered_through,
            full: !resumes && binding.covered_through == 0,
            usage: usage_for(reported, same_model, None),
            window: binding.context_window.map(|w| w.max(0) as u64),
        })
    }
}

/// The chat's newest run that started: its ordinal and its binding.
async fn newest_run(
    tx: &mut SqliteConnection,
    thread_id: i64,
) -> Result<Option<(i64, i64)>, String> {
    sqlx::query_as(
        "SELECT ordinal, binding_id FROM harness_runs
          WHERE thread_id = ?1 AND ordinal > 0 ORDER BY ordinal DESC LIMIT 1",
    )
    .bind(thread_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| e.to_string())
}

fn draft(rows: &[Row], newest: Option<(i64, i64)>, target: &Target, text: &str) -> Option<Draft> {
    let messages = collect(rows, target.covered_through, target.full);
    if messages.is_empty() {
        return None;
    }
    let kind = match target.full {
        true => Kind::Full,
        false => Kind::Delta,
    };
    let budget = handoff_budget(&Budget {
        cap: DEFAULT_CAP,
        user_text: text,
        images: 0,
        files: 0,
        usage: target.usage,
        native_estimate: 0,
        model_window: target.window,
    });
    let selected = select_history(&messages, kind.coverage(), budget);
    let bytes = selected.cost();
    let (covered_to, newest_binding) = newest.unwrap_or_default();
    Some(Draft {
        kind,
        from_binding: Some(newest_binding).filter(|id| *id > 0 && Some(*id) != target.binding_id),
        covered_from: target.covered_through + 1,
        covered_to,
        items: selected.messages.iter().map(|m| m.item_id).collect(),
        omitted: selected.omitted,
        bytes,
        error: (bytes > budget).then(|| OVER_BUDGET.to_string()),
    })
}

/// Every row a handoff may carry, with its run.
async fn rows(tx: &mut SqliteConnection, thread_id: i64) -> Result<Vec<Row>, String> {
    read_rows(
        &mut *tx,
        "i.thread_id = ?1 AND i.kind IN ('user', 'assistant')",
        thread_id,
    )
    .await
}

async fn read_rows(
    db: impl SqliteExecutor<'_>,
    filter: &str,
    bound: i64,
) -> Result<Vec<Row>, String> {
    let found = sqlx::query(&format!(
        "SELECT i.id, i.kind, i.content, r.ordinal, r.answer_item_id, b.provider
           FROM harness_items i
           LEFT JOIN harness_runs r ON r.id = i.run_id AND r.ordinal > 0
           LEFT JOIN harness_bindings b ON b.id = r.binding_id
          WHERE {filter} ORDER BY i.id"
    ))
    .bind(bound)
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(found
        .iter()
        .map(|row| {
            let item_id: i64 = row.get("id");
            let provider: Option<String> = row.get("provider");
            let run = row
                .get::<Option<i64>, _>("ordinal")
                .zip(provider.as_deref().and_then(Provider::parse))
                .map(|(ordinal, by)| RunOf {
                    ordinal,
                    answer: row.get::<Option<i64>, _>("answer_item_id") == Some(item_id),
                    by,
                });
            Row {
                item_id,
                student: row.get::<String, _>("kind") == "user",
                text: row.get::<Option<String>, _>("content").unwrap_or_default(),
                run,
            }
        })
        .collect())
}

/// A handoff went out to this session and was never confirmed.
async fn uncertain(tx: &mut SqliteConnection, binding_id: i64) -> Result<bool, String> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM harness_handoffs
                         WHERE to_binding_id = ?1 AND status = 'pending')",
    )
    .bind(binding_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())
}

/// Save a run's handoff: `pending`, or `failed` for one that does not fit.
pub(in crate::harness) async fn save(
    tx: &mut SqliteConnection,
    thread_id: i64,
    run_id: i64,
    binding_id: i64,
    draft: &Draft,
) -> Result<(), String> {
    let status = match draft.error {
        Some(_) => "failed",
        None => "pending",
    };
    sqlx::query(
        "INSERT INTO harness_handoffs
           (thread_id, run_id, from_binding_id, to_binding_id, kind, covered_from, covered_to,
            items, omitted, bytes, status, error)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )
    .bind(thread_id)
    .bind(run_id)
    .bind(draft.from_binding)
    .bind(binding_id)
    .bind(draft.kind.as_str())
    .bind(draft.covered_from)
    .bind(draft.covered_to)
    .bind(serde_json::to_string(&draft.items).map_err(|e| e.to_string())?)
    .bind(draft.omitted as i64)
    .bind(draft.bytes as i64)
    .bind(status)
    .bind(&draft.error)
    .execute(&mut *tx)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// The block a run's prompt opens with; `None` for a run with no handoff.
/// An `Err` is a handoff that cannot be sent, and so a run that must not be.
pub(in crate::harness) async fn outgoing(
    pool: &sqlx::SqlitePool,
    run_id: i64,
) -> Result<Option<String>, String> {
    let err = |e: sqlx::Error| e.to_string();
    let saved = sqlx::query(
        "SELECT id, kind, items, omitted, status, error FROM harness_handoffs WHERE run_id = ?1",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(err)?;
    let Some(saved) = saved else {
        return Ok(None);
    };
    if saved.get::<String, _>("status") == "failed" {
        let why: Option<String> = saved.get("error");
        return Err(why.unwrap_or_else(|| OVER_BUDGET.into()));
    }
    let items: Vec<i64> =
        serde_json::from_str(&saved.get::<String, _>("items")).map_err(|e| e.to_string())?;
    let rows = read_rows(
        pool,
        "i.id IN (SELECT value FROM json_each((SELECT items FROM harness_handoffs WHERE id = ?1)))",
        saved.get("id"),
    )
    .await?;
    let messages: Vec<Message> = rows
        .into_iter()
        .filter(|row| items.contains(&row.item_id))
        .map(|row| Message {
            item_id: row.item_id,
            role: match row.student {
                true => Role::Student,
                false => Role::Agent,
            },
            by: row.run.map(|run| run.by).filter(|_| !row.student),
            text: row.text,
        })
        .collect();
    let kind = Kind::parse(&saved.get::<String, _>("kind"));
    let omitted = saved.get::<i64, _>("omitted").max(0) as usize;
    Ok(Some(history::block(kind, &messages, omitted)))
}

/// The provider has the run's prompt. Its handoff, if it carried one, is
/// `inline`, and its session has now seen the chat through this run:
/// unless earlier runs it has not seen were left for a later prompt (a
/// CLI command carries no handoff), in which case its coverage stays.
pub(in crate::harness) async fn delivered(
    tx: &mut SqliteConnection,
    run_id: i64,
) -> Result<bool, String> {
    let err = |e: sqlx::Error| e.to_string();
    let changed = sqlx::query(
        "UPDATE harness_handoffs SET status = 'inline' WHERE run_id = ?1 AND status = 'pending'",
    )
    .bind(run_id)
    .execute(&mut *tx)
    .await
    .map_err(err)?
    .rows_affected();
    sqlx::query(
        "UPDATE harness_bindings
            SET covered_through = MAX(covered_through, r.ordinal)
           FROM harness_runs r
          WHERE r.id = ?1 AND r.ordinal > 0 AND harness_bindings.id = r.binding_id
            AND (EXISTS (SELECT 1 FROM harness_handoffs h
                          WHERE h.run_id = r.id AND h.status = 'inline')
                 OR NOT EXISTS (SELECT 1 FROM harness_runs e
                                 WHERE e.thread_id = r.thread_id
                                   AND e.ordinal > harness_bindings.covered_through
                                   AND e.ordinal < r.ordinal
                                   AND e.initiating_item_id IS NOT NULL))",
    )
    .bind(run_id)
    .execute(&mut *tx)
    .await
    .map_err(err)?;
    Ok(changed > 0)
}

/// The provider never heard the run's prompt: its handoff was not
/// delivered, and the next run plans one again.
pub(in crate::harness) async fn undelivered(
    tx: &mut SqliteConnection,
    run_id: i64,
) -> Result<bool, String> {
    sqlx::query(
        "UPDATE harness_handoffs SET status = 'failed' WHERE run_id = ?1 AND status = 'pending'",
    )
    .bind(run_id)
    .execute(&mut *tx)
    .await
    .map(|done| done.rows_affected() > 0)
    .map_err(|e| e.to_string())
}

/// A run's start step is over. `reached` is whether the provider has its
/// prompt: its handoff becomes `inline` or `failed`, and a view is told
/// the run's handoff as it now stands, with the thread's revision when it
/// changed. `None` for a run that carries none.
pub(in crate::harness) async fn settle(
    pool: &sqlx::SqlitePool,
    run_id: i64,
    reached: bool,
) -> Result<Option<(Option<i64>, HandoffView)>, String> {
    let err = |e: sqlx::Error| e.to_string();
    let mut tx = pool.begin().await.map_err(err)?;
    let changed = match reached {
        true => delivered(&mut tx, run_id).await?,
        false => undelivered(&mut tx, run_id).await?,
    };
    let view = view_of(&mut *tx, run_id).await?;
    let mut revision = None;
    if changed {
        let thread: Option<i64> =
            sqlx::query_scalar("SELECT thread_id FROM harness_runs WHERE id = ?1")
                .bind(run_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(err)?;
        if let Some(thread_id) = thread {
            revision = Some(crate::harness::store::raise_revision(&mut tx, thread_id).await?);
        }
    }
    tx.commit().await.map_err(err)?;
    Ok(view.map(|view| (revision, view)))
}

/// A run's handoff as a view draws it: a divider that says which provider
/// handed over, whether the handoff has gone in, and how much was left out.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HandoffView {
    /// `full` or `delta`.
    pub kind: String,
    /// `pending`, `inline` or `failed`.
    pub status: String,
    /// The provider that answered before, when it is another one.
    pub from: Option<Provider>,
    /// How many earlier messages were left out to fit.
    pub omitted: i64,
}

const VIEW: &str = "SELECT h.run_id, h.kind, h.status, h.omitted, b.provider AS from_provider
                      FROM harness_handoffs h
                      LEFT JOIN harness_bindings b ON b.id = h.from_binding_id";

fn view(row: &sqlx::sqlite::SqliteRow) -> (i64, HandoffView) {
    let from: Option<String> = row.get("from_provider");
    let view = HandoffView {
        kind: row.get("kind"),
        status: row.get("status"),
        from: from.as_deref().and_then(Provider::parse),
        omitted: row.get("omitted"),
    };
    (row.get("run_id"), view)
}

/// One run's handoff, if it has one.
pub(in crate::harness) async fn view_of(
    db: impl SqliteExecutor<'_>,
    run_id: i64,
) -> Result<Option<HandoffView>, String> {
    sqlx::query(&format!("{VIEW} WHERE h.run_id = ?1"))
        .bind(run_id)
        .fetch_optional(db)
        .await
        .map(|row| row.as_ref().map(|row| view(row).1))
        .map_err(|e| e.to_string())
}

/// Every handoff of a chat, by run.
pub(in crate::harness) async fn views(
    db: impl SqliteExecutor<'_>,
    thread_id: i64,
) -> Result<std::collections::HashMap<i64, HandoffView>, String> {
    sqlx::query(&format!("{VIEW} WHERE h.thread_id = ?1"))
        .bind(thread_id)
        .fetch_all(db)
        .await
        .map(|rows| rows.iter().map(view).collect())
        .map_err(|e| e.to_string())
}
