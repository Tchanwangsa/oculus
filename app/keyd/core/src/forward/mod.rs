//! The `forward` op's HTTP half: one request to a route's fixed origin, with
//! keyd attaching the credential.
//!
//! A route (`route`) names the origin, the path prefix every request must
//! stay under, and how the credential rides: a Bearer key for the three cloud
//! services, or a login session (Canvas's `Cookie`, Ed's `x-token`). The
//! client names a path (`path` rules), a method and a few headers (`call`);
//! the answer's status, headers and body come back as the origin sent them,
//! so each client's own 401/429/quota handling keeps working. Redirects are
//! returned, never followed, so a credential cannot leave its origin
//! (`upstream`).
//!
//! The cloud routes have no timeout beyond the 30 s connect: an embed or a
//! parse takes minutes (docs/architecture.md). A session route has a stall
//! detector instead of a deadline: one read of 180 s with no bytes fails that
//! forward, however long a download runs.

mod call;
mod path;
mod rejection;
mod route;
mod upstream;

pub use call::Call;
pub use rejection::session_rejected;
pub use route::{Auth, Route, Routes, SESSION_READ_TIMEOUT};
pub use upstream::{read_capped, Opened, Upstream};

/// The names a client gives `forward` for the session routes.
pub const CANVAS: &str = "canvas";
pub const ED: &str = "ed";
