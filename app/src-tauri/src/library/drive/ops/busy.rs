//! A move or delete never runs under a job: a file being parsed or embedded
//! right now, or a canvas shared to a device, is refused with an `io` error
//! that says so. The parse and embed claims (`parse::InFlight`, keyed by the
//! PDF a file parses as) are taken for every file the command touches and
//! held until its rows are written, so no parse starts on a file mid-move;
//! one asked for meanwhile waits, then finds its path gone and fails visibly.

use std::path::{Path, PathBuf};

use super::error::{shown, DriveError, DriveResult};
use crate::library::files::canvases::authority;
use crate::parse::{InFlight, InFlightClaim};

/// The claims a command holds; dropping it frees the files.
pub struct Held {
    _parse: Vec<InFlightClaim<'static>>,
    _embed: Vec<InFlightClaim<'static>>,
}

/// Run `work` with every one of `files` (data-dir-relative) free of jobs
/// and shared sessions, returning the claims to hold until the rows follow.
pub fn fenced<T>(
    data_dir: &Path,
    files: &[String],
    work: impl FnOnce() -> DriveResult<T>,
) -> DriveResult<(T, Held)> {
    authority::global().while_unshared(data_dir, files, |shared| {
        if let Some(rel) = shared {
            return Err(DriveError::io(format!(
                "{} is shared to a device; stop sharing it first",
                shown(&rel)
            )));
        }
        let held = hold(data_dir, files)?;
        Ok((work()?, held))
    })
}

fn hold(data_dir: &Path, files: &[String]) -> DriveResult<Held> {
    let keys: Vec<PathBuf> = files
        .iter()
        .map(|rel| crate::sync::parse_key(data_dir, rel))
        .collect();
    let busy = |key: PathBuf, doing: &str| {
        let name = key
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        DriveError::io(format!(
            "{} is being {doing} right now; try again when it finishes",
            shown(&name)
        ))
    };
    let parse = InFlight::shared()
        .try_claim_all(&keys)
        .map_err(|key| busy(key, "parsed"))?;
    let embed = InFlight::embedding()
        .try_claim_all(&keys)
        .map_err(|key| busy(key, "embedded"))?;
    Ok(Held {
        _parse: parse,
        _embed: embed,
    })
}
