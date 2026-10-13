//! The student's submissions for a challenge, from Ed's own list, newest
//! first. Ed answers with each one's marked result, so one read does.

use super::submission::{normalise, Submission};
use crate::sources::ed::Ed;

/// One Ed cannot shape (no id) is logged and left out.
pub fn submissions(ed: &Ed, user_id: i64, challenge_id: i64) -> Result<Vec<Submission>, String> {
    let mut read: Vec<Submission> = ed
        .user_submissions(user_id, challenge_id)?
        .iter()
        .filter_map(|raw| match normalise(raw) {
            Ok(submission) => Some(submission),
            Err(e) => {
                elog!("[oculus] ed challenge {challenge_id}: a submission was left out: {e}");
                None
            }
        })
        .collect();
    read.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
    Ok(read)
}
