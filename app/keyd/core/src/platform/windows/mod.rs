//! Windows uses current-user named pipes, DPAPI and an app-started broker.
mod endpoint;
pub mod files;
#[cfg(feature = "server")]
mod peer;
#[cfg(feature = "client")]
mod registrar;
#[cfg(feature = "server")]
mod secret_store;
mod security;

pub(crate) use endpoint::{accept_any, activated, bind, connect, set_timeout, Listener, Stream};
#[cfg(all(feature = "client", feature = "test-support"))]
pub use endpoint::{register_test_broker, TestBrokerRegistration};
#[cfg(feature = "server")]
pub(crate) fn peer_check(policy: super::Policy) -> Box<dyn super::PeerCheck> {
    Box::new(peer::WindowsPeers::new(policy))
}
#[cfg(feature = "server")]
pub(crate) fn master_key() -> Box<dyn crate::vault::KeySource> {
    Box::new(secret_store::Dpapi)
}
#[cfg(feature = "server")]
pub(crate) fn legacy_items() -> Box<dyn crate::vault::LegacySource> {
    Box::new(secret_store::NoLegacy)
}
#[cfg(feature = "client")]
pub(crate) fn registrar() -> &'static dyn super::Registrar {
    &registrar::Resident
}

pub fn pipe_name() -> std::io::Result<std::path::PathBuf> {
    Ok(format!(
        r"\\.\pipe\com.tchan.oculus.keyd.{}",
        security::current_sid()?
    )
    .into())
}
