//! Mark: a real submission that counts in Ed's records. Only the pane's Mark
//! button reaches it (`ed_workspace_mark`), after its own confirm dialogs;
//! nothing retries it, and no sync, reconnect, agent or CLI calls it. The
//! id goes into the sync record, because Ed lists no submissions for a
//! student: those ids are the history (`history`).

pub mod diff;
pub mod history;
pub mod standing;
pub mod submission;

use std::sync::TryLockError;

use self::submission::Submission;
use super::events::Body;
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
        let (workspace_id, _) = core
            .online()
            .ok_or("The workspace is offline: Mark needs Ed's server")?;
        let hash = snapshot::latest(core, &self.out)?;
        let raw = core.ed.mark(core.challenge_id, &hash)?;
        if let Some(id) = raw["id"].as_i64() {
            let kept = core.record(&workspace_id, |state| {
                if !state.submissions.contains(&id) {
                    state.submissions.push(id);
                }
            });
            if let Err(e) = kept {
                elog!(
                    "[oculus] ed workspace {}: submission {id} was not recorded: {e}",
                    core.challenge_id
                );
                core.emit(Body::Error {
                    message: format!(
                        "Ed took the submission, but Oculus could not keep it in its history: {e}"
                    ),
                });
            }
        }
        submission::normalise(&raw)
    }
}
