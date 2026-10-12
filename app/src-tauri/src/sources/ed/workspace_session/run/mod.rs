//! Run: the challenge's `run_command` on Ed's server at the workspace's
//! latest commit (`snapshot`), on a socket of its own
//! (`wss://<host>/run?ticket=`) served by its own thread (`stream`). One run
//! per challenge: a new Run stops the one before it.

pub mod frames;
pub mod stream;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, MutexGuard};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;

use self::stream::{RunEvent, RunOut};
use super::context::Context;
use super::events::{Body, RunStarted};
use super::queue::Place;
use super::session::Session;
use super::snapshot;
use super::terminal::PtySize;
use crate::sources::ed::live::{open_socket, set_read_timeout};

/// How long one read on the run socket waits before the queue is looked at.
const TURN: std::time::Duration = std::time::Duration::from_millis(100);

/// The challenge's run, while there is one.
#[derive(Default)]
pub struct Runs {
    current: Mutex<Option<Current>>,
    /// Held while a run starts, so two Runs cannot both start one.
    starting: Mutex<()>,
    next: AtomicU32,
}

struct Current {
    id: String,
    queue: Sender<RunOut>,
}

impl Runs {
    fn current(&self) -> MutexGuard<'_, Option<Current>> {
        self.current.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Stop the run in progress, if any; its thread reports the exit.
    pub fn stop_current(&self) {
        if let Some(run) = self.current().take() {
            let _ = run.queue.send(RunOut::Stop);
        }
    }

    fn stop(&self, run_id: &str) {
        let mut current = self.current();
        if current.as_ref().is_some_and(|run| run.id == run_id) {
            if let Some(run) = current.take() {
                let _ = run.queue.send(RunOut::Stop);
            }
        }
    }

    fn send(&self, run_id: &str, frame: String) -> Result<(), String> {
        match &*self.current() {
            Some(run) if run.id == run_id => run
                .queue
                .send(RunOut::Frame(frame))
                .map_err(|_| "the run has ended".to_string()),
            _ => Err("the run has ended".into()),
        }
    }

    /// Forget run `run_id` when it is still the current one.
    fn finished(&self, run_id: &str) {
        let mut current = self.current();
        if current.as_ref().is_some_and(|run| run.id == run_id) {
            *current = None;
        }
    }
}

impl Session {
    /// Start a run of `run_command` in a `size` terminal, after the saves
    /// asked before it have landed. Its output and exit arrive as events.
    pub fn run(&self, place: Place, size: PtySize) -> Result<RunStarted, String> {
        let runs = self.runs.clone();
        let _starting = runs.starting.lock().unwrap_or_else(|e| e.into_inner());
        runs.stop_current();
        let command = self
            .slide
            .run_command
            .clone()
            .ok_or("This challenge has no run command: use the terminal")?;
        drop(place.wait());
        let (_, host) = self
            .core
            .online()
            .ok_or("The workspace is offline: Run needs Ed's server")?;
        let hash = snapshot::latest(&self.core, &self.out)?;
        let ticket = self.core.ed.run_ticket(
            self.core.challenge_id,
            &hash,
            &command,
            (size.cols, size.rows),
        )?;
        let mut socket = open_socket(&host, "run", ticket.expose())?;
        set_read_timeout(&mut socket, TURN)?;

        let run_id = format!("run-{}", runs.next.fetch_add(1, Ordering::SeqCst) + 1);
        let (queue, from_commands) = mpsc::channel();
        *runs.current() = Some(Current {
            id: run_id.clone(),
            queue,
        });
        let core = self.core.clone();
        let id = run_id.clone();
        let thread_runs = runs.clone();
        let spawned = std::thread::Builder::new()
            .name(format!("ed-run-{}", self.core.challenge_id))
            .spawn(move || {
                stream::serve(&mut socket, &from_commands, &|event| {
                    emit(&core, &id, event)
                });
                thread_runs.finished(&id);
            });
        if let Err(e) = spawned {
            runs.finished(&run_id);
            return Err(format!("could not start the run: {e}"));
        }
        Ok(RunStarted { run_id })
    }

    /// Keystrokes for the run's program.
    pub fn run_input(&self, run_id: &str, text: &str) -> Result<(), String> {
        let data = BASE64.encode(text.as_bytes());
        self.runs.send(run_id, frames::stdin_frame(&data))
    }

    pub fn run_resize(&self, run_id: &str, size: PtySize) -> Result<(), String> {
        self.runs
            .send(run_id, frames::size_frame(size.cols, size.rows))
    }

    /// Stop run `run_id`; a run that already ended is left as it is.
    pub fn run_stop(&self, run_id: &str) {
        self.runs.stop(run_id);
    }
}

fn emit(core: &Context, run_id: &str, event: RunEvent) {
    let run_id = run_id.to_string();
    core.emit(match event {
        RunEvent::Output(data) => Body::Run { run_id, data },
        RunEvent::Exit(exit) => Body::RunExit {
            run_id,
            code: exit.code,
            signal: exit.signal,
            message: exit.message,
        },
    });
}
