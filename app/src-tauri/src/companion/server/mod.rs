//! The transport: hyper's HTTP/1 server with upgrades on a private two-worker
//! tokio runtime, never Tauri's. Every task hangs off one cancellation token
//! and one task tracker, so `shutdown` cancels the tree, waits briefly for the
//! tasks to say goodbye, then drops the runtime: it returns in about a second
//! at most, whatever the clients do.

mod accept;
mod bind;
mod guard;
pub mod http;
mod router;
mod ws;

use super::core::{Core, Listening};
use super::limits::SHUTDOWN_GRACE;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

pub use bind::bind;

/// One accepted connection; `share` is the share its last authenticated
/// request named, so revoking that share closes the socket.
pub struct Connection {
    pub cancel: CancellationToken,
    pub share: Mutex<Option<String>>,
    pub upgraded: AtomicBool,
}

impl Connection {
    pub fn serve(&self, share: &str) {
        *self.share.lock().unwrap_or_else(|e| e.into_inner()) = Some(share.to_string());
    }

    fn serves(&self, share: &str) -> bool {
        self.share
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_deref()
            == Some(share)
    }
}

/// What the server's tasks share.
pub struct Net {
    pub root: CancellationToken,
    pub tracker: TaskTracker,
    pub connections: Mutex<HashMap<u64, Arc<Connection>>>,
    pub next_connection: AtomicU64,
    /// WebSockets upgraded but not yet past `hello`.
    pub unauthenticated: AtomicUsize,
    pub listening: Listening,
}

impl Net {
    fn connections(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Arc<Connection>>> {
        self.connections.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn forget(&self, id: u64) {
        self.connections().remove(&id);
    }
}

pub struct Server {
    runtime: Option<tokio::runtime::Runtime>,
    net: Arc<Net>,
}

impl Server {
    /// Bind and start accepting. Nothing is served until a share exists.
    pub fn start(core: Arc<Core>, preferred: u16, attempts: u16) -> std::io::Result<Server> {
        let bound = bind(preferred, attempts)?;
        let listening = Listening {
            port: bound.port,
            v6: bound.v6.is_some(),
        };
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("companion")
            .enable_all()
            .build()?;
        let net = Arc::new(Net {
            root: CancellationToken::new(),
            tracker: TaskTracker::new(),
            connections: Mutex::new(HashMap::new()),
            next_connection: AtomicU64::new(1),
            unauthenticated: AtomicUsize::new(0),
            listening,
        });
        for listener in std::iter::once(bound.v4).chain(bound.v6) {
            listener.set_nonblocking(true)?;
            let listener = {
                let _entered = runtime.enter();
                tokio::net::TcpListener::from_std(listener)?
            };
            let task = accept::accept_loop(core.clone(), net.clone(), listener);
            runtime.spawn(net.tracker.track_future(task));
        }
        elog!(
            "[companion] sharing server on port {} (IPv4{})",
            listening.port,
            if listening.v6 { " and IPv6" } else { "" }
        );
        Ok(Server {
            runtime: Some(runtime),
            net,
        })
    }

    pub fn listening(&self) -> Listening {
        self.net.listening
    }

    pub fn root_token(&self) -> CancellationToken {
        self.net.root.clone()
    }

    /// Close every connection whose last request was for `share`, including
    /// one mid-response. Requests and sockets also watch the share's own
    /// token; this catches idle keep-alive sockets too.
    pub fn close_share(&self, share: &str) {
        for connection in self.net.connections().values() {
            if connection.serves(share) {
                connection.cancel.cancel();
            }
        }
    }

    /// Cancel everything, give tasks `SHUTDOWN_GRACE` to send their Close
    /// frames, then drop the runtime. Runs the teardown on its own thread,
    /// so it is safe from any context, async or not.
    pub fn shutdown(mut self) -> Duration {
        let started = std::time::Instant::now();
        self.stop();
        started.elapsed()
    }

    fn stop(&mut self) {
        let Some(runtime) = self.runtime.take() else {
            return;
        };
        let net = self.net.clone();
        net.root.cancel();
        net.tracker.close();
        let teardown = std::thread::Builder::new()
            .name("companion-shutdown".into())
            .spawn(move || {
                runtime.block_on(async {
                    tokio::time::timeout(SHUTDOWN_GRACE, net.tracker.wait())
                        .await
                        .ok();
                });
                runtime.shutdown_timeout(Duration::from_millis(100));
            });
        match teardown {
            Ok(thread) => {
                thread.join().ok();
            }
            Err(error) => elog!("[companion] shutdown thread did not start: {error}"),
        }
        elog!("[companion] sharing server stopped");
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}
