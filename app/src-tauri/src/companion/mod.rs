//! Device sharing: a LAN HTTP/WebSocket server that lets a browser on another
//! device (an iPad, a phone) open a shared canvas and edit it through the
//! canvas authority. Off at launch; the server starts with the first share
//! and stops with the last. The wire protocol, the security boundary and the
//! limits are in docs/device-sharing.md.
//!
//! - `core` owns the shares and the server's lifecycle.
//! - `grants` is the share registry: ids, secrets, change logs, sockets.
//! - `server` is the transport: bind, accept, route, WebSockets.
//! - `bridge` adapts requests and authority events to each other.
//! - `assets` serves the browser bundle, `endpoints` this Mac's addresses.

mod assets;
mod bridge;
pub mod commands;
mod core;
mod endpoints;
mod error;
mod grants;
mod limits;
mod server;
mod wire;

#[cfg(test)]
mod tests;

pub use self::core::Companion;
