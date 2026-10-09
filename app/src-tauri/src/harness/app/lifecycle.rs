use std::sync::{mpsc, Arc, Mutex};

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager};

use crate::harness::{jobs, opencode, store};
use crate::harness::{Harness, HarnessEvent, Provider, Queue};

use super::send::dispatch;
use super::{sink_for, Envelope, HarnessState};

/// One consumer thread folds every event, from every thread, in order:
/// a row is written before the webview hears about it, and a tool's
/// finish can never overtake its start.
pub fn init(app: &AppHandle) -> HarnessState {
    let (tx, rx) = mpsc::channel::<(i64, Provider, HarnessEvent)>();
    let handle = app.clone();
    let harness = Arc::new(Harness::new(crate::library::paths::data_dir()));
    let queue: Arc<Mutex<Queue>> = Arc::new(Mutex::new(Queue::default()));
    // Thread-less events use the same routing as thread events, on id 0.
    harness.set_codex_account_sink(sink_for(&tx, 0, Provider::Codex));
    harness.set_opencode_default_sink(sink_for(&tx, 0, Provider::Opencode));
    // The naming turn's answer comes back in as an event like any other,
    // so it is written and forwarded by this same loop.
    let (namer, bus, queued) = (harness.clone(), tx.clone(), queue.clone());
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let mut pool: Option<SqlitePool> = None;
        for (thread_id, provider, ev) in rx {
            if pool.is_none() {
                pool = rt.block_on(crate::db::store::open_pool()).ok();
            }
            let mut item_id = None;
            if let Some(p) = &pool {
                if thread_id > 0 {
                    match rt.block_on(store::apply(p, thread_id, &ev)) {
                        Ok(id) => item_id = id,
                        Err(e) => eprintln!("[oculus] harness store: {e}"),
                    }
                }
                if let Err(e) = rt.block_on(store::save_rate_limits(p, provider, &ev)) {
                    eprintln!("[oculus] harness rate limits: {e}");
                }
                // A finished first exchange is when a thread is named. The
                // claim is atomic (asks at most once); the naming turn runs
                // off this loop, which every thread's events wait behind.
                if thread_id > 0
                    && matches!(&ev, HarnessEvent::TurnFinished { status } if status == "completed")
                {
                    match rt.block_on(store::claim_naming(p, thread_id)) {
                        Ok(Some(seed)) => {
                            let sel = rt.block_on(jobs::selection(p, jobs::Job::ThreadNaming));
                            let (namer, bus) = (namer.clone(), bus.clone());
                            std::thread::spawn(move || {
                                match namer.name_thread(&sel, &seed.first_message, &seed.reply) {
                                    Ok(title) => {
                                        let _ = bus.send((
                                            thread_id,
                                            provider,
                                            HarnessEvent::ThreadTitled { title },
                                        ));
                                    }
                                    Err(e) => eprintln!("[oculus] harness title: {e}"),
                                }
                            });
                        }
                        Ok(None) => {}
                        Err(e) => eprintln!("[oculus] harness title: {e}"),
                    }
                }
            }
            // A closed turn releases the next queued message; its send is
            // spawned, since a send may start a process.
            if thread_id > 0 && matches!(&ev, HarnessEvent::TurnFinished { .. }) {
                let next = queued.lock().unwrap().next(thread_id);
                if let Some((msg, opts)) = next {
                    let (h, b) = (namer.clone(), bus.clone());
                    let _ = b.send((thread_id, provider, HarnessEvent::Unqueued { id: msg.id }));
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = dispatch(h, b, thread_id, provider, opts, msg.text).await {
                            eprintln!("[oculus] harness queued send: {e}");
                        }
                    });
                }
            }

            let _ = handle.emit(
                "harness-event",
                Envelope {
                    thread_id,
                    provider,
                    item_id,
                    event: ev,
                },
            );
        }
    });
    HarnessState {
        harness,
        bus: tx,
        queue,
    }
}

/// Startup: kill opencode servers a previous run was terminated out of
/// (no `shutdown` or `Drop` ran). See [`opencode::sweep`].
pub fn sweep_strays() {
    let killed = opencode::sweep();
    if !killed.is_empty() {
        eprintln!(
            "[oculus] harness: killed {} stray opencode server(s): {killed:?}",
            killed.len()
        );
    }
}

/// Startup: nothing survives a restart as `running`.
pub fn reconcile(app: &AppHandle) {
    let _ = app;
    tauri::async_runtime::spawn(async {
        if let Ok(pool) = crate::db::store::open_pool().await {
            if let Ok(n) = store::reconcile(&pool).await {
                if n > 0 {
                    eprintln!("[oculus] harness: marked {n} interrupted thread(s) idle");
                }
            }
        }
    });
}

pub fn shutdown(app: &AppHandle) {
    if let Some(s) = app.try_state::<HarnessState>() {
        s.harness.shutdown();
    }
}
