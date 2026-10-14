//! One run's thread: it owns the run socket, sends what the commands queue
//! (stdin, a resize, stop) and reads for at most a turn between. Ed sends
//! `dump_files` after `run_exit`, so the run ends once both are in, or the
//! socket closes. It reports exactly one exit, however the run ends.

use std::io::{ErrorKind, Read, Write};
use std::sync::mpsc::{Receiver, TryRecvError};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::{Message, WebSocket};

use super::dump::{self, Dumped};
use super::frames::{self, Exit, RunIncoming};
use crate::sources::ed::live::socket_error;

/// What the commands hand a run's thread.
pub enum RunOut {
    Frame(String),
    Stop,
}

/// What a run tells the pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunEvent {
    /// Base64 of the program's (or the build's) output.
    Output(String),
    /// The output files `dump_files` named, before the exit.
    Files(Vec<Dumped>),
    Exit(Exit),
}

/// Serve the run until it ends. `socket`'s reads must already be bounded,
/// so the queue is looked at between them.
pub fn serve<S: Read + Write>(
    socket: &mut WebSocket<S>,
    queue: &Receiver<RunOut>,
    emit: &dyn Fn(RunEvent),
) {
    let exit = serve_until_exit(socket, queue, emit);
    let _ = socket.close(None);
    let _ = socket.flush();
    emit(RunEvent::Exit(exit));
}

fn serve_until_exit<S: Read + Write>(
    socket: &mut WebSocket<S>,
    queue: &Receiver<RunOut>,
    emit: &dyn Fn(RunEvent),
) -> Exit {
    // The program's exit, held until `dump_files` follows it.
    let mut exited: Option<Exit> = None;
    let mut dumped = false;
    let end = |exited: Option<Exit>, why: &str| exited.unwrap_or_else(|| Exit::because(why));
    loop {
        loop {
            match queue.try_recv() {
                Ok(RunOut::Frame(frame)) => {
                    if let Err(e) = socket.send(Message::text(frame)) {
                        return end(exited, &socket_error(&e));
                    }
                }
                Ok(RunOut::Stop) | Err(TryRecvError::Disconnected) => {
                    return end(exited, frames::STOPPED)
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        match socket.read() {
            Ok(Message::Text(text)) => match frames::parse(text.as_str()) {
                Some(RunIncoming::Output(data)) => emit(RunEvent::Output(data)),
                Some(RunIncoming::Built { ok, output }) => {
                    if !output.is_empty() {
                        emit(RunEvent::Output(BASE64.encode(output.as_bytes())));
                    }
                    if !ok {
                        return Exit::because(frames::BUILD_FAILED);
                    }
                }
                Some(RunIncoming::Exit(exit)) if dumped => return exit,
                Some(RunIncoming::Exit(exit)) => exited = Some(exit),
                Some(RunIncoming::Files(data)) => {
                    let files = dump::decode(&data);
                    if !files.is_empty() {
                        emit(RunEvent::Files(files));
                    }
                    dumped = true;
                    if let Some(exit) = exited.take() {
                        return exit;
                    }
                }
                None => {}
            },
            Ok(Message::Close(close)) => {
                let busy = close.is_some_and(|c| c.code == CloseCode::from(frames::NO_CAPACITY));
                let why = if busy {
                    frames::NO_RESOURCES
                } else {
                    frames::CUT_OFF
                };
                return end(exited, why);
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                let _ = socket.flush();
            }
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                return end(exited, frames::CUT_OFF)
            }
            Err(e) => return end(exited, &socket_error(&e)),
        }
    }
}
