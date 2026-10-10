//! The inbound side: the one `GET /event` reader, the drain watchdog, and the
//! failing of open turns when the stream or process dies.

use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::harness::event::{HarnessEvent, Provider};

use super::events::{close_turn, session_of, translate, translate_server};
use super::server::OpencodeServer;
use super::session::SessionRoute;

/// User row to first assistant row, which opencode creates at once: not a
/// slow-model budget ([`OpencodeServer::watch_drains`]).
const DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

impl OpencodeServer {
    /// One `GET /event` for the whole app. A dropped stream fails the open
    /// turns before reconnecting: the gap may have eaten their close, and the
    /// queue releases a thread only on `TurnFinished`.
    pub(super) fn read_events(self: Arc<Self>) {
        let mut backoff = Duration::from_millis(200);
        let url = format!("{}/event?{}", self.base, self.directory_query());
        while self.is_alive() {
            match self.stream.get(&url).call() {
                Ok(resp) => {
                    backoff = Duration::from_millis(200);
                    for line in BufReader::new(resp.into_reader())
                        .lines()
                        .map_while(Result::ok)
                    {
                        let Some(payload) = line.strip_prefix("data: ") else {
                            continue;
                        };
                        if let Some(log) = &self.raw_log {
                            log.write(payload);
                        }
                        let Ok(v) = serde_json::from_str::<Value>(payload) else {
                            continue;
                        };
                        self.dispatch(&v);
                    }
                }
                Err(_) => {
                    backoff = (backoff * 2).min(Duration::from_secs(5));
                }
            }
            if !self.is_alive() {
                break;
            }
            self.fail_open_turns("the opencode event stream dropped mid-turn");
            std::thread::sleep(backoff);
        }
    }

    /// Backstop: fail a turn whose user row arrived with no assistant row in
    /// [`DRAIN_TIMEOUT`]. The known failures close themselves; a server that
    /// goes silent instead would otherwise strand the thread for good.
    pub(super) fn watch_drains(self: Arc<Self>) {
        while self.is_alive() {
            std::thread::sleep(Duration::from_secs(2));
            let routes: Vec<Arc<SessionRoute>> =
                self.routes.lock().unwrap().values().cloned().collect();
            for r in routes {
                let events = {
                    let mut st = r.state.lock().unwrap();
                    let stalled = st
                        .awaiting_step
                        .is_some_and(|at| at.elapsed() > DRAIN_TIMEOUT);
                    if !stalled {
                        continue;
                    }
                    st.awaiting_step = None;
                    close_turn(&mut st, "failed")
                };
                if events.is_empty() {
                    continue;
                }
                (r.sink)(HarnessEvent::error_for(
                    Provider::Opencode,
                    "opencode took the message and then never started answering it — \
                     usually a model it cannot resolve, or a provider it is not signed \
                     in to. Check the model in the picker and its provider in Settings.",
                ));
                for ev in events {
                    (r.sink)(ev);
                }
            }
        }
    }

    pub(super) fn dispatch(&self, v: &Value) {
        let Some(ty) = v["type"].as_str() else { return };
        // Session-less events go to the harness sink rather than being lost
        // to the route lookup.
        let Some(session) = session_of(v) else {
            if let Some(sink) = &self.default_sink {
                for ev in translate_server(ty, &v["properties"]) {
                    sink(ev);
                }
            }
            return;
        };
        let route = self.routes.lock().unwrap().get(session).cloned();
        // Not ours: the student's own TUI, or a detached naming session.
        let Some(route) = route else { return };
        let events = {
            let mut st = route.state.lock().unwrap();
            translate(ty, &v["properties"], &mut st)
        };
        for ev in events {
            (route.sink)(ev);
        }
    }

    /// Close every open turn as failed, when the stream drops or the process
    /// dies.
    pub(super) fn fail_open_turns(&self, why: &str) {
        let routes: Vec<Arc<SessionRoute>> =
            self.routes.lock().unwrap().values().cloned().collect();
        for r in routes {
            let events = {
                let mut st = r.state.lock().unwrap();
                close_turn(&mut st, "failed")
            };
            if events.is_empty() {
                continue;
            }
            (r.sink)(HarnessEvent::error_for(Provider::Opencode, why));
            for ev in events {
                (r.sink)(ev);
            }
        }
    }
}
