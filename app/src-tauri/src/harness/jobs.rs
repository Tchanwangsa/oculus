//! Which agent runs which job, on what model, at what reasoning level.
//!
//! Nothing is defaulted out of sight: a non-chat job names its provider, model
//! and level, set in Settings → AI with the composer's `ModelPicker`. Stored as
//! one JSON value under [`SETTINGS_KEY`] (`getJobModels`/`setJobModels` in
//! `app/src/lib/db.ts`); an unreadable value costs the job its config, not its run.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::Provider;

/// The `settings` key holding the whole registry — one object, one row.
pub const SETTINGS_KEY: &str = "job_models";

/// A model-backed job that is not a chat turn. Adding one is a variant, a
/// default, and a row in the frontend's `JOBS` list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    /// `oculus lecture chapters` / `lecture_find_chapters`.
    LectureChapters,
    /// `oculus lecture end` / `lecture_find_end`: one tool-less turn.
    LectureEnd,
    /// The one-line naming turn after a thread's first exchange.
    ThreadNaming,
    /// The document editor's inline completion (`document_suggest`).
    DocumentSuggestions,
}

impl Job {
    /// The key inside the stored object (camelCase, like the webview's JSON).
    pub fn key(self) -> &'static str {
        match self {
            Job::LectureChapters => "lectureChapters",
            Job::LectureEnd => "lectureEnd",
            Job::ThreadNaming => "threadNaming",
            Job::DocumentSuggestions => "documentSuggestions",
        }
    }
}

/// One job's agent, model and level; `reasoning_effort` is None only for a model with none.
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
pub fn default_selection(job: Job) -> JobSelection {
    match job {
        Job::LectureChapters => JobSelection {
            provider: Provider::Codex,
            model: "gpt-5.6-luna".into(),
            reasoning_effort: Some("xhigh".into()),
        },
        Job::LectureEnd => JobSelection {
            provider: Provider::Claude,
            // Provisional, until the lecture-end eval picks the default.
            model: "claude-haiku-4-5-20251001".into(),
            reasoning_effort: None,
        },
        Job::ThreadNaming => JobSelection {
            provider: Provider::Claude,
            // Haiku declares no effort levels, so none is asked for.
            model: "claude-haiku-4-5-20251001".into(),
            reasoning_effort: None,
        },
        Job::DocumentSuggestions => JobSelection {
            provider: Provider::Claude,
            // Sonnet 5.5: `sonnet` in the CLI's catalogue, stored resolved.
            model: "claude-sonnet-5-5".into(),
            reasoning_effort: Some("medium".into()),
        },
    }
}

/// The job's configured selection, or its default when anything about the
/// stored row is unreadable.
pub async fn selection(pool: &SqlitePool, job: Job) -> JobSelection {
    let stored = crate::store::setting(pool, SETTINGS_KEY)
        .await
        .ok()
        .flatten();
    stored
        .as_deref()
        .and_then(|raw| from_json(raw, job))
        .unwrap_or_else(|| default_selection(job))
}

/// The parse half of [`selection`], testable without a database.
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
        assert_eq!(
            (c.provider, c.model.as_str(), c.effort()),
            (Provider::Codex, "gpt-5.6-luna", Some("xhigh"))
        );
        let n = default_selection(Job::ThreadNaming);
        assert_eq!(n.provider, Provider::Claude);
        assert_eq!(
            (n.model.as_str(), n.effort()),
            ("claude-haiku-4-5-20251001", None)
        );
        let e = default_selection(Job::LectureEnd);
        assert_eq!(
            (e.provider, e.model.as_str(), e.effort()),
            (Provider::Claude, "claude-haiku-4-5-20251001", None)
        );
        assert_eq!(Job::LectureEnd.key(), "lectureEnd");
        let d = default_selection(Job::DocumentSuggestions);
        assert_eq!(
            (d.provider, d.model.as_str(), d.effort()),
            (Provider::Claude, "claude-sonnet-5-5", Some("medium"))
        );
    }

    #[test]
    fn document_suggestions_have_their_own_registry_key() {
        let body = r#"{"documentSuggestions":{"provider":"opencode","model":"openrouter/x","reasoningEffort":null}}"#;
        let s = from_json(body, Job::DocumentSuggestions).expect("the suggestions key resolves");
        assert_eq!(
            (s.provider, s.model.as_str()),
            (Provider::Opencode, "openrouter/x")
        );
        assert!(from_json(body, Job::ThreadNaming).is_none());
    }
}
