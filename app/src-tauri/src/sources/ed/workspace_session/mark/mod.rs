//! Mark: a real submission that counts in Ed's records. Only the pane's Mark
//! button reaches it (`ed_workspace_mark`), after its own confirm dialogs;
//! nothing retries it, and no sync, reconnect, agent or CLI calls it. The
//! history is Ed's own list of the student's submissions (`history`), and a
//! submission's code is read back from a temporary copy (`code`).

pub mod code;
pub mod diff;
pub mod history;
pub mod standing;
pub mod submission;

use std::sync::TryLockError;

use self::submission::Submission;
use super::queue::Place;
use super::session::Session;
use super::snapshot;

impl Session {
    /// Submit the workspace at its latest commit, once the saves asked
    /// before this have landed.
    pub fn mark(&self, place: Place) -> Result<Submission, String> {
        let _marking = match self.marking.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::Poisoned(e)) => e.into_inner(),
            Err(TryLockError::WouldBlock) => return Err("A Mark is already in progress".into()),
        };
        drop(place.wait());
        let core = &self.core;
        core.online()
            .ok_or("The workspace is offline: Mark needs Ed's server")?;
        let hash = snapshot::latest(core, &self.out)?;
        submission::normalise(&core.ed.mark(core.challenge_id, &hash)?)
    }
}
