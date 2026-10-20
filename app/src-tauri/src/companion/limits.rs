//! Every bound the companion server enforces. The timings tests shorten live
//! in `Limits`; the rest are constants.

use std::time::Duration;

/// The port a share link uses when it is free, so links stay the same across
/// launches; the next `PORT_ATTEMPTS - 1` ports are tried after it.
pub const PREFERRED_PORT: u16 = 43821;
pub const PORT_ATTEMPTS: u16 = 20;

/// hyper's read buffer, which bounds a request head; larger heads get 431.
pub const HEADER_BYTES: usize = 16 * 1024;
/// Open connections of any kind; one more is closed on accept.
pub const CONNECTIONS: usize = 128;
/// WebSockets that have not yet sent a valid `hello`, across all shares.
pub const UNAUTHENTICATED_SOCKETS: usize = 8;
/// Authenticated WebSockets per share.
pub const SOCKETS_PER_SHARE: usize = 16;
/// Any one WebSocket message or frame from a client.
pub const FRAME_BYTES: usize = 64 * 1024;
/// A change entry at most this size is pushed inline; larger ones as a
/// `revision` frame the client follows with `/changes`.
pub const INLINE_CHANGE_BYTES: usize = 32 * 1024;
/// Pushes queued per socket before it is told to catch up instead.
pub const PUSH_QUEUE: usize = 64;
/// The per-share change log keeps whichever is smaller.
pub const LOG_REVISIONS: usize = 256;
pub const LOG_BYTES: usize = 64 * 1024 * 1024;
/// Revoked shares remembered so their clients get `session-ended`, not 403.
pub const REVOKED_KEPT: usize = 32;
/// A socket write to a vanished peer must not park its task.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
/// The Close frame on revoke or shutdown is best effort.
pub const CLOSE_TIMEOUT: Duration = Duration::from_millis(250);
/// Tasks get this long to finish on shutdown before the runtime drops them.
pub const SHUTDOWN_GRACE: Duration = Duration::from_millis(800);
pub const KEEPALIVE_TIME: Duration = Duration::from_secs(30);
pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);
/// Client ids and op ids longer than this are refused.
pub const ID_CHARS: usize = 128;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Slow-loris bound on a request head, and on an idle keep-alive wait.
    pub header_timeout: Duration,
    /// A commit body, read in full before it is parsed.
    pub commit_bytes: usize,
    /// No body progress for this long ends the request.
    pub body_idle: Duration,
    /// The first WebSocket frame must be a `hello` within this.
    pub hello_timeout: Duration,
    pub ping_every: Duration,
    /// No frame from the client for this long closes the socket.
    pub dead_after: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            header_timeout: Duration::from_secs(10),
            commit_bytes: 80 * 1024 * 1024,
            body_idle: Duration::from_secs(30),
            hello_timeout: Duration::from_secs(5),
            ping_every: Duration::from_secs(20),
            dead_after: Duration::from_secs(45),
        }
    }
}
