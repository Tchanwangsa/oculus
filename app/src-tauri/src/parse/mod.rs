//! The seam every PDF parser plugs into: MinerU cloud over HTTPS, or MinerU's
//! own server (installed and started by the user) over loopback. Nothing here
//! may assume the cloud; an API root, a token or an upload ceiling arrives as
//! configuration.
//!
//! The seam owns the contract, not the parsing: the on-disk artifact layout,
//! the version that marks a file done, the error vocabulary the failure UI
//! reads, and the order the artifacts hit the disk. See `docs/parsing.md`.

mod config;
mod error;
mod health;
mod inflight;
mod parser;
mod progress;
mod record;
mod skips;
mod staging;
#[cfg(test)]
mod tests;

pub mod commands;
pub mod events;
pub mod mineru;

pub use config::{
    backend, parse_config, CredentialSource, Engine, ParseConfig, CLOUD_BASE_URL, LOCAL_BASE_URL,
};
pub use error::{check_size, ParseError, CONVERSION_FAILED, SHEET_UNREADABLE};
pub use health::{preflight, Health};
pub use inflight::{InFlight, InFlightClaim};
pub use parser::Parser;
pub use progress::{Phase, Progress};
pub use record::{
    images_dir_for, md_path, pages_path, parse_mode, read_record, ParseOutput, ParsePage,
};
pub use skips::{check_skipped, Skips};
pub use staging::ImageStaging;

/// The version stamped into every `.pages.json`. It moves only when the
/// artifacts themselves change shape — not when a backend or the app changes.
pub const PARSER_VERSION: u32 = 2;

/// The one parse tier. Records on disk carry the field, so it round-trips.
pub const MODE: &str = "quality";
