//! A temp directory per test, removed on drop, a fixed `ping` identity, a
//! caller check that answers as told, and a fake HTTP origin for `forward`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::ops::Build;
use crate::platform::{Caller, Conn, PeerCheck, Role};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// What a test keyd reports from `ping`.
pub const BUILD: Build = Build { version: "0.0.0-test", source_hash: "5555555555555555555555555555555555555555555555555555555555555555" };

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
pub struct Peers {
    verdict: Result<(), String>,
    pub seen: Arc<AtomicUsize>,
}

impl Peers {
    pub fn admit() -> Peers {
        Peers { verdict: Ok(()), seen: Arc::new(AtomicUsize::new(0)) }
    }

    pub fn refuse(why: &str) -> Peers {
        Peers { verdict: Err(why.to_string()), seen: Arc::new(AtomicUsize::new(0)) }
    }
}

impl PeerCheck for Peers {
    fn inspect(&self, _conn: &Conn) -> Caller {
        self.seen.fetch_add(1, Ordering::SeqCst);
        Caller { pid: Some(std::process::id()), role: Role::Cli, ..Caller::default() }
    }

    fn admit(&self, _caller: &Caller) -> Result<(), String> {
        self.verdict.clone()
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
        self.headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
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

impl FakeOrigin {
    pub fn start(handler: impl Fn(&Hit) -> Answer + Send + 'static) -> FakeOrigin {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let hits = Arc::new(Mutex::new(Vec::new()));
        let log = hits.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let mut reader = BufReader::new(&stream);
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    continue;
                }
                let mut parts = line.split_whitespace();
                let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
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
                let len: usize = headers.iter().find(|(k, _)| k == "content-length").map_or(0, |(_, v)| v.parse().unwrap());
                let mut body = vec![0; len];
                reader.read_exact(&mut body).unwrap();
                let hit = Hit { method, path, headers, body };
                log.lock().unwrap().push(hit.clone());

                let answer = handler(&hit);
                let mut out = format!("HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n", answer.status, answer.body.len());
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

    pub fn hits(&self) -> Vec<Hit> {
        self.hits.lock().unwrap().clone()
    }
}
