//! The session's thread. It owns the workspace socket: each turn it sends
//! what the commands queued, a `keep_alive` when one is due, then reads for
//! at most [`TURN`]. When the socket drops it goes offline and connects again
//! with a fresh ticket, waiting longer after each failure. Ed's own server
//! decides when a session is dead; nothing here times one out.

use std::io::ErrorKind;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tungstenite::Message;

use super::bootstrap;
use super::context::Context;
use super::events::{Body, State};
use super::frames::{self, Incoming};
use crate::sources::ed::live::{
    open_socket, set_read_timeout, socket_error, wait_for_init, Socket,
};

/// How long one read waits before the thread looks at its queue again.
const TURN: Duration = Duration::from_millis(200);
/// The web app's `keepOpen` interval.
const KEEP_ALIVE: Duration = Duration::from_secs(270);
const FIRST_RETRY: Duration = Duration::from_secs(2);
const LAST_RETRY: Duration = Duration::from_secs(60);

/// What the commands hand the thread.
pub enum Out {
    /// A text frame to send while online; dropped while offline.
    Frame(String),
    Stop,
}

enum Ended {
    Stopped,
    Lost { reason: String, was_online: bool },
}

pub fn run(core: Arc<Context>, queue: Receiver<Out>) {
    let mut retry = FIRST_RETRY;
    loop {
        match connect_and_serve(&core, &queue) {
            Ended::Stopped => return,
            Ended::Lost { reason, was_online } => {
                elog!("[oculus] ed workspace {}: {reason}", core.challenge_id);
                core.set_status(State::Offline, Some(reason));
                if was_online {
                    retry = FIRST_RETRY;
                }
            }
        }
        if !pause(&queue, retry) {
            return;
        }
        retry = (retry * 2).min(LAST_RETRY);
        core.set_status(State::Connecting, None);
    }
}

/// Wait out `wait`, still answering `Stop`. False when the session ended.
fn pause(queue: &Receiver<Out>, wait: Duration) -> bool {
    let until = Instant::now() + wait;
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return true;
        }
        match queue.recv_timeout(left) {
            Ok(Out::Stop) | Err(RecvTimeoutError::Disconnected) => return false,
            Ok(Out::Frame(_)) => {}
            Err(RecvTimeoutError::Timeout) => return true,
        }
    }
}

fn connect_and_serve(core: &Context, queue: &Receiver<Out>) -> Ended {
    let lost = |reason: String| Ended::Lost {
        reason,
        was_online: false,
    };
    let host = match core.ed.workspace_host() {
        Ok(host) => host,
        Err(reason) => return lost(reason),
    };
    let connected = match core.ed.connect(core.challenge_id) {
        Ok(connected) => connected,
        Err(reason) => return lost(reason),
    };
    let mut socket = match open_socket(&host, "connect", connected.ticket.expose()) {
        Ok(socket) => socket,
        Err(reason) => return lost(reason),
    };
    let init = match wait_for_init(&mut socket).and_then(|init| {
        set_read_timeout(&mut socket, TURN)?;
        Ok(frames::init(init))
    }) {
        Ok(init) => init,
        Err(reason) => return lost(reason),
    };
    let wid = connected.workspace_id;
    let pending = match bootstrap::on_connect(core, &wid, &host, &init) {
        Ok(pending) => pending,
        Err(reason) => {
            close(&mut socket);
            return lost(reason);
        }
    };
    core.shared().online = Some((wid, host));
    core.set_status(State::Online, None);
    for frame in pending {
        if let Err(e) = socket.send(Message::text(frame)) {
            return Ended::Lost {
                reason: socket_error(&e),
                was_online: true,
            };
        }
    }
    serve(core, &mut socket, queue)
}

fn serve(core: &Context, socket: &mut Socket, queue: &Receiver<Out>) -> Ended {
    let dropped = |reason: String| Ended::Lost {
        reason,
        was_online: true,
    };
    let mut kept_alive = Instant::now();
    loop {
        loop {
            match queue.try_recv() {
                Ok(Out::Frame(frame)) => {
                    if let Err(e) = socket.send(Message::text(frame)) {
                        return dropped(socket_error(&e));
                    }
                }
                Ok(Out::Stop) | Err(TryRecvError::Disconnected) => {
                    close(socket);
                    return Ended::Stopped;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        if kept_alive.elapsed() >= KEEP_ALIVE {
            if let Err(e) = socket.send(Message::text(frames::keep_alive())) {
                return dropped(socket_error(&e));
            }
            kept_alive = Instant::now();
        }
        match socket.read() {
            Ok(Message::Text(text)) => {
                if let Some(frame) = frames::parse(text.as_str()) {
                    handle(core, frame);
                }
            }
            Ok(Message::Close(_)) => {
                return dropped("the workspace server ended the session".into());
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                // A pong tungstenite queued goes out on the next write; nudge it.
                let _ = socket.flush();
            }
            Err(e) => return dropped(socket_error(&e)),
        }
    }
}

/// One frame from the server. Terminal frames (`program_*`) wait for the
/// terminal; another client's file changes wait for reconcile.
fn handle(core: &Context, frame: Incoming) {
    match frame {
        Incoming::NewCommit(commit) => {
            let decided = core.shared().commits.arrived(&commit.commit.hash);
            if let Some((hash, ours)) = decided {
                core.emit(Body::Commit { hash, ours });
            }
        }
        Incoming::DisplayError(message) => core.emit(Body::Error { message }),
        _ => {}
    }
}

fn close(socket: &mut Socket) {
    let _ = socket.close(None);
    let _ = socket.flush();
}
