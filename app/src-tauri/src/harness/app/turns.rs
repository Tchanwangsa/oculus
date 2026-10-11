//! A thread's turns: send, edit and resend, rewind, the queue behind a
//! running turn, interrupt and delete.

use sqlx::SqlitePool;
use tauri::State;

use crate::harness::manager::validate_effort;
use crate::harness::store;
use crate::harness::{HarnessEvent, Provider, QueuedMessage, SendOptions};
use crate::runtime::blocking::run as blocking;

use super::send::{dispatch, lecture_brief};
use super::HarnessState;

/// Send a message; creates the thread when `thread_id` is null. Returns
/// the thread id. A message sent mid-turn waits in the [`Queue`] and the
/// webview gets only a `queued` event until it goes out.
#[tauri::command]
pub async fn harness_send(
    state: State<'_, HarnessState>,
    thread_id: Option<i64>,
    provider: String,
    text: String,
    options: Option<SendOptions>,
) -> Result<i64, String> {
    let provider =
        Provider::parse(&provider).ok_or_else(|| format!("unknown provider {provider}"))?;
    let mut opts = options.unwrap_or_default();
    opts.reasoning_effort = validate_effort(opts.reasoning_effort)?;
    let pool = crate::db::store::open_pool().await?;

    let id = match thread_id {
        Some(id) => {
            let row = store::thread(&pool, id).await?;
            if row.provider != provider {
                return Err(format!("thread {id} is a {} thread", row.provider.label()));
            }
            id
        }
        None => {
            store::create_thread(
                &pool,
                provider,
                opts.model.as_deref(),
                opts.subject_id,
                opts.lecture_id.as_deref(),
                &text,
            )
            .await?
        }
    };

    if !state.queue.lock().unwrap().try_claim(id) {
        let msg = state.queue.lock().unwrap().push(id, &text, &opts);
        state.sink(id, provider)(HarnessEvent::Queued {
            id: msg.id,
            text: msg.text,
        });
        return Ok(id);
    }
    dispatch(
        state.harness.clone(),
        state.bus.clone(),
        id,
        provider,
        opts,
        text,
    )
    .await?;
    Ok(id)
}

/// Ask a question again, differently: rewind the thread to that question
/// (rows and, where it can, the agent's context) and send the new text.
/// A provider that cannot rewind is not fatal — the rows still go, and
/// the `Rewound` event says the agent kept the original.
#[tauri::command]
pub async fn harness_edit_resend(
    state: State<'_, HarnessState>,
    thread_id: i64,
    item_id: i64,
    text: String,
    options: Option<SendOptions>,
) -> Result<(), String> {
    let mut opts = options.unwrap_or_default();
    opts.reasoning_effort = validate_effort(opts.reasoning_effort)?;
    let pool = crate::db::store::open_pool().await?;
    let row = store::thread(&pool, thread_id).await?;
    // Checked against the row: everything from it on is about to go.
    let question = store::user_item(&pool, thread_id, item_id).await?;
    if !state.queue.lock().unwrap().try_claim(thread_id) {
        return Err("stop the current turn before editing a question".into());
    }
    let context = rewind_provider(&state, &pool, thread_id, &row, &question, &opts).await;
    if let Err(e) = store::truncate_from(&pool, thread_id, item_id).await {
        // Nothing was sent, so nothing will close the turn this claimed.
        state.queue.lock().unwrap().next(thread_id);
        return Err(e);
    }
    let sink = state.sink(thread_id, row.provider);
    sink(HarnessEvent::Rewound {
        from_item_id: item_id,
        context,
    });
    dispatch(
        state.harness.clone(),
        state.bus.clone(),
        thread_id,
        row.provider,
        opts,
        text,
    )
    .await
}

/// Ask the provider to forget this question and everything after it, and
/// answer whether it did. No anchor, a refusal or an unresumable session
/// all answer `false`, which the timeline shows as a note.
async fn rewind_provider(
    state: &State<'_, HarnessState>,
    pool: &SqlitePool,
    thread_id: i64,
    row: &store::ThreadRow,
    question: &store::Question,
    opts: &SendOptions,
) -> bool {
    let Some(anchor) = question.anchor.clone() else {
        return false;
    };
    let Some(resume) = row.provider_session_id.clone() else {
        return false;
    };
    let last_seen = store::newest_anchor(pool, thread_id).await.ok().flatten();
    let (h, provider) = (state.harness.clone(), row.provider);
    let sink = state.sink(thread_id, provider);
    // A rewind may respawn the session, which binds the brief.
    let opts = SendOptions {
        model: opts.model.clone().or_else(|| row.model.clone()),
        scope: row.subject_code.clone(),
        lecture: lecture_brief(pool, row).await,
        ..opts.clone()
    };
    blocking(move || {
        h.rewind(
            thread_id,
            provider,
            Some(&resume),
            &opts,
            &anchor,
            last_seen.as_deref(),
            sink,
        )
    })
    .await
    .is_ok()
}

