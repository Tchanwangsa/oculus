//! What `oculus keyd status` reports.

use std::path::{Path, PathBuf};

use keyd_core::client::Client;
use keyd_core::paths;
use keyd_core::platform;

use super::candidate::{candidate, installed_stamp, no_candidate_reason, source_hash_of};

#[derive(Debug, serde::Serialize)]
pub struct Status {
    pub plist: PathBuf,
    /// The binary the registration runs, when one is installed.
    pub program: Option<String>,
    pub loaded: bool,
    pub installed_hash: Option<String>,
    /// What this build would install, and its source hash or why it has none.
    pub candidate: Option<PathBuf>,
    pub candidate_hash: Option<String>,
    pub candidate_error: Option<String>,
    pub socket: PathBuf,
    /// keyd's `ping` reply: version, source hash, pid.
    pub ping: Option<serde_json::Value>,
    pub ping_error: Option<String>,
    pub vault: PathBuf,
    pub vault_bytes: Option<u64>,
}

/// Never reads a secret: `ping` is the one op it sends, and keyd answers it
/// without opening the vault or the keychain.
pub fn status(data_dir: &Path) -> Result<Status, String> {
    let registration = platform::registrar().status()?;
    let candidate = candidate();
    let (candidate_hash, candidate_error) = match &candidate {
        Some(c) => match source_hash_of(c) {
            Ok(h) => (Some(h), None),
            Err(e) => (None, Some(e)),
        },
        None => (None, no_candidate_reason()),
    };
    let (ping, ping_error) = match Client::at(data_dir).ping() {
        Ok(v) => (Some(v), None),
        Err(e) => (None, Some(e)),
    };
    let vault = paths::vault(data_dir);
    Ok(Status {
        loaded: registration.loaded,
        plist: registration.path,
        program: registration.program,
        installed_hash: installed_stamp(data_dir),
        candidate,
        candidate_hash,
        candidate_error,
        vault_bytes: std::fs::metadata(&vault).ok().map(|m| m.len()),
        vault,
        socket: paths::socket(data_dir),
        ping,
        ping_error,
    })
}
