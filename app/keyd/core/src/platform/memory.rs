//! An in-memory endpoint for core's tests: a duplex `Conn` pair with the
//! same timeout and end-of-stream behaviour as a socket, and a `Listener`
//! fed by a `Connector`. It lets the server loop and the client be tested on
//! any target, with no OS endpoint at all.

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use super::{Accept, Conn, Listener, Stream};

#[derive(Default)]
struct State {
    data: VecDeque<u8>,
    /// The writer went away (reads end) or the reader did (writes fail).
    closed: bool,
}

/// One direction of the duplex.
#[derive(Default)]
struct Pipe {
    state: Mutex<State>,
    ready: Condvar,
}

impl Pipe {
    fn close(&self) {
        self.state.lock().unwrap().closed = true;
        self.ready.notify_all();
    }
}

pub(crate) struct Half {
    rx: Arc<Pipe>,
    tx: Arc<Pipe>,
    timeout: Mutex<Option<Duration>>,
}

impl Half {
    pub(crate) fn set_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        *self.timeout.lock().unwrap() = timeout;
        Ok(())
    }

    pub(crate) fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let deadline = self.timeout.lock().unwrap().map(|t| Instant::now() + t);
        let mut state = self.rx.state.lock().unwrap();
        while state.data.is_empty() && !state.closed {
            state = match deadline {
                None => self.rx.ready.wait(state).unwrap(),
                Some(at) => {
                    let left = at.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Err(io::Error::new(io::ErrorKind::WouldBlock, "timed out"));
                    }
                    self.rx.ready.wait_timeout(state, left).unwrap().0
                }
            };
        }
        let n = buf.len().min(state.data.len());
        for (slot, byte) in buf.iter_mut().zip(state.data.drain(..n)) {
            *slot = byte;
        }
        Ok(n)
    }

    pub(crate) fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut state = self.tx.state.lock().unwrap();
        if state.closed {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "the peer closed"));
        }
        state.data.extend(buf);
        self.tx.ready.notify_all();
        Ok(buf.len())
    }

    pub(crate) fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for Half {
    fn drop(&mut self) {
        self.tx.close();
        self.rx.close();
    }
}

/// Two connected ends.
pub(crate) fn pair() -> (Conn, Conn) {
    let (a, b) = (Arc::new(Pipe::default()), Arc::new(Pipe::default()));
    let one = Half { rx: a.clone(), tx: b.clone(), timeout: Mutex::new(None) };
    let other = Half { rx: b, tx: a, timeout: Mutex::new(None) };
    (Conn(Stream::Memory(one)), Conn(Stream::Memory(other)))
}

/// Connections wait in the channel until accepted, as in a socket's backlog.
pub(crate) struct Acceptor(Mutex<Receiver<Conn>>);

impl Acceptor {
    /// Once every `Connector` is gone, waits out `within` (for ever when
    /// `None`), as a listener nobody connects to would.
    pub(crate) fn accept_within(&self, within: Option<Duration>) -> io::Result<Option<Conn>> {
        let queue = self.0.lock().unwrap();
        let next = match within {
            None => queue.recv().map_err(|_| RecvTimeoutError::Disconnected),
            Some(t) => queue.recv_timeout(t),
        };
        match next {
            Ok(conn) => Ok(Some(conn)),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => match within {
                Some(t) => {
                    std::thread::sleep(t);
                    Ok(None)
                }
                None => loop {
                    std::thread::park();
                },
            },
        }
    }
}

/// Opens connections to its `Listener`.
#[derive(Clone)]
pub(crate) struct Connector(Sender<Conn>);

impl Connector {
    pub(crate) fn connect(&self) -> Conn {
        let (client, server) = pair();
        self.0.send(server).expect("the listener is gone");
        client
    }
}

pub(crate) fn listener() -> (Listener, Connector) {
    let (tx, rx) = channel();
    (Listener(Accept::Memory(Acceptor(Mutex::new(rx)))), Connector(tx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_cross_both_ways_and_a_dropped_end_is_eof() {
        let (mut a, mut b) = pair();
        a.write_all(b"ping").unwrap();
        let mut buf = [0u8; 4];
        b.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"ping");
        b.write_all(b"pong").unwrap();
        drop(b);
        let mut rest = Vec::new();
        a.read_to_end(&mut rest).unwrap();
        assert_eq!(rest, b"pong");
        assert!(a.write_all(b"x").is_err(), "the peer is gone");
    }

    #[test]
    fn a_read_past_its_timeout_fails() {
        let (a, _b) = pair();
        let mut a = a;
        a.set_timeout(Some(Duration::from_millis(50))).unwrap();
        let started = Instant::now();
        let err = a.read(&mut [0u8; 1]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::WouldBlock);
        assert!(started.elapsed() >= Duration::from_millis(50));
    }
}
