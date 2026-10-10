//! The accept loop, and each connection's requests in turn.
//!
//! One thread both accepts and decides to exit: `run` waits up to a tick for
//! a client on any listener, and only when none came does it check whether
//! keyd has been idle for `idle` with no request in flight, and return
//! (`main` then exits). So a client is never accepted and then dropped by
//! the exit; one that arrives after the decision stays queued at the
//! endpoint for the next keyd. A connection counts as busy from a complete
//! header line until its reply (a streamed body too) is written, so a
//! `forward` waiting on its origin holds keyd open, but a client that
//! connects and stalls cannot. No read timeouts: a forwarded parse or embed
//! takes minutes. The wire format is `framing`'s.

use std::io::BufReader;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::framing::{self, MAX_LINE};
use crate::log;
use crate::ops::{OpError, Reply, State};
use crate::platform::{self, Conn, Listener, PeerCheck};

/// How long one wait for a client lasts before the idle check.
const TICK: Duration = Duration::from_millis(250);

pub struct Server {
    pub state: State,
    peers: Box<dyn PeerCheck>,
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
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl Server {
    pub fn new(state: State, peers: Box<dyn PeerCheck>, idle: Duration) -> Self {
        Server {
            state,
            peers,
            idle,
            busy: AtomicUsize::new(0),
            last: AtomicU64::new(now_ms()),
        }
    }

    fn touch(&self) {
        self.last.store(now_ms(), Ordering::SeqCst);
    }

    fn idle_for(&self) -> Duration {
        Duration::from_millis(now_ms().saturating_sub(self.last.load(Ordering::SeqCst)))
    }

    /// Serves `listeners` until keyd has been idle for `self.idle`, then
    /// returns, leaving any client it has not accepted queued.
    pub fn run(self: Arc<Self>, listeners: &[Listener]) {
        loop {
            match platform::accept_any(listeners, TICK) {
                Ok(Some(conn)) => {
                    self.touch();
                    let server = self.clone();
                    std::thread::spawn(move || server.connection(conn));
                }
                Ok(None) => {
                    if self.busy.load(Ordering::SeqCst) == 0 && self.idle_for() >= self.idle {
                        log(&format!("idle for {}s, exiting", self.idle.as_secs()));
                        return;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => {
                    log(&format!("accept failed: {e}"));
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }

    fn connection(&self, conn: Conn) {
        // The caller check runs before a byte of the request is read.
        let caller = self.peers.inspect(&conn);
        let verdict = self.peers.admit(&caller);
        let mut stream = BufReader::new(conn);
        let who = caller.label();

        loop {
            let line = match framing::read_line(&mut stream, MAX_LINE) {
                Ok(Some(line)) => line,
                Ok(None) => return,
                Err(e) => {
                    log(&format!("caller={who} closed: {e}"));
                    framing::write_frame(
                        stream.get_mut(),
                        &OpError::new("request", e.0).to_json(),
                        &[],
                    )
                    .ok();
                    return;
                }
            };
            let _busy = {
                self.busy.fetch_add(1, Ordering::SeqCst);
                Busy(self)
            };

            // A refused caller gets its reply without keyd reading the body.
            let parsed = parse_request(&line);
            // Client text, so only a plain word reaches the log.
            let op = match &parsed {
                Ok((op, _, _))
                    if !op.is_empty()
                        && op.len() <= 32
                        && op.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') =>
                {
                    op.clone()
                }
                _ => "?".to_string(),
            };
            let mut in_step = false;
            let reply = match (&verdict, parsed) {
                (Err(why), _) => Err(OpError::new("caller", why.clone())),
                (Ok(()), Err(e)) => Err(e),
                (Ok(()), Ok((op, req, body_len))) => {
                    match framing::read_body(&mut stream, body_len) {
                        Err(e) => Err(OpError::new("request", e.0)),
                        Ok(body) => {
                            in_step = true;
                            self.state.dispatch(&caller, &op, &req, &body)
                        }
                    }
                }
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
            let admitted = if verdict.is_ok() {
                "admitted"
            } else {
                "refused"
            };
            match &verdict {
                Err(why) => log(&format!(
                    "op={op} caller={who} {admitted} ({why}) {outcome}"
                )),
                Ok(()) => log(&format!("op={op} caller={who} {admitted} {outcome}")),
            }
            if let Err(e) = framing::write_frame(stream.get_mut(), &reply.header, &reply.body) {
                log(&format!("op={op} caller={who} reply failed: {e}"));
                return;
            }
            // A streamed body is the rest of the connection: it ends the
            // connection, and the client learns its length from the origin's
            // headers.
            if let Some(body) = reply.stream {
                let (sent, why) = stream_body(body.0, stream.get_mut());
                log(&format!("op={op} caller={who} streamed {sent} bytes{why}"));
                return;
            }
            // A refused caller, or a stream whose framing is lost, gets one reply.
            if !in_step {
                return;
            }
        }
    }
}

/// Copies `body` to `out` until either ends. A stream that fails partway
/// just stops, so the client finds the body short; the log line says which
/// end failed.
fn stream_body(
    mut body: Box<dyn std::io::Read + Send>,
    out: &mut impl std::io::Write,
) -> (u64, &'static str) {
    let mut chunk = vec![0u8; 64 * 1024];
    let mut sent = 0u64;
    loop {
        let n = match body.read(&mut chunk) {
            Ok(0) => return (sent, ""),
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return (sent, ", the origin's body ended early"),
        };
        if out.write_all(&chunk[..n]).is_err() {
            return (sent, ", the client went away");
        }
        sent += n as u64;
    }
}

/// The op name, the whole header, and the body length it announces.
fn parse_request(line: &[u8]) -> Result<(String, Value, u64), OpError> {
    let (req, body_len) = framing::parse_header(line).map_err(|e| OpError::new("request", e.0))?;
    let op = req
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| OpError::new("request", "missing \"op\""))?
        .to_string();
    Ok((op, req, body_len))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forward::Routes;
    use crate::framing::{read_body, read_line};
    use crate::platform::memory::{self, Connector};
    use crate::test_support::{Answer, FakeOrigin, Peers, Scratch, BUILD};
    use crate::vault::{KeyError, KeySource, MasterKey, NoLegacy, StaticKey, Vault};
    use serde_json::json;
    use std::io::Write;

    type Served = (Scratch, Connector, std::thread::JoinHandle<()>);

    fn serve(peers: Peers, idle: Duration) -> Served {
        serve_with(
            peers,
            idle,
            Box::new(StaticKey(MasterKey::from_bytes([3; 32]))),
        )
    }

    fn serve_with(peers: Peers, idle: Duration, keys: Box<dyn KeySource>) -> Served {
        serve_state(peers, idle, |dir| {
            State::new(BUILD, dir.to_path_buf(), keys, Box::new(NoLegacy))
        })
    }

    fn serve_state(
        peers: Peers,
        idle: Duration,
        state: impl FnOnce(&std::path::Path) -> State,
    ) -> Served {
        let dir = Scratch::new("srv");
        let (listener, connector) = memory::listener();
        let state = state(&dir.0);
        let server = Arc::new(Server::new(state, Box::new(peers), idle));
        let handle = std::thread::spawn(move || server.run(&[listener]));
        (dir, connector, handle)
    }

    fn call(stream: &mut BufReader<Conn>, req: &Value) -> Value {
        stream
            .get_mut()
            .write_all(format!("{req}\n").as_bytes())
            .unwrap();
        let line = read_line(stream, MAX_LINE).unwrap().unwrap();
        serde_json::from_slice(&line).unwrap()
    }

    #[test]
    fn ops_run_in_turn_over_one_connection() {
        let (_dir, keyd, _h) = serve(Peers::admit(), Duration::from_secs(60));
        let mut s = BufReader::new(keyd.connect());
        assert_eq!(
            call(&mut s, &json!({"op": "ping"}))["pid"],
            std::process::id()
        );
        assert_eq!(
            call(
                &mut s,
                &json!({"op": "store", "secret": "mineru", "value": "tok"})
            )["stored"],
            true
        );
        assert_eq!(
            call(&mut s, &json!({"op": "has", "secret": "mineru"}))["has"],
            true
        );
        assert_eq!(call(&mut s, &json!({"op": "nope"}))["error"], "request");
        assert_eq!(
            call(&mut s, &json!({"op": "delete", "secret": "mineru"}))["existed"],
            true
        );
    }

    #[test]
    fn a_body_after_the_header_is_read_with_the_same_reader() {
        let (_dir, keyd, _h) = serve(Peers::admit(), Duration::from_secs(60));
        let mut s = BufReader::new(keyd.connect());
        // Header, body and the next request in one write: none of it may be lost.
        s.get_mut()
            .write_all(b"{\"op\":\"ping\",\"body_len\":5}\nhello{\"op\":\"ping\"}\n")
            .unwrap();
        let first: Value =
            serde_json::from_slice(&read_line(&mut s, MAX_LINE).unwrap().unwrap()).unwrap();
        assert_eq!(first["error"], "request", "ping takes no body");
        let second: Value =
            serde_json::from_slice(&read_line(&mut s, MAX_LINE).unwrap().unwrap()).unwrap();
        assert_eq!(second["pid"], std::process::id());
    }

    #[test]
    fn an_overlong_header_line_is_refused_and_closed() {
        let (_dir, keyd, _h) = serve(Peers::admit(), Duration::from_secs(60));
        let mut s = BufReader::new(keyd.connect());
        let long = vec![b'a'; MAX_LINE + 10];
        // The server stops reading partway, so this write may fail; the reply still comes.
        s.get_mut().write_all(&long).ok();
        let reply: Value =
            serde_json::from_slice(&read_line(&mut s, MAX_LINE).unwrap().unwrap()).unwrap();
        assert_eq!(reply["error"], "request");
        assert!(
            read_line(&mut s, MAX_LINE).unwrap().is_none(),
            "the connection is closed"
        );
    }

    #[test]
    fn a_refused_caller_gets_one_error_and_no_op_runs() {
        let (dir, keyd, _h) = serve(
            Peers::refuse("outside the install"),
            Duration::from_secs(60),
        );
        let mut s = BufReader::new(keyd.connect());
        let reply = call(
            &mut s,
            &json!({"op": "store", "secret": "groq", "value": "gsk"}),
        );
        assert_eq!(reply["error"], "caller");
        assert_eq!(reply["detail"], "outside the install");
        assert!(!crate::paths::vault(&dir.0).exists(), "the op never ran");
        assert!(read_line(&mut s, MAX_LINE).unwrap().is_none());
    }

    #[test]
    fn header_parsing_names_a_missing_op() {
        assert_eq!(parse_request(b"{}").unwrap_err().kind, "request");
        assert_eq!(
            parse_request(b"{\"op\":\"ping\",\"body_len\":-1}")
                .unwrap_err()
                .kind,
            "request"
        );
        let (op, _, len) = parse_request(b"{\"op\":\"forward\",\"body_len\":3}").unwrap();
        assert_eq!((op.as_str(), len), ("forward", 3));
    }

    #[test]
    fn a_stalled_connection_does_not_hold_keyd_open() {
        let (_dir, keyd, handle) = serve(Peers::admit(), Duration::from_secs(1));
        let _stalled = keyd.connect();
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
        let (_dir, keyd, handle) =
            serve_with(Peers::admit(), Duration::from_secs(1), Box::new(Slow));
        let mut s = BufReader::new(keyd.connect());
        assert_eq!(
            call(&mut s, &json!({"op": "has", "secret": "voyage"}))["has"],
            false
        );
        assert!(
            !handle.is_finished(),
            "keyd answered, then idles from the reply"
        );
        handle.join().unwrap();
    }

    /// Serves `origin` as Voyage's, with a key already stored.
    fn serve_forwarding(idle: Duration, origin: &FakeOrigin) -> Served {
        let origin = origin.origin.clone();
        serve_state(Peers::admit(), idle, move |dir| {
            let state = State::new(
                BUILD,
                dir.to_path_buf(),
                Box::new(StaticKey(MasterKey::from_bytes([3; 32]))),
                Box::new(NoLegacy),
            )
            .with_routes(Routes::compiled().with_origin("voyage", &origin).unwrap());
            Vault::new(crate::paths::vault(dir), MasterKey::from_bytes([3; 32]))
                .store("voyage", "pa-KEY")
                .unwrap();
            state
        })
    }

    /// One request with a body; the reply's header and body.
    fn exchange(stream: &mut BufReader<Conn>, req: &Value, body: &[u8]) -> (Value, Vec<u8>) {
        stream
            .get_mut()
            .write_all(format!("{req}\n").as_bytes())
            .unwrap();
        stream.get_mut().write_all(body).unwrap();
        let header: Value =
            serde_json::from_slice(&read_line(stream, MAX_LINE).unwrap().unwrap()).unwrap();
        let len = header.get("body_len").and_then(Value::as_u64).unwrap_or(0);
        (header, read_body(stream, len).unwrap())
    }

    fn forward_req(body_len: usize) -> Value {
        json!({"op": "forward", "secret": "voyage", "method": "POST", "path": "/v1/multimodalembeddings",
               "headers": [["Content-Type", "application/json"]], "body_len": body_len})
    }

    #[test]
    fn a_forward_carries_bodies_both_ways_and_the_connection_goes_on() {
        let origin = FakeOrigin::start(|hit| Answer {
            status: 200,
            headers: vec![],
            body: hit.body.iter().rev().copied().collect(),
        });
        let (_dir, keyd, _h) = serve_forwarding(Duration::from_secs(60), &origin);
        let mut s = BufReader::new(keyd.connect());
        let big: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        let (header, body) = exchange(&mut s, &forward_req(big.len()), &big);
        assert_eq!(header["status"], 200);
        assert_eq!(body, big.iter().rev().copied().collect::<Vec<_>>());
        assert_eq!(origin.hits()[0].body, big);
        // Framing held: the next request on the same connection is answered.
        assert_eq!(
            call(&mut s, &json!({"op": "has", "secret": "voyage"}))["has"],
            true
        );
    }

    #[test]
    fn a_forward_waiting_on_its_origin_holds_keyd_open_past_the_idle_window() {
        let origin = FakeOrigin::start(|_| {
            std::thread::sleep(Duration::from_millis(2500));
            Answer {
                status: 200,
                headers: vec![],
                body: b"late".to_vec(),
            }
        });
        let (_dir, keyd, handle) = serve_forwarding(Duration::from_secs(1), &origin);
        let mut s = BufReader::new(keyd.connect());
        let (header, body) = exchange(&mut s, &forward_req(2), b"{}");
        assert_eq!(
            (header["status"].as_u64(), body.as_slice()),
            (Some(200), &b"late"[..])
        );
        assert!(
            !handle.is_finished(),
            "keyd answered, then idles from the reply"
        );
        handle.join().unwrap();
    }

    /// The exit decision and the accept are one thread's: a client that comes
    /// after keyd decided to exit is not taken and dropped, but waits for the
    /// next keyd, as launchd's socket keeps it.
    #[test]
    fn a_client_after_the_exit_decision_waits_for_the_next_keyd() {
        let dir = Scratch::new("srv-requeue");
        let (listener, keyd) = memory::listener();
        let listeners = vec![listener];
        let state = || {
            State::new(
                BUILD,
                dir.0.clone(),
                Box::new(StaticKey(MasterKey::from_bytes([3; 32]))),
                Box::new(NoLegacy),
            )
        };
        let first = Arc::new(Server::new(
            state(),
            Box::new(Peers::admit()),
            Duration::ZERO,
        ));
        first.run(&listeners);

        let mut s = BufReader::new(keyd.connect());
        s.get_mut().write_all(b"{\"op\":\"ping\"}\n").unwrap();
        // The keyd that exited never read it: no reply is waiting.
        s.get_mut()
            .set_timeout(Some(Duration::from_millis(300)))
            .unwrap();
        assert!(
            read_line(&mut s, MAX_LINE).is_err(),
            "nothing answered after the exit"
        );

        let second = Arc::new(Server::new(
            state(),
            Box::new(Peers::admit()),
            Duration::from_secs(60),
        ));
        std::thread::spawn(move || second.run(&listeners));
        s.get_mut()
            .set_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let reply: Value =
            serde_json::from_slice(&read_line(&mut s, MAX_LINE).unwrap().unwrap()).unwrap();
        assert_eq!(reply["pid"], std::process::id());
    }

    #[test]
    fn the_caller_check_sees_the_connection_before_any_request() {
        let peers = Peers::admit();
        let seen = peers.seen.clone();
        let (_dir, keyd, _h) = serve(peers, Duration::from_secs(60));
        let _quiet = keyd.connect();
        let started = std::time::Instant::now();
        while seen.load(Ordering::SeqCst) == 0 {
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "inspect never ran"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
