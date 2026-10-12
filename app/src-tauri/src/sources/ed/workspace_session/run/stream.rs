//! One run's thread: it owns the run socket, sends what the commands queue
//! (stdin, a resize, stop) and reads for at most a turn between. It ends
//! after reporting exactly one exit, however the run ends.

use std::io::{ErrorKind, Read, Write};
use std::sync::mpsc::{Receiver, TryRecvError};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::{Message, WebSocket};

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
    loop {
        loop {
            match queue.try_recv() {
                Ok(RunOut::Frame(frame)) => {
                    if let Err(e) = socket.send(Message::text(frame)) {
                        return Exit::because(&socket_error(&e));
                    }
                }
                Ok(RunOut::Stop) | Err(TryRecvError::Disconnected) => {
                    return Exit::because(frames::STOPPED)
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
                Some(RunIncoming::Exit(exit)) => return exit,
                None => {}
            },
            Ok(Message::Close(close)) => {
                let busy = close.is_some_and(|c| c.code == CloseCode::from(frames::NO_CAPACITY));
                return Exit::because(if busy {
                    frames::NO_RESOURCES
                } else {
                    frames::CUT_OFF
                });
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                let _ = socket.flush();
            }
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                return Exit::because(frames::CUT_OFF)
            }
            Err(e) => return Exit::because(&socket_error(&e)),
        }
    }
}