/// Take the thread back to just before a question and hand the question
/// back for the composer. Unlike [`harness_edit_resend`], nothing is sent.
#[tauri::command]
pub async fn harness_rewind(
    state: State<'_, HarnessState>,
    thread_id: i64,
    item_id: i64,
) -> Result<String, String> {
    let pool = crate::db::store::open_pool().await?;
    let row = store::thread(&pool, thread_id).await?;
    let question = store::user_item(&pool, thread_id, item_id).await?;
    if state.queue.lock().unwrap().is_busy(thread_id) {
        return Err("stop the current turn before rewinding".into());
    }
    let opts = SendOptions {
        model: row.model.clone(),
        ..Default::default()
    };
    let context = rewind_provider(&state, &pool, thread_id, &row, &question, &opts).await;
    store::truncate_from(&pool, thread_id, item_id).await?;
    state.sink(thread_id, row.provider)(HarnessEvent::Rewound {
        from_item_id: item_id,
        context,
    });
    Ok(question.text)
}

/// What is still waiting behind this thread's turn (the queue is only in
/// memory), for a reloaded page.
#[tauri::command]
pub async fn harness_queued(
    state: State<'_, HarnessState>,
    thread_id: i64,
) -> Result<Vec<QueuedMessage>, String> {
    Ok(state.queue.lock().unwrap().list(thread_id))
}

/// Drop one message that has not gone out yet.
#[tauri::command]
pub async fn harness_unqueue(
    state: State<'_, HarnessState>,
    thread_id: i64,
    queue_id: String,
) -> Result<(), String> {
    if state.queue.lock().unwrap().remove(thread_id, &queue_id) {
        let pool = crate::db::store::open_pool().await?;
        let row = store::thread(&pool, thread_id).await?;
        state.sink(thread_id, row.provider)(HarnessEvent::Unqueued { id: queue_id });
    }
    Ok(())
}

/// Rewrite one that has not gone out yet; it comes back as a `queued`
/// event under the same id, so the webview replaces it.
#[tauri::command]
pub async fn harness_edit_queued(
    state: State<'_, HarnessState>,
    thread_id: i64,
    queue_id: String,
    text: String,
) -> Result<(), String> {
    let edited = state
        .queue
        .lock()
        .unwrap()
        .edit(thread_id, &queue_id, &text);
    if let Some(msg) = edited {
        let pool = crate::db::store::open_pool().await?;
        let row = store::thread(&pool, thread_id).await?;
        state.sink(thread_id, row.provider)(HarnessEvent::Queued {
            id: msg.id,
            text: msg.text,
        });
    }
    Ok(())
}

/// Stop the running turn and drop whatever was waiting behind it,
/// handing the dropped texts back to the composer.
#[tauri::command]
pub async fn harness_interrupt(
    state: State<'_, HarnessState>,
    thread_id: i64,
) -> Result<Vec<String>, String> {
    let cleared = state.queue.lock().unwrap().clear(thread_id);
    if !cleared.is_empty() {
        let pool = crate::db::store::open_pool().await?;
        let row = store::thread(&pool, thread_id).await?;
        let sink = state.sink(thread_id, row.provider);
        for m in &cleared {
            sink(HarnessEvent::Unqueued { id: m.id.clone() });
        }
    }
    let h = state.harness.clone();
    blocking(move || h.interrupt(thread_id)).await?;
    Ok(cleared.into_iter().map(|m| m.text).collect())
}

#[tauri::command]
pub async fn harness_delete_thread(
    state: State<'_, HarnessState>,
    thread_id: i64,
) -> Result<(), String> {
    state.harness.close(thread_id);
    state.queue.lock().unwrap().forget(thread_id);
    let pool = crate::db::store::open_pool().await?;
    store::delete_thread(&pool, thread_id).await
}
