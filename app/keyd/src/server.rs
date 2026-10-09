//! The accept loop and the wire format.
//!
//! A request is one JSON line, then `body_len` raw bytes when the header has
//! that field; a reply has the same shape. A connection may carry several
//! requests in turn. keyd exits after `idle` with no request in flight; a
//! connection counts as busy from a complete header line until its reply is
//! written, so a `forward` waiting on its origin holds keyd open, but a
//! client that connects and stalls cannot. No read timeouts: a forwarded
//! parse or embed takes minutes.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{FromRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::caller::{self, Policy};
use crate::log;
use crate::ops::{OpError, Reply, State};

/// Longest header line; a longer one is refused and the connection closed.
pub const MAX_LINE: usize = 64 * 1024;
/// Largest body keyd will buffer.
pub const MAX_BODY: u64 = 256 * 1024 * 1024;

pub struct Server {
    pub state: State,
    pub policy: Policy,
    pub idle: Duration,
    busy: AtomicUsize,
    /// Milliseconds since the epoch of the last accept or reply.
    last: AtomicU64,
}

/// Decrements `busy` however the request ends, panics included.
struct Busy<'a>(&'a Server);

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.busy.fetch_sub(1, Ordering::SeqCst);
        self.0.touch();
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl Server {
    pub fn new(state: State, policy: Policy, idle: Duration) -> Self {
        Server { state, policy, idle, busy: AtomicUsize::new(0), last: AtomicU64::new(now_ms()) }
    }

    fn touch(&self) {
        self.last.store(now_ms(), Ordering::SeqCst);
    }

    fn idle_for(&self) -> Duration {
        Duration::from_millis(now_ms().saturating_sub(self.last.load(Ordering::SeqCst)))
    }

    /// Accepts on `listeners` (non-blocking, as launchd hands them over) until
    /// keyd has been idle for `self.idle`, then returns.
    pub fn run(self: Arc<Self>, listeners: &[RawFd]) {
        loop {
            let mut pfds: Vec<libc::pollfd> = listeners.iter().map(|&fd| libc::pollfd { fd, events: libc::POLLIN, revents: 0 }).collect();
            let n = unsafe { libc::poll(pfds.as_mut_ptr(), pfds.len() as libc::nfds_t, 1000) };
            if n < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                log(&format!("poll failed: {e}"));
                return;
            }
            if n == 0 {
                if self.busy.load(Ordering::SeqCst) == 0 && self.idle_for() >= self.idle {
                    log(&format!("idle for {}s, exiting", self.idle.as_secs()));
                    return;
                }
                continue;
            }
            for p in pfds.iter().filter(|p| p.revents & libc::POLLIN != 0) {
                let conn = unsafe { libc::accept(p.fd, std::ptr::null_mut(), std::ptr::null_mut()) };
                if conn < 0 {
                    let e = std::io::Error::last_os_error();
                    if e.kind() != std::io::ErrorKind::WouldBlock {
                        log(&format!("accept failed: {e}"));
                    }
                    continue;
                }
                // accept() inherits O_NONBLOCK from the listener.
                unsafe {
                    let fl = libc::fcntl(conn, libc::F_GETFL);
                    libc::fcntl(conn, libc::F_SETFL, fl & !libc::O_NONBLOCK);
                }
                self.touch();
                let server = self.clone();
                std::thread::spawn(move || server.connection(conn));
            }
        }
    }

    fn connection(&self, fd: RawFd) {
        // The caller check runs before a byte of the request is read.
        let caller = caller::inspect(fd);
        let verdict = caller::admit(self.policy, &caller);
        let stream = unsafe { UnixStream::from_raw_fd(fd) };
        let mut reader = BufReader::new(&stream);
        let mut writer = &stream;
        let who = caller.label();

        loop {
            let line = match read_line(&mut reader) {
                Ok(Some(line)) => line,
                Ok(None) => return,
                Err(e) => {
                    log(&format!("caller={who} closed: {}", e.detail));
                    write_frame(&mut writer, &e.to_json(), &[]).ok();
                    return;
                }
            };
            let _busy = {
                self.busy.fetch_add(1, Ordering::SeqCst);
                Busy(self)
            };

            // A refused caller gets its reply without keyd reading the body.
            let parsed = parse_header(&line);
            // Client text, so only a plain word reaches the log.
            let op = match &parsed {
                Ok((op, _, _)) if !op.is_empty() && op.len() <= 32 && op.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') => op.clone(),
                _ => "?".to_string(),
            };
            let mut in_step = false;
            let reply = match (&verdict, parsed) {
                (Err(why), _) => Err(OpError::new("caller", why.clone())),
                (Ok(()), Err(e)) => Err(e),
                (Ok(()), Ok((op, req, body_len))) => match read_body(&mut reader, body_len) {
                    Err(e) => Err(e),
                    Ok(body) => {
                        in_step = true;
                        self.state.dispatch(&caller, &op, &req, &body)
                    }
                },
            };

            let (reply, outcome) = match reply {
                Ok(r) => {
                    let outcome = match &r.note {
                        Some(note) => format!("ok {note}"),
                        None => "ok".to_string(),
                    };
                    (r, outcome)
                }
                Err(e) => (Reply::from(e.to_json()), format!("error={}", e.kind)),
            };
            let admitted = if verdict.is_ok() { "admitted" } else { "refused" };
            match &verdict {
                Err(why) => log(&format!("op={op} caller={who} {admitted} ({why}) {outcome}")),
                Ok(()) => log(&format!("op={op} caller={who} {admitted} {outcome}")),
            }
            if let Err(e) = write_frame(&mut writer, &reply.header, &reply.body) {
                log(&format!("op={op} caller={who} reply failed: {e}"));
                return;
            }
            // A refused caller, or a stream whose framing is lost, gets one reply.
            if !in_step {
                return;
            }
        }
    }
}

