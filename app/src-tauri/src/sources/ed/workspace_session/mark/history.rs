//! The submissions this install made, read back from Ed newest first. Ed
//! answers bursts with 429, so the reads are spaced out.

use std::path::Path;
use std::time::Duration;

use super::submission::{normalise, Submission};
use crate::sources::ed::workspace_state;
use crate::sources::ed::Ed;

/// The gap between two reads.
const PACE: Duration = Duration::from_millis(300);

/// Each id in the challenge's record, newest first. One that cannot be read
/// is logged and left out; an error only when none could be.
pub fn submissions(ed: &Ed, data_dir: &Path, challenge_id: i64) -> Result<Vec<Submission>, String> {
    let ids = workspace_state::load(data_dir, challenge_id)
        .map(|state| state.submissions)
        .unwrap_or_default();
    let mut read = Vec::new();
    let mut failed = None;
    for (n, id) in ids.iter().rev().enumerate() {
        if n > 0 && !cfg!(test) {
            std::thread::sleep(PACE);
        }
        match ed.submission(*id).and_then(|raw| normalise(&raw)) {
            Ok(submission) => read.push(submission),
            Err(e) => {
                elog!("[oculus] ed submission {id} of challenge {challenge_id}: {e}");
                failed = Some(e);
            }
        }
    }
    match failed {
        Some(e) if read.is_empty() => Err(e),
        _ => Ok(read),
    }
}
