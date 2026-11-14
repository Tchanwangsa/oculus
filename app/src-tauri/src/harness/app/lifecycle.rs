use std::sync::{mpsc, Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager};

use crate::harness::{jobs, opencode, store};
use crate::harness::{Harness, HarnessEvent, Provider, Queue};

use super::consumer::ConsumerIo;
use super::send::dispatch;
use super::wake::{Alarm, Wakeful};
use super::{sink_for, BusMsg, Envelope, HarnessState};

/// One consumer thread folds every event, from every thread, in order:
/// a row is written before the webview hears about it, and a tool's
/// finish can never overtake its start.
pub fn init(app: &AppHandle) -> HarnessState {
    let (tx, rx) = mpsc::channel::<BusMsg>();
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
        let alarm = Alarm::default();
        let wakeful = Wakeful {
            rx,
            alarm: alarm.clone(),
        };
        let mut io = AppIo {
            handle,
            namer,
            bus,
            alarm,
        };
        super::consumer::consume(wakeful, queued, &mut io);
    });
    HarnessState {
        harness,
        bus: tx,
        queue,
    }
}

struct AppIo {
    handle: AppHandle,
    namer: Arc<Harness>,
    bus: super::Bus,
    /// When the bus should send the consumer a `Wake` (`wake.rs`).
    alarm: Alarm,
}

impl ConsumerIo for AppIo {
    fn open_pool(&mut self, rt: &tokio::runtime::Runtime) -> Option<sqlx::SqlitePool> {
        rt.block_on(crate::db::store::open_pool()).ok()
    }

    fn name(
        &mut self,
        thread_id: i64,
        provider: Provider,
        chain: Vec<jobs::JobSelection>,
        seed: store::NamingSeed,
    ) {
        let (namer, bus) = (self.namer.clone(), self.bus.clone());
        std::thread::spawn(move || {
            match jobs::try_each(jobs::Job::ThreadNaming, &chain, |sel| {
                namer.name_thread(sel, &seed.first_message, &seed.reply)
            }) {
                Ok(title) => {
                    let _ = bus.send(BusMsg::app(
                        thread_id,
                        provider,
                        HarnessEvent::ThreadTitled { title },
                    ));
                }
                Err(e) => elog!("[oculus] harness title: {e}"),
            }
        });
    }

    fn dispatch(
        &mut self,
        thread_id: i64,
        provider: Provider,
        msg: crate::harness::QueuedMessage,
        opts: crate::harness::SendOptions,
    ) {
        let (h, b) = (self.namer.clone(), self.bus.clone());
        // A continuation was never shown as queued.
        if opts.continuation.is_none() {
            let _ = b.send(BusMsg::app(
                thread_id,
                provider,
                HarnessEvent::Unqueued { id: msg.id },
            ));
        }
        tauri::async_runtime::spawn(async move {
            if let Err(e) = dispatch(h, b, thread_id, provider, opts, msg.text).await {
                elog!("[oculus] harness queued send: {e}");
            }
        });
    }

    fn emit(&mut self, envelope: Envelope) {
        let _ = self.handle.emit("harness-event", envelope);
    }

    fn wake_at(&mut self, at: std::time::Instant) {
        *self.alarm.lock().unwrap() = Some(at);
    }
}

/// Startup: kill opencode servers a previous run was terminated out of
/// (no `shutdown` or `Drop` ran). See [`opencode::sweep`].
pub fn sweep_strays() {
    let killed = opencode::sweep();
    if !killed.is_empty() {
        elog!(
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
            if let Err(error) = super::proposal_jobs::reconcile(&pool).await {
                elog!("[oculus] proposal jobs: {error}");
            }
            if let Err(error) = super::continuation::reconcile(&pool).await {
                elog!("[oculus] card continuations: {error}");
            }
            if let Ok(r) = store::reconcile(&pool).await {
                if r.threads > 0 || r.tools > 0 {
                    elog!(
                        "[oculus] harness: marked {} interrupted thread(s) idle, closed {} open tool row(s)",
                        r.threads,
                        r.tools
                    );
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
