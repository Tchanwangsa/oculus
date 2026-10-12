//! Echo360 videos embedded in Canvas pages, resolved and downloaded from the
//! app on request (`docs/sync.md`). A sync never resolves an embed: the page's
//! `<echo-video>` tag is the only record until its card asks.

pub(crate) mod commands;
mod naming;
mod resolve;
mod row;
mod state;

#[cfg(test)]
mod tests;

pub use state::{EmbedBearer, EmbedCancels};
