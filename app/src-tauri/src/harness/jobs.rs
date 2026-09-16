//! Which agent runs which job, on what model, at what reasoning level.
//!
//! The composer's rule generalised past chat: nothing is defaulted out of
//! sight. A model-backed job that is not a conversation — naming a thread,
//! chaptering a lecture — still names its provider, its model and its level
//! outright, and the row in Settings → AI that names them is the same
//! `ModelPicker` the composer uses, so what is on screen is what the CLI is
//! told.
//!
//! One JSON value in `settings` under [`SETTINGS_KEY`], written by the
//! frontend (`getJobModels` / `setJobModels` in `app/src/lib/db.ts`) and read
//! here, beside the app's other settings rows. The read is tolerant on
//! purpose: a half-written or older value should cost the job its
//! configuration, not its run.

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use super::Provider;

/// The `settings` key holding the whole registry — one object, one row.
pub const SETTINGS_KEY: &str = "job_models";

/// A model-backed job that is not a chat turn.
///
/// The variants are the registry's keys, so adding a job is a variant, a
/// default, and a row in the frontend's `JOBS` list — nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    /// `oculus lecture chapters` / `lecture_find_chapters`.
    LectureChapters,
    /// `oculus lecture recap` / `lecture_write_recap`.
    LectureRecap,
    /// The one-line naming turn after a thread's first exchange.
    ThreadNaming,
}

impl Job {
    /// The key inside the stored object. camelCase, like the rest of the
    /// JSON this side shares with the webview.
    pub fn key(self) -> &'static str {
        match self {
            Job::LectureChapters => "lectureChapters",
            Job::LectureRecap => "lectureRecap",
            Job::ThreadNaming => "threadNaming",
        }
    }
}

/// One job's agent, model and reasoning level. Every field is answered:
/// `reasoning_effort` is `None` only when the model takes no level at all.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSelection {
    pub provider: Provider,
    pub model: String,
    pub reasoning_effort: Option<String>,
}

impl JobSelection {
    pub fn effort(&self) -> Option<&str> {
        self.reasoning_effort.as_deref()
    }
}

/// What a job runs on until someone says otherwise.
///
/// Chaptering is a long look at fifty slide frames and a transcript, which is
/// what the reasoning level is for. Naming is one line about a clipped
/// exchange, so it is the cheapest model in the catalogue — the same model
/// this was a `TITLE_MODEL_CLAUDE` constant for.
pub fn default_selection(job: Job) -> JobSelection {
    match job {
        Job::LectureChapters => JobSelection {
            provider: Provider::Codex,
            model: "gpt-5.6-luna".into(),
            reasoning_effort: Some("xhigh".into()),
        },
        Job::LectureRecap => JobSelection {
            provider: Provider::Codex,
            model: "gpt-5.6-luna".into(),
            reasoning_effort: Some("medium".into()),
        },
        Job::ThreadNaming => JobSelection {
            provider: Provider::Claude,
            model: "claude-haiku-4-5".into(),
            reasoning_effort: Some("low".into()),
        },
    }
}

/// The job's configured selection, or its default.
///
/// Anything unreadable — no row, bad JSON, a key that is not there, a
/// provider this build does not know, an empty model — falls back to the
/// default rather than failing the job. A settings row is not worth a run.
pub async fn selection(pool: &SqlitePool, job: Job) -> JobSelection {
    let stored = sqlx::query("SELECT value FROM settings WHERE key = ?1")
        .bind(SETTINGS_KEY)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .map(|r| r.get::<String, _>("value"));
    stored
        .as_deref()
        .and_then(|raw| from_json(raw, job))
        .unwrap_or_else(|| default_selection(job))
}

/// The parse half of [`selection`], kept separate so it can be tested without
/// a database.
fn from_json(raw: &str, job: Job) -> Option<JobSelection> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let picked: JobSelection = serde_json::from_value(value.get(job.key())?.clone()).ok()?;
    if picked.model.trim().is_empty() {
        return None;
    }
    Some(picked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(body: &str) -> Option<JobSelection> {
        from_json(body, Job::LectureChapters)
    }

    #[test]
    fn a_configured_job_is_read_back() {
        let s = json(r#"{"lectureChapters":{"provider":"claude","model":"claude-opus-5","reasoningEffort":"max"}}"#)
            .expect("a well-formed row parses");
        assert_eq!(s.provider, Provider::Claude);
        assert_eq!(s.model, "claude-opus-5");
        assert_eq!(s.effort(), Some("max"));
    }

    #[test]
    fn a_model_that_takes_no_level_keeps_its_null() {
        let s = json(r#"{"lectureChapters":{"provider":"codex","model":"gpt-5.6-luna","reasoningEffort":null}}"#)
            .expect("a null level is a level");
        assert_eq!(s.effort(), None);
    }

    #[test]
    fn another_jobs_row_is_not_this_jobs() {
        assert!(json(r#"{"threadNaming":{"provider":"codex","model":"gpt-5.6-luna","reasoningEffort":"low"}}"#).is_none());
        let s = from_json(
            r#"{"threadNaming":{"provider":"codex","model":"gpt-5.6-luna","reasoningEffort":"low"}}"#,
            Job::ThreadNaming,
        )
        .expect("the key it does have");
        assert_eq!(s.model, "gpt-5.6-luna");
    }

    #[test]
    fn recap_has_its_own_registry_key() {
        let s = from_json(
            r#"{"lectureRecap":{"provider":"codex","model":"gpt-5.6-luna","reasoningEffort":"medium"}}"#,
            Job::LectureRecap,
        )
        .expect("the recap key resolves");
        assert_eq!(s.provider, Provider::Codex);
        assert_eq!(s.effort(), Some("medium"));
        assert!(from_json(
            r#"{"lectureRecap":{"provider":"codex","model":"gpt-5.6-luna","reasoningEffort":"medium"}}"#,
            Job::LectureChapters,
        )
        .is_none());
    }

    #[test]
    fn malformed_or_partial_values_fall_back_rather_than_fail() {
        for body in [
            "not json at all",
            "[]",
            "{}",
            r#"{"lectureChapters":{}}"#,
            r#"{"lectureChapters":{"provider":"gemini","model":"x","reasoningEffort":null}}"#,
            r#"{"lectureChapters":{"provider":"codex","model":"  ","reasoningEffort":null}}"#,
            r#"{"lectureChapters":"gpt-5.6-luna"}"#,
        ] {
            assert!(json(body).is_none(), "{body} should not resolve");
        }
    }

    #[test]
    fn the_defaults_are_the_ones_the_cli_shipped_with() {
        let c = default_selection(Job::LectureChapters);
        assert_eq!((c.provider, c.model.as_str(), c.effort()), (Provider::Codex, "gpt-5.6-luna", Some("xhigh")));
        let n = default_selection(Job::ThreadNaming);
        assert_eq!(n.provider, Provider::Claude);
        assert_eq!(n.model, "claude-haiku-4-5");
        let r = default_selection(Job::LectureRecap);
        assert_eq!(
            (r.provider, r.model.as_str(), r.effort()),
            (Provider::Codex, "gpt-5.6-luna", Some("medium"))
        );
    }
}
