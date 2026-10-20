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
        let submission = submission::normalise(&core.ed.mark(core.challenge_id, &hash)?)?;
        self.complete_slide_if_passed(&submission);
        Ok(submission)
    }

    /// Ed's web app completes the lesson slide once a submission passes; a
    /// Mark from here does the same so the slide is done on Ed too. A failure
    /// is logged: the submission stands either way.
    fn complete_slide_if_passed(&self, submission: &Submission) {
        let Some(slide_id) = self.slide.slide_id else {
            return;
        };
        if !matches!(submission.status.as_str(), "passed" | "submitted") {
            return;
        }
        if let Err(e) = self.core.ed.complete_slide(slide_id) {
            elog!(
                "[oculus] ed workspace {}: completing slide {slide_id}: {e}",
                self.core.challenge_id
            );
        }
    }
}
