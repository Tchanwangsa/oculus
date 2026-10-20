//! What the sharing commands return and the `companion-state` event carries.
//! Only `ShareInfo` holds a secret (inside each endpoint's link), and only
//! command results carry it; none of these types implement `Debug`.

use super::endpoints::Endpoint;
use crate::library::files::canvases::authority::events::Phase;
use serde::Serialize;

/// A live share, with a link per endpoint.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareInfo {
    pub file_id: i64,
    pub share_id: String,
    pub epoch: String,
    pub port: u16,
    pub generation: u64,
    pub endpoints: Vec<Endpoint>,
    pub clients: usize,
    pub phase: Phase,
}

/// A share as the event reports it: no link, no secret.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareSummary {
    pub file_id: i64,
    pub share_id: String,
    pub epoch: String,
    pub clients: usize,
    pub phase: Phase,
}

/// The `companion-state` payload.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionState {
    pub running: bool,
    pub port: Option<u16>,
    pub ipv6: bool,
    pub generation: u64,
    /// The addresses a link could use, as bare `/c/` pages without a key;
    /// a share's own links come from `companion_shares`.
    pub endpoints: Vec<Endpoint>,
    pub shares: Vec<ShareSummary>,
}

/// `companion_endpoints`: the addresses a link could use, without a share.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoints {
    pub port: Option<u16>,
    pub generation: u64,
    pub endpoints: Vec<Endpoint>,
}