/// One header line, without its newline. `Ok(None)` is a clean end of stream
/// between requests.
fn read_line(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, OpError> {
    let mut line = Vec::new();
    loop {
        let buf = reader.fill_buf().map_err(|e| OpError::new("request", format!("read: {e}")))?;
        if buf.is_empty() {
            return if line.is_empty() { Ok(None) } else { Err(OpError::new("request", "the stream ended mid-line")) };
        }
        let (take, done) = match buf.iter().position(|&b| b == b'\n') {
            Some(i) => (i + 1, true),
            None => (buf.len(), false),
        };
        if line.len() + take > MAX_LINE + 1 {
            return Err(OpError::new("request", format!("the header line is longer than {MAX_LINE} bytes")));
        }
        line.extend_from_slice(&buf[..take]);
        reader.consume(take);
        if done {
            line.pop();
            return Ok(Some(line));
        }
    }
}

/// The op name, the whole header, and the body length it announces.
fn parse_header(line: &[u8]) -> Result<(String, Value, u64), OpError> {
    // serde's message is not echoed: it can quote the request, which may hold a value.
    let req: Value = serde_json::from_slice(line).map_err(|_| OpError::new("request", "the header line is not JSON"))?;
    if !req.is_object() {
        return Err(OpError::new("request", "the header line is not a JSON object"));
    }
    let op = req.get("op").and_then(Value::as_str).ok_or_else(|| OpError::new("request", "missing \"op\""))?.to_string();
    let body_len = match req.get("body_len") {
        None => 0,
        Some(v) => v.as_u64().ok_or_else(|| OpError::new("request", "body_len is not a length"))?,
    };
    if body_len > MAX_BODY {
        return Err(OpError::new("request", format!("body_len is over {MAX_BODY} bytes")));
    }
    Ok((op, req, body_len))
}

fn read_body(reader: &mut impl Read, len: u64) -> Result<Vec<u8>, OpError> {
    let mut body = Vec::new();
    reader.take(len).read_to_end(&mut body).map_err(|e| OpError::new("request", format!("read: {e}")))?;
    if body.len() as u64 != len {
        return Err(OpError::new("request", "the stream ended inside the body"));
    }
    Ok(body)
}

/// The header line, then the body unbuffered: a body can be tens of MB.
fn write_frame(w: &mut impl Write, header: &Value, body: &[u8]) -> std::io::Result<()> {
    let mut header = header.clone();
    if !body.is_empty() {
        header["body_len"] = json!(body.len());
    }
    let mut line = header.to_string().into_bytes();
    line.push(b'\n');
    w.write_all(&line)?;
    w.write_all(body)?;
    w.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forward::Routes;
    use crate::ops::NoLegacy;
    use crate::test_support::{Answer, FakeOrigin, Scratch};
    use std::os::fd::IntoRawFd;
    use std::os::unix::net::UnixListener;
    use vault::{KeyError, KeySource, MasterKey, StaticKey, Vault};

    fn serve(policy: Policy, idle: Duration) -> (Scratch, std::path::PathBuf, std::thread::JoinHandle<()>) {
        serve_with(policy, idle, Box::new(StaticKey(MasterKey::from_bytes([3; 32]))))
    }

    fn serve_with(policy: Policy, idle: Duration, keys: Box<dyn KeySource>) -> (Scratch, std::path::PathBuf, std::thread::JoinHandle<()>) {
        serve_state(policy, idle, |dir| State::new(dir.to_path_buf(), keys, Box::new(NoLegacy)))
    }

    fn serve_state(policy: Policy, idle: Duration, state: impl FnOnce(&std::path::Path) -> State) -> (Scratch, std::path::PathBuf, std::thread::JoinHandle<()>) {
        let dir = Scratch::new("srv");
        let sock = dir.0.join("k.sock");
        let listener = UnixListener::bind(&sock).unwrap();
        listener.set_nonblocking(true).unwrap();
        let fd = listener.into_raw_fd();
        let state = state(&dir.0);
        let server = Arc::new(Server::new(state, policy, idle));
        let handle = std::thread::spawn(move || server.run(&[fd]));
        (dir, sock, handle)
    }

    fn call(stream: &UnixStream, reader: &mut BufReader<&UnixStream>, req: &Value) -> Value {
        let mut w = stream;
        w.write_all(format!("{req}\n").as_bytes()).unwrap();
        let line = read_line(reader).unwrap().unwrap();
        serde_json::from_slice(&line).unwrap()
    }

    #[test]
    fn ops_run_in_turn_over_one_connection() {
        let (_dir, sock, _h) = serve(Policy::SameUser, Duration::from_secs(60));
        let stream = UnixStream::connect(&sock).unwrap();
        let mut r = BufReader::new(&stream);
        assert_eq!(call(&stream, &mut r, &json!({"op": "ping"}))["pid"], std::process::id());
        assert_eq!(call(&stream, &mut r, &json!({"op": "store", "secret": "mineru", "value": "tok"}))["stored"], true);
        assert_eq!(call(&stream, &mut r, &json!({"op": "has", "secret": "mineru"}))["has"], true);
        assert_eq!(call(&stream, &mut r, &json!({"op": "nope"}))["error"], "request");
        assert_eq!(call(&stream, &mut r, &json!({"op": "delete", "secret": "mineru"}))["existed"], true);
    }

    #[test]
    fn a_body_after_the_header_is_read_with_the_same_reader() {
        let (_dir, sock, _h) = serve(Policy::SameUser, Duration::from_secs(60));
        let stream = UnixStream::connect(&sock).unwrap();
        let mut r = BufReader::new(&stream);
        // Header, body and the next request in one write: none of it may be lost.
        let mut w = &stream;
        w.write_all(b"{\"op\":\"ping\",\"body_len\":5}\nhello{\"op\":\"ping\"}\n").unwrap();
        let first: Value = serde_json::from_slice(&read_line(&mut r).unwrap().unwrap()).unwrap();
        assert_eq!(first["error"], "request", "ping takes no body");
        let second: Value = serde_json::from_slice(&read_line(&mut r).unwrap().unwrap()).unwrap();
        assert_eq!(second["pid"], std::process::id());
    }

    #[test]
    fn an_overlong_header_line_is_refused_and_closed() {
        let (_dir, sock, _h) = serve(Policy::SameUser, Duration::from_secs(60));
        let stream = UnixStream::connect(&sock).unwrap();
        let mut w = &stream;
        let long = vec![b'a'; MAX_LINE + 10];
        // The server stops reading partway, so this write may fail; the reply still comes.
        w.write_all(&long).ok();
        let mut r = BufReader::new(&stream);
        let reply: Value = serde_json::from_slice(&read_line(&mut r).unwrap().unwrap()).unwrap();
        assert_eq!(reply["error"], "request");
        assert!(read_line(&mut r).unwrap().is_none(), "the connection is closed");
    }

    #[test]
    fn a_refused_caller_gets_one_error_and_no_op_runs() {
        // The test binary is in no app bundle, so the bundle policy refuses it.
        let (dir, sock, _h) = serve(Policy::Bundle, Duration::from_secs(60));
        let stream = UnixStream::connect(&sock).unwrap();
        let mut r = BufReader::new(&stream);
        let reply = call(&stream, &mut r, &json!({"op": "store", "secret": "groq", "value": "gsk"}));
        assert_eq!(reply["error"], "caller");
        assert!(!vault::path_in(&dir.0).exists(), "the op never ran");
        assert!(read_line(&mut r).unwrap().is_none());
    }

    #[test]
    fn a_stalled_connection_does_not_hold_keyd_open() {
        let (_dir, sock, handle) = serve(Policy::SameUser, Duration::from_secs(1));
        let _stalled = UnixStream::connect(&sock).unwrap();
        let started = std::time::Instant::now();
        handle.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    /// Stands in for a keychain prompt the user takes a while to answer.
    struct Slow;

    impl KeySource for Slow {
        fn get_or_create(&self) -> Result<MasterKey, KeyError> {
            std::thread::sleep(Duration::from_millis(2500));
            Ok(MasterKey::from_bytes([4; 32]))
        }
    }

    #[test]
    fn a_request_in_flight_holds_keyd_open_past_the_idle_window() {
        let (_dir, sock, handle) = serve_with(Policy::SameUser, Duration::from_secs(1), Box::new(Slow));
        let stream = UnixStream::connect(&sock).unwrap();
        let mut r = BufReader::new(&stream);
        assert_eq!(call(&stream, &mut r, &json!({"op": "has", "secret": "voyage"}))["has"], false);
        assert!(!handle.is_finished(), "keyd answered, then idles from the reply");
        handle.join().unwrap();
    }

    /// Serves `origin` as Voyage's, with a key already stored.
    fn serve_forwarding(idle: Duration, origin: &FakeOrigin) -> (Scratch, std::path::PathBuf, std::thread::JoinHandle<()>) {
        let origin = origin.origin.clone();
        serve_state(Policy::SameUser, idle, move |dir| {
            let state = State::new(dir.to_path_buf(), Box::new(StaticKey(MasterKey::from_bytes([3; 32]))), Box::new(NoLegacy))
                .with_routes(Routes::compiled().with_origin("voyage", &origin).unwrap());
            Vault::new(vault::path_in(dir), MasterKey::from_bytes([3; 32])).store("voyage", "pa-KEY").unwrap();
            state
        })
    }

    /// One request with a body; the reply's header and body.
    fn exchange(stream: &UnixStream, reader: &mut BufReader<&UnixStream>, req: &Value, body: &[u8]) -> (Value, Vec<u8>) {
        let mut w = stream;
        w.write_all(format!("{req}\n").as_bytes()).unwrap();
        w.write_all(body).unwrap();
        let header: Value = serde_json::from_slice(&read_line(reader).unwrap().unwrap()).unwrap();
        let len = header.get("body_len").and_then(Value::as_u64).unwrap_or(0);
        (header, read_body(reader, len).unwrap())
    }

    fn forward_req(body_len: usize) -> Value {
        json!({"op": "forward", "secret": "voyage", "method": "POST", "path": "/v1/multimodalembeddings",
               "headers": [["Content-Type", "application/json"]], "body_len": body_len})
    }

    #[test]
    fn a_forward_carries_bodies_both_ways_and_the_connection_goes_on() {
        let origin = FakeOrigin::start(|hit| Answer { status: 200, headers: vec![], body: hit.body.iter().rev().copied().collect() });
        let (_dir, sock, _h) = serve_forwarding(Duration::from_secs(60), &origin);
        let stream = UnixStream::connect(&sock).unwrap();
        let mut r = BufReader::new(&stream);
        let big: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        let (header, body) = exchange(&stream, &mut r, &forward_req(big.len()), &big);
        assert_eq!(header["status"], 200);
        assert_eq!(body, big.iter().rev().copied().collect::<Vec<_>>());
        assert_eq!(origin.hits()[0].body, big);
        // Framing held: the next request on the same connection is answered.
        assert_eq!(call(&stream, &mut r, &json!({"op": "has", "secret": "voyage"}))["has"], true);
    }

    #[test]
    fn a_forward_waiting_on_its_origin_holds_keyd_open_past_the_idle_window() {
        let origin = FakeOrigin::start(|_| {
            std::thread::sleep(Duration::from_millis(2500));
            Answer { status: 200, headers: vec![], body: b"late".to_vec() }
        });
        let (_dir, sock, handle) = serve_forwarding(Duration::from_secs(1), &origin);
        let stream = UnixStream::connect(&sock).unwrap();
        let mut r = BufReader::new(&stream);
        let (header, body) = exchange(&stream, &mut r, &forward_req(2), b"{}");
        assert_eq!((header["status"].as_u64(), body.as_slice()), (Some(200), &b"late"[..]));
        assert!(!handle.is_finished(), "keyd answered, then idles from the reply");
        handle.join().unwrap();
    }

    #[test]
    fn header_parsing_rejects_junk_without_quoting_it() {
        for line in [&b"not json"[..], b"[1]", b"{}", b"{\"op\":\"ping\",\"body_len\":-1}", b"{\"op\":\"ping\",\"body_len\":\"x\"}"] {
            let err = parse_header(line).unwrap_err();
            assert_eq!(err.kind, "request");
        }
        let err = parse_header(b"{\"op\":\"store\",\"value\":\"pa-SECRET\"").unwrap_err();
        assert!(!err.detail.contains("pa-SECRET"));
        let too_big = format!("{{\"op\":\"ping\",\"body_len\":{}}}", MAX_BODY + 1);
        assert!(parse_header(too_big.as_bytes()).is_err());
    }
}
