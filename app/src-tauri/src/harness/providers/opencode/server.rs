//! The `opencode serve` child: spawn, readiness, liveness and teardown.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::harness::child::ChildProc;
use crate::harness::event::HarnessEvent;
use crate::harness::{RawLog, Sink};

use super::http::urlencode;
use super::session::SessionRoute;
use super::types::OpencodeSpawn;
use super::{AGENT, CONFIG_NAME};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const READY_TIMEOUT: Duration = Duration::from_secs(20);
const BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(20);

/// The server's argv after the binary. [`super::strays::sweep`] recognises a stray by exactly
/// this list. `--port 0` means "prefer 4096", not "any free port".
pub(super) const SERVE_ARGS: [&str; 6] = [
    "serve",
    "--port",
    "0",
    "--hostname",
    "127.0.0.1",
    "--print-logs",
];

pub struct OpencodeServer {
    pub(super) proc: ChildProc,
    pub(super) base: String,
    pub(super) directory: PathBuf,
    /// With a timeout. `stream` has none: it would cut a healthy idle stream.
    pub(super) api: ureq::Agent,
    pub(super) stream: ureq::Agent,
    pub(super) routes: Mutex<HashMap<String, Arc<SessionRoute>>>,
    pub(super) default_sink: Option<Sink>,
    pub(super) raw_log: Option<RawLog>,
}

impl OpencodeServer {
    pub fn spawn(cfg: OpencodeSpawn) -> Result<Arc<Self>, String> {
        let mut cmd = Command::new(&cfg.bin);
        cmd.args(SERVE_ARGS)
            .env_clear()
            .envs(cfg.env.iter().map(|(k, v)| (k, v)))
            // No binary swapped under a running thread, no coursework on the web.
            .env("OPENCODE_DISABLE_AUTOUPDATE", "1")
            .env("OPENCODE_DISABLE_SHARE", "1");
        // stderr is the structured log, kept only as a tail.
        let (proc, stdout) = ChildProc::spawn("opencode", &mut cmd, false)?;

        // The real port is printed only on stdout. This thread then owns
        // stdout for the process's life: its EOF is how the bridge learns the
        // server is gone.
        let (port_tx, port_rx) = mpsc::channel::<Result<u16, String>>();
        let (srv_tx, srv_rx) = mpsc::channel::<Arc<OpencodeServer>>();
        std::thread::spawn(move || {
            let mut lines = BufReader::new(stdout).lines();
            let mut found = None;
            for line in lines.by_ref().map_while(Result::ok) {
                if let Some(p) = parse_port(&line) {
                    found = Some(p);
                    let _ = port_tx.send(Ok(p));
                    break;
                }
            }
            if found.is_none() {
                let _ = port_tx.send(Err("opencode serve exited before it said which port".into()));
            }
            drop(port_tx);
            let Ok(server) = srv_rx.recv() else { return };
            for _line in lines.map_while(Result::ok) {}
            server.on_exit();
        });

        let port = match port_rx.recv_timeout(READY_TIMEOUT) {
            Ok(r) => r?,
            Err(_) => {
                proc.kill();
                return Err(format!(
                    "opencode serve did not start in {}s",
                    READY_TIMEOUT.as_secs()
                ));
            }
        };

        let server = Arc::new(OpencodeServer {
            proc,
            base: format!("http://127.0.0.1:{port}"),
            directory: cfg.directory.clone(),
            api: ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(5))
                .timeout(REQUEST_TIMEOUT)
                .build(),
            stream: ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(5))
                .build(),
            routes: Mutex::new(HashMap::new()),
            default_sink: cfg.default_sink,
            raw_log: cfg.raw_log,
        });
        let _ = srv_tx.send(server.clone());

        server.get("/global/health")?;
        server.await_bootstrap()?;
        {
            let s = server.clone();
            std::thread::spawn(move || s.read_events());
        }
        {
            let s = server.clone();
            std::thread::spawn(move || s.watch_drains());
        }
        Ok(server)
    }

    /// No session is created until `oculus` is in the directory's agent list:
    /// without it a session runs as a built-in agent with none of the
    /// containment. Normally passes on the first request.
    fn await_bootstrap(&self) -> Result<(), String> {
        let deadline = Instant::now() + BOOTSTRAP_TIMEOUT;
        let mut last;
        loop {
            match self.get(&format!("/agent?{}", self.directory_query())) {
                Ok(v) => {
                    let found = v
                        .as_array()
                        .is_some_and(|a| a.iter().any(|x| x["name"].as_str() == Some(AGENT)));
                    if found {
                        return Ok(());
                    }
                    last = format!("`{AGENT}` is not in the agent list yet");
                }
                Err(e) => last = e,
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "opencode did not load {}: {last}",
                    self.directory.join(CONFIG_NAME).display()
                ));
            }
            std::thread::sleep(Duration::from_millis(400));
        }
    }

    /// On every instance-scoped call: an unscoped one binds to the server's
    /// own cwd, a different instance from ours.
    pub(super) fn directory_query(&self) -> String {
        format!(
            "directory={}",
            urlencode(&self.directory.display().to_string())
        )
    }

    pub fn is_alive(&self) -> bool {
        self.proc.is_alive()
    }

    /// True while any session has a turn open.
    pub fn busy(&self) -> bool {
        self.routes
            .lock()
            .unwrap()
            .values()
            .any(|r| r.state.lock().unwrap().turn_open)
    }

    /// `POST /instance/dispose`, which re-reads `auth.json`. Sessions and the
    /// stream survive it, but it releases the instance's resources, so it is
    /// skipped (and `false` returned) while a turn runs.
    pub fn refresh(&self) -> bool {
        if self.busy() {
            return false;
        }
        self.post(
            &format!("/instance/dispose?{}", self.directory_query()),
            json!({}),
        )
        .is_ok()
    }

    pub fn kill(&self) {
        self.proc.kill();
    }

    fn on_exit(&self) {
        let code = self.proc.reap();
        self.fail_open_turns(
            &self
                .proc
                .with_tail(format!("opencode serve exited (code {code:?})")),
        );
        let routes: Vec<Arc<SessionRoute>> = self
            .routes
            .lock()
            .unwrap()
            .drain()
            .map(|(_, r)| r)
            .collect();
        for r in routes {
            (r.sink)(HarnessEvent::Exited { code });
        }
    }
}

/// `opencode server listening on http://127.0.0.1:4096` (stdout only).
fn parse_port(line: &str) -> Option<u16> {
    let rest = line.split("listening on").nth(1)?;
    let host = rest.trim().trim_end_matches('/');
    host.rsplit(':').next()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_port_is_read_off_the_line_that_says_it() {
        assert_eq!(
            parse_port("opencode server listening on http://127.0.0.1:4096"),
            Some(4096)
        );
        assert_eq!(
            parse_port("opencode server listening on http://127.0.0.1:54888/"),
            Some(54888)
        );
        assert_eq!(
            parse_port("Warning: OPENCODE_SERVER_PASSWORD is not set"),
            None
        );
    }
}
