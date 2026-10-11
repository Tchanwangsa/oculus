//! The daemon under test and the way to talk to it.

use keyd_core::platform::{self, Conn};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

/// Long enough for a loaded machine building in parallel, short enough that a
/// stuck keyd, origin or child fails the test instead of hanging the suite.
pub(crate) const DEADLINE: Duration = Duration::from_secs(60);

/// Transferred cross-compiled tests use the daemon's path on the guest.
pub(crate) fn keyd_binary() -> PathBuf {
    std::env::var_os("OCULUS_TEST_KEYD")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_oculus-keyd")))
}

/// A connection whose reads and writes give up after `DEADLINE`.
pub(crate) fn connect(sock: &std::path::Path) -> Conn {
    let stream = platform::connect(sock).unwrap();
    stream.set_timeout(Some(DEADLINE)).unwrap();
    stream
}

pub(crate) struct Keyd {
    pub(crate) child: Child,
    pub(crate) dir: PathBuf,
    pub(crate) sock: PathBuf,
}

impl Keyd {
    pub(crate) fn start(idle_secs: u64) -> Keyd {
        Keyd::start_with(idle_secs, &[])
    }

    /// `origins` sets each `(variable, origin)`, e.g. a fake Voyage under
    /// `OCULUS_KEYD_VOYAGE_ORIGIN`. This process is the CLI to the daemon, as
    /// every op but `ping` needs a role.
    pub(crate) fn start_with(idle_secs: u64, origins: &[(&str, &str)]) -> Keyd {
        Keyd::spawn(idle_secs, origins, Some("cli"))
    }

    /// Like `start_with`, but no role is forced: a caller has the role its
    /// executable's name gives it, and this process has none.
    pub(crate) fn start_by_name(idle_secs: u64, origins: &[(&str, &str)]) -> Keyd {
        Keyd::spawn(idle_secs, origins, None)
    }

    fn spawn(idle_secs: u64, origins: &[(&str, &str)], role: Option<&str>) -> Keyd {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("keyd-bin-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("k.sock");
        let child = Keyd::run(&dir, &sock, idle_secs, origins, role);
        Keyd { child, dir, sock }
    }

    /// A keyd over `dir`, started once its socket is bound.
    fn run(
        dir: &std::path::Path,
        sock: &std::path::Path,
        idle_secs: u64,
        origins: &[(&str, &str)],
        role: Option<&str>,
    ) -> Child {
        let mut cmd = Command::new(keyd_binary());
        cmd.arg("serve-local")
            .arg(sock)
            .env("OCULUS_KEYD_DATA_DIR", dir)
            .env("OCULUS_KEYD_TEST_KEY", "11".repeat(32))
            .env("OCULUS_KEYD_IDLE_SECS", idle_secs.to_string())
            .stderr(Stdio::piped());
        if let Some(role) = role {
            cmd.env("OCULUS_KEYD_TEST_ROLE", role);
        }
        for (var, origin) in origins {
            cmd.env(var, origin);
        }
        let child = cmd.spawn().unwrap();
        let started = Instant::now();
        while platform::connect(sock).is_err() {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "keyd never bound its socket"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        child
    }

    /// Stops this keyd and starts another over the same data dir (and so the
    /// same vault) that sees every caller as `role`.
    pub(crate) fn restart_as(&mut self, role: &str) {
        self.child.kill().ok();
        self.child.wait().ok();
        std::fs::remove_file(&self.sock).ok();
        self.child = Keyd::run(&self.dir, &self.sock, 60, &[], Some(role));
    }

    pub(crate) fn call(&self, req: Value) -> Value {
        call_at(&self.sock, &req)
    }
}

impl Drop for Keyd {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

/// One request and its one-line reply, over the endpoint at `sock`.
pub(crate) fn call_at(sock: &std::path::Path, req: &Value) -> Value {
    let mut stream = connect(sock);
    stream.write_all(format!("{req}\n").as_bytes()).unwrap();
    let mut line = String::new();
    BufReader::new(&mut stream)
        .read_line(&mut line)
        .unwrap_or_else(|error| {
            let op = req
                .get("op")
                .and_then(Value::as_str)
                .filter(|op| {
                    !op.is_empty()
                        && op.len() <= 32
                        && op.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
                })
                .unwrap_or("?");
            panic!("keyd reply for op={op} failed: {error}");
        });
    serde_json::from_str(&line).unwrap()
}
