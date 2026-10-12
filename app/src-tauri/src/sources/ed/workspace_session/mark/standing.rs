//! The student's standing on a challenge, read live: the slide's snapshot
//! leaves it out because it goes stale (`docs/sync.md`).

use serde::Serialize;
use serde_json::Value;

use crate::sources::ed::Ed;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Standing {
    pub score: Option<f64>,
    pub status: Option<String>,
    pub attempts_remaining: Option<i64>,
    pub due_at: Option<String>,
}

/// `GET /challenges/<ch>`; a challenge with no `due_at` takes `slide_due`,
/// the slide's front matter (due with its lesson).
pub fn standing(ed: &Ed, challenge_id: i64, slide_due: Option<String>) -> Result<Standing, String> {
    Ok(of(&ed.challenge(challenge_id)?, slide_due))
}

pub fn of(challenge: &Value, slide_due: Option<String>) -> Standing {
    let text = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_string);
    Standing {
        score: challenge["score"].as_f64(),
        status: text(&challenge["status"]),
        attempts_remaining: challenge["attempts_remaining"].as_i64(),
        due_at: text(&challenge["due_at"]).or(slide_due),
    }
}
