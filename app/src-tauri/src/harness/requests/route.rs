//! The way back to the callback a CLI is waiting on. Each bridge implements
//! [`Route`] for its own wire; a route lives only as long as the process
//! that asked, which is why a request is never answered after a restart.

use std::fmt;
use std::sync::Arc;

use super::RequestOption;

/// What a reply's write proved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    /// The provider has it now: the write itself is the proof.
    Acknowledged,
    /// Written; the provider will say when it has it (Codex's
    /// `serverRequest/resolved`).
    Written,
    /// Nothing reached the provider: its process was gone before the write.
    Failed(String),
    /// The write broke partway, so the provider may have it.
    Uncertain(String),
}

/// A bridge's way to one waiting callback. Called off the consumer's
/// thread, never under the live-session map's lock.
pub trait Route: Send + Sync {
    /// Answer the callback with `option`.
    fn reply(&self, option: &RequestOption) -> Reply;

    /// Decline it without asking anyone: the app is giving the request up
    /// (its session was replaced, no turn is open to show it in, or it
    /// names a protected path). Best effort; a dead process needs nothing.
    fn withdraw(&self);
}

/// A [`Route`] carried on an event and kept by the registry. Two are equal
/// only when they are the same route.
#[derive(Clone)]
pub struct LiveRoute(pub Arc<dyn Route>);

impl fmt::Debug for LiveRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LiveRoute")
    }
}

impl PartialEq for LiveRoute {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
