//! The sharing lifecycle. The server starts with the first share and stops
//! with the last; stopping a share ends only that grant. One lock (`server`)
//! orders every start, stop and revoke. Lock order: `server`, then an
//! authority session's lock, then the grants; the authority's event callback
//! runs under a session lock, so it takes only the grants and hands anything
//! needing `server` to a thread. While the server runs, `core` follows the
//! address observer: a changed list publishes a new `companion-state` and
//! every link is rebuilt from its unchanged grant on the next read; no
//! listener, socket, token or canvas is touched.

use super::assets::Assets;
use super::bridge;
use super::endpoints::{self, Addresses, Event};
use super::error::{Code, CompanionError};
use super::grants::{EndReason, Grants, Share};
use super::limits::{Limits, PORT_ATTEMPTS, PREFERRED_PORT};
use super::server::Server;
use super::wire::{CompanionState, Endpoints, ShareInfo, ShareSummary};
use crate::library::files::canvases::authority::events::Phase;
use crate::library::files::canvases::authority::Authority;
use crate::runtime::network::{self, Observer, Subscription};
use std::net::IpAddr;
use std::sync::{Arc, Mutex, MutexGuard};

pub struct Config {
    pub preferred_port: u16,
    pub port_attempts: u16,
    pub limits: Limits,
    pub network: Observer,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            preferred_port: PREFERRED_PORT,
            port_attempts: PORT_ATTEMPTS,
            limits: Limits::default(),
            network: network::global().clone(),
        }
    }
}

/// Where the server listens while it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listening {
    pub port: u16,
    /// An IPv6 listener is bound beside the required IPv4 one.
    pub v6: bool,
}

type Notify = Arc<dyn Fn(&CompanionState) + Send + Sync>;

pub struct Core {
    pub authority: &'static Authority,
    pub grants: Grants,
    pub assets: Assets,
    pub addresses: Addresses,
    pub limits: Limits,
    preferred_port: u16,
    port_attempts: u16,
    server: Mutex<Option<Server>>,
    listening: Mutex<Option<Listening>>,
    /// The observer subscription, held while the server runs.
    watch: Mutex<Option<Subscription>>,
    notify: Mutex<Option<Notify>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// The app's handle on sharing; cheap to clone.
#[derive(Clone)]
pub struct Companion {
    core: Arc<Core>,
}

impl Companion {
    /// Sharing over `authority`, off until the first share. Subscribes to the
    /// authority's events for the life of the process.
    pub fn new(authority: &'static Authority, assets: Assets, config: Config) -> Self {
        let core = Arc::new(Core {
            authority,
            grants: Grants::default(),
            assets,
            addresses: Addresses::new(config.network),
            limits: config.limits,
            preferred_port: config.preferred_port,
            port_attempts: config.port_attempts,
            server: Mutex::new(None),
            listening: Mutex::new(None),
            watch: Mutex::new(None),
            notify: Mutex::new(None),
        });
        let weak = Arc::downgrade(&core);
        authority.subscribers().subscribe(move |event| {
            if let Some(core) = weak.upgrade() {
                bridge::events::on_event(&core, event);
            }
        });
        Self { core }
    }

    /// Call `notify` with the new state after every change.
    pub fn on_state(&self, notify: impl Fn(&CompanionState) + Send + Sync + 'static) {
        *lock(&self.core.notify) = Some(Arc::new(notify));
    }

    #[cfg(test)]
    pub(crate) fn core(&self) -> &Arc<Core> {
        &self.core
    }

    /// Share `file_id`, whose authority session must be live. A file already
    /// shared answers with its existing share.
    pub fn share_start(&self, file_id: i64) -> Result<ShareInfo, CompanionError> {
        let core = &self.core;
        let share = {
            let mut server = lock(&core.server);
            let existing = core.grants.by_file(file_id);
            if let Some(share) = existing.filter(|share| share.head().is_some()) {
                drop(server);
                return core.info(&share).ok_or_else(CompanionError::not_live);
            }
            if server.is_none() {
                let started = Server::start(core.clone(), core.preferred_port, core.port_attempts)
                    .map_err(|error| {
                        CompanionError::new(
                            Code::Server,
                            format!("Sharing could not start a server: {error}"),
                        )
                    })?;
                *lock(&core.listening) = Some(started.listening());
                *server = Some(started);
                core.follow_network();
            }
            let root = server.as_ref().map(Server::root_token).unwrap_or_default();
            let Some(share) = Share::new(file_id, root.child_token()).map(Arc::new) else {
                core.stop_if_idle(&mut server);
                return Err(CompanionError::new(
                    Code::Random,
                    "Sharing could not make a secret: the system random source failed.",
                ));
            };
            // Registered before the base is read, so a change committed in
            // between is logged; `establish` drops what the base already has.
            core.grants.insert(share.clone());
            let live = core.authority.snapshot(file_id).map(|snapshot| {
                let phase = if snapshot.paused {
                    Phase::Paused
                } else {
                    Phase::Live
                };
                share.establish(&snapshot.epoch, snapshot.revision, phase);
            });
            if live.is_err() || share.cancel.is_cancelled() {
                core.grants.revoke(&share.id, EndReason::Ended);
                core.stop_if_idle(&mut server);
                return Err(CompanionError::not_live());
            }
            share
        };
        core.notify();
        core.info(&share).ok_or_else(CompanionError::not_live)
    }

