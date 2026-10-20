//! The share registry. A share grants one canvas file to whoever holds its
//! secret, for one authority epoch; at most one share per file. Each keeps
//! its change log, its push channel to sockets, its socket count (and which
//! of this Mac's addresses each socket came in on) and a cancellation token
//! every request and socket serving it watches. Revoking
//! removes the grant, remembers its id briefly so clients learn the session
//! ended, and cancels the token.

pub mod log;
pub mod secret;

use crate::companion::limits::{PUSH_QUEUE, REVOKED_KEPT};
use crate::library::files::canvases::authority::events::{Change, Phase};
use log::{ChangeLog, Entry};
use secret::Secret;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// Why a share ended, as sent in `session-ended`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EndReason {
    /// The desktop stopped sharing.
    Stopped,
    /// The canvas's authority session ended: closed, deleted or gone.
    Ended,
    /// The app is quitting.
    Shutdown,
}

/// What a share tells its sockets.
#[derive(Clone)]
pub enum Push {
    Change(Arc<Entry>),
    State(Phase),
    Presence(usize),
}

pub struct Share {
    pub id: String,
    secret: Secret,
    pub file_id: i64,
    pub cancel: CancellationToken,
    log: Mutex<ChangeLog>,
    push: broadcast::Sender<Push>,
    sockets: AtomicUsize,
    /// Joined sockets per address their Host named.
    via: Mutex<HashMap<IpAddr, usize>>,
    phase: Mutex<Phase>,
    ended: Mutex<Option<EndReason>>,
}

impl std::fmt::Debug for Share {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Share")
            .field("id", &self.id)
            .field("secret", &self.secret)
            .field("file_id", &self.file_id)
            .finish_non_exhaustive()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl Share {
    /// A share whose base revision is not known yet (`establish`).
    pub fn new(file_id: i64, cancel: CancellationToken) -> Option<Self> {
        Some(Self {
            id: secret::share_id()?,
            secret: Secret::generate()?,
            file_id,
            cancel,
            log: Mutex::new(ChangeLog::default()),
            push: broadcast::channel(PUSH_QUEUE).0,
            sockets: AtomicUsize::new(0),
            via: Mutex::new(HashMap::new()),
            phase: Mutex::new(Phase::Live),
            ended: Mutex::new(None),
        })
    }

    /// `<shareId>.<secret>`, the link fragment; for the desktop only.
    pub fn token(&self) -> String {
        format!("{}.{}", self.id, self.secret.expose())
    }

    pub fn establish(&self, epoch: &str, revision: u64, phase: Phase) {
        lock(&self.log).establish(epoch, revision);
        *lock(&self.phase) = phase;
    }

    pub fn epoch(&self) -> Option<String> {
        lock(&self.log).epoch().map(str::to_string)
    }

    /// The epoch and latest revision; `None` before `establish`.
    pub fn head(&self) -> Option<(String, u64)> {
        let log = lock(&self.log);
        Some((log.epoch()?.to_string(), log.head()))
    }

    pub fn since(&self, from: u64, epoch: Option<&str>) -> Result<Vec<Arc<Entry>>, log::Gone> {
        lock(&self.log).since(from, epoch)
    }

    /// Log an accepted change and push it to the sockets.
    pub fn record(&self, change: &Change) {
        let entry = match Entry::from_change(change) {
            Ok(entry) => Arc::new(entry),
            Err(error) => {
                elog!("[companion] change {} not logged: {error}", change.revision);
                return;
            }
        };
        if lock(&self.log).push(entry.clone()) {
            self.push.send(Push::Change(entry)).ok();
        }
    }

    pub fn phase(&self) -> Phase {
        *lock(&self.phase)
    }

    pub fn set_phase(&self, phase: Phase) {
        let mut current = lock(&self.phase);
        if *current != phase {
            *current = phase;
            self.push.send(Push::State(phase)).ok();
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Push> {
        self.push.subscribe()
    }

    pub fn clients(&self) -> usize {
        self.sockets.load(Ordering::SeqCst)
    }

    /// Count a socket in, unless the share is at `cap`. `via` is the
    /// address its Host named, when that was an IP.
    pub fn join(&self, cap: usize, via: Option<IpAddr>) -> bool {
        if self.sockets.fetch_add(1, Ordering::SeqCst) >= cap {
            self.sockets.fetch_sub(1, Ordering::SeqCst);
            return false;
        }
        if let Some(ip) = via {
            *lock(&self.via).entry(ip).or_default() += 1;
        }
        self.push.send(Push::Presence(self.clients())).ok();
        true
    }

    pub fn leave(&self, via: Option<IpAddr>) {
        if let Some(ip) = via {
            let mut counts = lock(&self.via);
            if let Some(count) = counts.get_mut(&ip) {
                *count -= 1;
                if *count == 0 {
                    counts.remove(&ip);
                }
            }
        }
        self.sockets.fetch_sub(1, Ordering::SeqCst);
        self.push.send(Push::Presence(self.clients())).ok();
    }

    /// Joined sockets that reached this share through `ip`.
    pub fn connected_via(&self, ip: IpAddr) -> usize {
        lock(&self.via).get(&ip).copied().unwrap_or(0)
    }

    pub fn ended(&self) -> Option<EndReason> {
        *lock(&self.ended)
    }

    fn end(&self, reason: EndReason) {
        lock(&self.ended).get_or_insert(reason);
        self.cancel.cancel();
    }
}

/// The result of presenting a share id and secret.
pub enum Access {
    Granted(Arc<Share>),
    /// The secret was right, but the share has been revoked.
    Ended(EndReason),
    Denied,
}

#[derive(Default)]
pub struct Grants {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    live: HashMap<String, Arc<Share>>,
    revoked: VecDeque<(String, Secret, EndReason)>,
}

impl Grants {
    fn inner(&self) -> MutexGuard<'_, Inner> {
        lock(&self.inner)
    }

    pub fn insert(&self, share: Arc<Share>) {
        self.inner().live.insert(share.id.clone(), share);
    }

    pub fn get(&self, id: &str) -> Option<Arc<Share>> {
        self.inner().live.get(id).cloned()
    }

    pub fn by_file(&self, file_id: i64) -> Option<Arc<Share>> {
        self.inner()
            .live
            .values()
            .find(|share| share.file_id == file_id)
            .cloned()
    }

    pub fn all(&self) -> Vec<Arc<Share>> {
        let mut shares: Vec<_> = self.inner().live.values().cloned().collect();
        shares.sort_by_key(|share| share.file_id);
        shares
    }

    pub fn is_empty(&self) -> bool {
        self.inner().live.is_empty()
    }

    pub fn authorize(&self, id: &str, secret: &str) -> Access {
        let inner = self.inner();
        if let Some(share) = inner.live.get(id) {
            return if share.secret.matches(secret) {
                Access::Granted(share.clone())
            } else {
                Access::Denied
            };
        }
        inner
            .revoked
            .iter()
            .find(|(revoked, kept, _)| revoked == id && kept.matches(secret))
            .map_or(Access::Denied, |(_, _, reason)| Access::Ended(*reason))
    }

    /// Remove the grant and cancel everything serving it.
    pub fn revoke(&self, id: &str, reason: EndReason) -> Option<Arc<Share>> {
        let share = {
            let mut inner = self.inner();
            let share = inner.live.remove(id)?;
            inner
                .revoked
                .push_back((id.to_string(), share.secret.clone(), reason));
            while inner.revoked.len() > REVOKED_KEPT {
                inner.revoked.pop_front();
            }
            share
        };
        share.end(reason);
        Some(share)
    }
}
