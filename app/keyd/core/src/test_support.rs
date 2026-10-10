//! A temp directory per test, removed on drop, a fixed `ping` identity, a
//! caller check that answers as told, and a fake HTTP origin for `forward`
//! and the sign-in. Compiled for tests, and for keyd's own integration test
//! under the `test-support` feature.

pub mod okta_fake;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(feature = "server")]
use crate::ops::Build;
#[cfg(feature = "server")]
use crate::platform::{Caller, Conn, PeerCheck, Role};
#[cfg(feature = "server")]
use crate::vault::{KeyError, LegacySource};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A clock a test sets and moves; clones share the time.
#[derive(Clone)]
pub struct TestClock(Arc<AtomicU64>);

impl TestClock {
    pub fn at(secs: u64) -> TestClock {
        TestClock(Arc::new(AtomicU64::new(secs)))
    }

    pub fn advance(&self, secs: u64) {
        self.0.fetch_add(secs, Ordering::SeqCst);
    }

    pub fn clock(&self) -> crate::clock::Clock {
        let secs = self.0.clone();
        Arc::new(move || secs.load(Ordering::SeqCst))
    }
}

/// What a test keyd reports from `ping`.
#[cfg(feature = "server")]
pub const BUILD: Build = Build {
    version: "0.0.0-test",
    source_hash: "5555555555555555555555555555555555555555555555555555555555555555",
};

pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(name: &str) -> Self {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("keyd-{name}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// Admits or refuses every caller, counting the connections it inspected.
#[cfg(feature = "server")]
pub struct Peers {
    verdict: Result<(), String>,
    role: Role,
    pub seen: Arc<AtomicUsize>,
}

#[cfg(feature = "server")]
impl Peers {
    pub fn admit() -> Peers {
        Peers {
            verdict: Ok(()),
            role: Role::Cli,
            seen: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Admits every caller, reporting `role` for each.
    pub fn admit_as(role: Role) -> Peers {
        Peers {
            role,
            ..Peers::admit()
        }
    }

    pub fn refuse(why: &str) -> Peers {
        Peers {
            verdict: Err(why.to_string()),
            role: Role::Cli,
            seen: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[cfg(feature = "server")]
impl PeerCheck for Peers {
    fn inspect(&self, _conn: &Conn) -> Caller {
        self.seen.fetch_add(1, Ordering::SeqCst);
        Caller {
            pid: Some(std::process::id()),
            role: self.role,
            ..Caller::default()
        }
    }

    fn admit(&self, _caller: &Caller) -> Result<(), String> {
        self.verdict.clone()
    }
}

/// How many old keychain items a test keyd has read.
#[cfg(feature = "server")]
pub type Reads = Arc<AtomicUsize>;

/// Old keychain items by (service, account), counting every read. It can only
/// read: keyd never writes or deletes an old item.
#[cfg(feature = "server")]
pub struct OldItems(
    pub  Vec<(
        (&'static str, &'static str),
        Result<Option<String>, KeyError>,
    )>,
    pub Reads,
);

#[cfg(feature = "server")]
impl LegacySource for OldItems {
    fn read(&self, service: &str, account: &str) -> Result<Option<String>, KeyError> {
        self.1.fetch_add(1, Ordering::SeqCst);
        self.0
            .iter()
            .find(|((s, a), _)| *s == service && *a == account)
            .map(|(_, r)| r.clone())
            .unwrap_or(Ok(None))
    }
}

/// One request as the fake origin saw it. Header names are lowercased.
#[derive(Debug, Clone)]
pub struct Hit {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Hit {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

pub struct Answer {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

/// An HTTP/1.1 origin on a free loopback port, one request per connection,
/// answering each with `handler`. Its thread ends with the test process.
pub struct FakeOrigin {
    pub origin: String,
    hits: Arc<Mutex<Vec<Hit>>>,
}

/// Reads one request off `stream`; `None` when the peer sent nothing.
fn read_hit(stream: &TcpStream) -> Option<Hit> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let (method, path) = (
        parts.next().unwrap_or("").to_string(),
        parts.next().unwrap_or("").to_string(),
    );
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        reader.read_line(&mut h).unwrap();
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        let (k, v) = h.split_once(':').unwrap();
        headers.push((k.trim().to_lowercase(), v.trim().to_string()));
    }
    let len: usize = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .map_or(0, |(_, v)| v.parse().unwrap());
    let mut body = vec![0; len];
    reader.read_exact(&mut body).unwrap();
    Some(Hit {
        method,
        path,
        headers,
        body,
    })
}

impl FakeOrigin {
    pub fn start(handler: impl Fn(&Hit) -> Answer + Send + 'static) -> FakeOrigin {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let hits = Arc::new(Mutex::new(Vec::new()));
        let log = hits.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let Some(hit) = read_hit(&stream) else {
                    continue;
                };
                log.lock().unwrap().push(hit.clone());

                let answer = handler(&hit);
                let mut out = format!(
                    "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n",
                    answer.status,
                    answer.body.len()
                );
                for (k, v) in &answer.headers {
                    out.push_str(&format!("{k}: {v}\r\n"));
                }
                out.push_str("\r\n");
                let mut w = &stream;
                w.write_all(out.as_bytes()).ok();
                w.write_all(&answer.body).ok();
            }
        });
        FakeOrigin { origin, hits }
    }

    /// An origin whose `handler` writes the whole response itself (the status
    /// line, headers and body, at whatever pace and for however long it
    /// likes), and the connection closes when it returns. Each connection
    /// gets a thread, so one slow answer holds up no other.
    pub fn start_raw(handler: impl Fn(&Hit, &mut TcpStream) + Send + Sync + 'static) -> FakeOrigin {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let hits = Arc::new(Mutex::new(Vec::new()));
        let log = hits.clone();
        let handler = Arc::new(handler);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let (log, handler) = (log.clone(), handler.clone());
                std::thread::spawn(move || {
                    let Some(hit) = read_hit(&stream) else { return };
                    log.lock().unwrap().push(hit.clone());
                    handler(&hit, &mut stream);
                });
            }
        });
        FakeOrigin { origin, hits }
    }

    pub fn hits(&self) -> Vec<Hit> {
        self.hits.lock().unwrap().clone()
    }
}