    /// Stop sharing `file_id`: its clients get `session-ended` and its
    /// requests and sockets close. Not shared is a no-op.
    pub fn share_stop(&self, file_id: i64) {
        let core = &self.core;
        {
            let mut server = lock(&core.server);
            if let Some(share) = core.grants.by_file(file_id) {
                core.revoke(&mut server, &share.id, EndReason::Stopped);
            }
            core.stop_if_idle(&mut server);
        }
        core.notify();
    }

    /// End every share and stop the server; returns once it has stopped.
    pub fn stop_all(&self, reason: EndReason) {
        let core = &self.core;
        {
            let mut server = lock(&core.server);
            for share in core.grants.all() {
                core.revoke(&mut server, &share.id, reason);
            }
            core.stop_if_idle(&mut server);
        }
        core.notify();
    }

    pub fn shares(&self) -> Vec<ShareInfo> {
        let core = &self.core;
        core.grants
            .all()
            .iter()
            .filter_map(|share| core.info(share))
            .collect()
    }

    /// The addresses a link could use, re-read first: the Share dialog asks
    /// for these as it opens.
    pub fn endpoints(&self) -> Endpoints {
        let core = &self.core;
        core.addresses.refresh();
        let (generation, endpoints) = core.bare_endpoints();
        Endpoints {
            port: core.listening().map(|l| l.port),
            generation,
            endpoints,
        }
    }

    pub fn state(&self) -> CompanionState {
        self.core.state()
    }
}

impl Core {
    pub fn listening(&self) -> Option<Listening> {
        *lock(&self.listening)
    }

    fn revoke(&self, server: &mut Option<Server>, id: &str, reason: EndReason) {
        self.grants.revoke(id, reason);
        if let Some(server) = server.as_ref() {
            server.close_share(id);
        }
    }

    fn stop_if_idle(&self, server: &mut Option<Server>) {
        if !self.grants.is_empty() {
            return;
        }
        if let Some(stopped) = server.take() {
            lock(&self.watch).take();
            *lock(&self.listening) = None;
            stopped.shutdown();
        }
    }

    /// Publish a new state whenever the address list changes, until the
    /// server stops.
    fn follow_network(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        let subscription = self.addresses.subscribe(move |event| {
            if let (Event::Changed(_), Some(core)) = (event, weak.upgrade()) {
                core.notify();
            }
        });
        *lock(&self.watch) = Some(subscription);
    }

    /// Live sockets, across every share, that came in through `ip`.
    fn connected_via(&self, ip: IpAddr) -> usize {
        self.grants.all().iter().map(|s| s.connected_via(ip)).sum()
    }

    /// The generation and the endpoints without a share's key.
    fn bare_endpoints(&self) -> (u64, Vec<endpoints::Endpoint>) {
        let listening = self.listening();
        let (addresses, generation) = self.addresses.current();
        let endpoints = endpoints::build(
            &addresses,
            generation,
            listening.map(|l| l.port),
            listening.is_some_and(|l| l.v6),
            None,
            &|ip| self.connected_via(ip),
        );
        (generation, endpoints)
    }

    /// The authority ended `file_id`'s session: revoke its share. Called
    /// under the session's lock, so closing connections and stopping the
    /// server happen on a thread.
    pub fn end_file(self: &Arc<Self>, file_id: i64) {
        let Some(share) = self.grants.by_file(file_id) else {
            return;
        };
        self.grants.revoke(&share.id, EndReason::Ended);
        let core = self.clone();
        let spawned = std::thread::Builder::new()
            .name("companion-stop".into())
            .spawn(move || {
                {
                    let mut server = lock(&core.server);
                    if let Some(server) = server.as_ref() {
                        server.close_share(&share.id);
                    }
                    core.stop_if_idle(&mut server);
                }
                core.notify();
            });
        if let Err(error) = spawned {
            elog!("[companion] share teardown thread did not start: {error}");
        }
    }

    pub fn state(&self) -> CompanionState {
        let listening = self.listening();
        let (generation, endpoints) = self.bare_endpoints();
        CompanionState {
            running: listening.is_some(),
            port: listening.map(|l| l.port),
            ipv6: listening.is_some_and(|l| l.v6),
            generation,
            endpoints,
            shares: self
                .grants
                .all()
                .iter()
                .filter_map(|share| {
                    let (epoch, _) = share.head()?;
                    Some(ShareSummary {
                        file_id: share.file_id,
                        share_id: share.id.clone(),
                        epoch,
                        clients: share.clients(),
                        phase: share.phase(),
                    })
                })
                .collect(),
        }
    }

    pub fn notify(&self) {
        let notify = lock(&self.notify).clone();
        if let Some(notify) = notify {
            notify(&self.state());
        }
    }

    fn info(&self, share: &Share) -> Option<ShareInfo> {
        let listening = self.listening()?;
        let (epoch, _) = share.head()?;
        let (addresses, generation) = self.addresses.current();
        Some(ShareInfo {
            file_id: share.file_id,
            share_id: share.id.clone(),
            epoch,
            port: listening.port,
            generation,
            endpoints: endpoints::build(
                &addresses,
                generation,
                Some(listening.port),
                listening.v6,
                Some(&share.token()),
                &|ip| share.connected_via(ip),
            ),
            clients: share.clients(),
            phase: share.phase(),
        })
    }
}
