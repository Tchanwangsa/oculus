//! Which agent runs which job, on what model, at what reasoning level.
//!
//! Nothing is defaulted out of sight: a non-chat job names an ordered list of
//! models (provider, model, level), set in Settings → Jobs with the composer's
//! `ModelPicker`. The first runs; when its run fails for any reason the next
//! one does ([`try_each`]). An empty list switches the job off, and never
//! falls back to the default, which can be an expensive model. Stored as one
//! JSON value under [`SETTINGS_KEY`] (`getJobModels`/`setJobModels` in
//! `app/src/lib/db/jobModels.ts`); an unreadable value costs the job its
//! config, not its run.

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

    /// Its name in Settings → Jobs (`JOBS` in `jobModels.ts`).
    pub fn label(self) -> &'static str {
        match self {
            Job::LectureChapters => "Lecture chapters",
            Job::LectureEnd => "Lecture end",
            Job::ThreadNaming => "Chat thread names",
            Job::DocumentSuggestions => "Document suggestions",
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

/// The job's models in the order they are tried. Empty means the job is off;
/// see [`from_json`] for what an unreadable value resolves to.
pub async fn chain(pool: &SqlitePool, job: Job) -> Vec<JobSelection> {
    let stored = crate::db::store::setting(pool, SETTINGS_KEY)
        .await
        .ok()
        .flatten();
    match stored {
        Some(raw) => from_json(&raw, job),
        None => vec![default_selection(job)],
    }
}

/// The parse half of [`chain`], testable without a database. Must match
/// `getJobModels` in `jobModels.ts`.
///
/// An array is the job's list as written: its entries that parse with a model,
/// in order, so `[]` is off and never the default. A single object is one
/// model, as long as it is readable. Anything else unreadable (bad JSON, no
/// key, another type) is the default.
fn from_json(raw: &str, job: Job) -> Vec<JobSelection> {
    let entry = serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|v| v.as_object()?.get(job.key()).cloned());
    match entry {
        Some(serde_json::Value::Array(items)) => items.into_iter().filter_map(one).collect(),
        Some(item @ serde_json::Value::Object(_)) => {
            one(item).map_or_else(|| vec![default_selection(job)], |s| vec![s])
        }
        _ => vec![default_selection(job)],
    }
}

/// One stored selection, if it parses and names a model.
fn one(value: serde_json::Value) -> Option<JobSelection> {
    let picked: JobSelection = serde_json::from_value(value).ok()?;
    (!picked.model.trim().is_empty()).then_some(picked)
}

/// Tries each model in order until one succeeds. Every failure moves on, so
/// `attempt` covers the whole turn: spawning, the reply, parsing it. The final
/// error names each model with its error, or is the only one's own error.
pub fn try_each<T>(
    job: Job,
    chain: &[JobSelection],
    mut attempt: impl FnMut(&JobSelection) -> Result<T, String>,
) -> Result<T, String> {
    if chain.is_empty() {
        return Err(off(job));
    }
    let mut failures: Vec<String> = Vec::new();
    for (i, sel) in chain.iter().enumerate() {
        match attempt(sel) {
            Ok(value) => return Ok(value),
            Err(e) => {
                if i + 1 < chain.len() {
                    eprintln!(
                        "[oculus] {}: {} failed, trying the next model: {e}",
                        job.label(),
                        sel.model
                    );
                }
                failures.push(format!("{}: {e}", sel.model));
                if chain.len() == 1 {
                    return Err(e);
                }
            }
        }
    }
    Err(format!("every model failed — {}", failures.join("; ")))
}

/// What a job with no model says when asked to run.
pub fn off(job: Job) -> String {
    format!(
        "No model is set for {} — choose one in Settings → Jobs.",
        job.label()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(body: &str) -> Vec<JobSelection> {
        from_json(body, Job::LectureChapters)
    }

    fn models(chain: &[JobSelection]) -> Vec<&str> {
        chain.iter().map(|s| s.model.as_str()).collect()
    }

    fn is_default(chain: &[JobSelection], job: Job) -> bool {
        models(chain) == [default_selection(job).model.as_str()]
    }

    fn sel(model: &str) -> JobSelection {
        JobSelection {
            provider: Provider::Claude,
            model: model.into(),
            reasoning_effort: None,
        }
    }

    #[test]
    fn a_configured_job_is_read_back() {
        let chain = json(
            r#"{"lectureChapters":[{"provider":"claude","model":"claude-opus-5","reasoningEffort":"max"}]}"#,
        );
        let [s] = chain.as_slice() else {
            panic!("one entry, got {chain:?}")
        };
        assert_eq!(s.provider, Provider::Claude);
        assert_eq!(s.model, "claude-opus-5");
        assert_eq!(s.effort(), Some("max"));
    }

    #[test]
    fn a_legacy_single_object_is_still_read() {
        let chain = json(
            r#"{"lectureChapters":{"provider":"claude","model":"claude-opus-5","reasoningEffort":"max"}}"#,
        );
        assert_eq!(models(&chain), ["claude-opus-5"]);
        assert_eq!(chain[0].effort(), Some("max"));
    }

    #[test]
    fn a_model_that_takes_no_level_keeps_its_null() {
        let chain = json(
            r#"{"lectureChapters":[{"provider":"codex","model":"gpt-5.6-luna","reasoningEffort":null}]}"#,
        );
        assert_eq!(chain[0].effort(), None);
    }

    #[test]
    fn an_arrays_order_is_the_order_tried() {
        let chain = json(
            r#"{"lectureChapters":[
                {"provider":"opencode","model":"openrouter/b","reasoningEffort":null},
                {"provider":"claude","model":"claude-opus-5","reasoningEffort":"high"},
                {"provider":"codex","model":"gpt-5.6-luna","reasoningEffort":"low"}]}"#,
        );
        assert_eq!(
            models(&chain),
            ["openrouter/b", "claude-opus-5", "gpt-5.6-luna"]
        );
        assert_eq!(chain[0].provider, Provider::Opencode);
    }

    #[test]
    fn an_empty_array_switches_the_job_off() {
        assert!(json(r#"{"lectureChapters":[]}"#).is_empty());
    }

    #[test]
    fn a_bad_entry_in_an_array_drops_just_that_entry() {
        let chain = json(
            r#"{"lectureChapters":[
                {"provider":"claude","model":"claude-opus-5","reasoningEffort":null},
                {"provider":"gemini","model":"x","reasoningEffort":null},
                {"provider":"codex","model":"gpt-5.6-luna","reasoningEffort":"low"}]}"#,
        );
        assert_eq!(models(&chain), ["claude-opus-5", "gpt-5.6-luna"]);
    }

    #[test]
    fn an_array_of_only_bad_entries_is_off_not_the_default() {
        let chain = json(
            r#"{"lectureChapters":[{},"gpt-5.6-luna",null,
                {"provider":"codex","model":"  ","reasoningEffort":null},
                {"provider":"gemini","model":"x","reasoningEffort":null}]}"#,
        );
        assert!(chain.is_empty(), "{chain:?}");
    }

    #[test]
    fn another_jobs_row_is_not_this_jobs() {
        let body = r#"{"threadNaming":[{"provider":"codex","model":"gpt-5.6-luna","reasoningEffort":"low"}]}"#;
        assert!(is_default(&json(body), Job::LectureChapters));
        assert_eq!(
            models(&from_json(body, Job::ThreadNaming)),
            ["gpt-5.6-luna"]
        );
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
            r#"{"lectureChapters":null}"#,
            r#"{"lectureChapters":3}"#,
        ] {
            assert!(
                is_default(&json(body), Job::LectureChapters),
                "{body} should resolve to the default"
            );
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
        let body = r#"{"documentSuggestions":[{"provider":"opencode","model":"openrouter/x","reasoningEffort":null}]}"#;
        let chain = from_json(body, Job::DocumentSuggestions);
        assert_eq!(
            (chain[0].provider, chain[0].model.as_str()),
            (Provider::Opencode, "openrouter/x")
        );
        assert!(is_default(
            &from_json(body, Job::ThreadNaming),
            Job::ThreadNaming
        ));
    }

    #[test]
    fn try_each_goes_in_order_and_stops_at_the_first_success() {
        let chain = [sel("a"), sel("b"), sel("c")];
        let mut tried = Vec::new();
        let got = try_each(Job::LectureEnd, &chain, |s| {
            tried.push(s.model.clone());
            if s.model == "b" {
                Ok(s.model.clone())
            } else {
                Err("no".into())
            }
        });
        assert_eq!(got.as_deref(), Ok("b"));
        assert_eq!(tried, ["a", "b"]);
    }

    #[test]
    fn try_each_on_an_empty_chain_says_the_job_has_no_model() {
        let got: Result<(), String> =
            try_each(Job::LectureChapters, &[], |_| panic!("nothing to try"));
        assert_eq!(
            got.unwrap_err(),
            "No model is set for Lecture chapters — choose one in Settings → Jobs."
        );
    }

    #[test]
    fn try_each_names_every_failed_model() {
        let got: Result<(), String> = try_each(Job::ThreadNaming, &[sel("a"), sel("b")], |s| {
            Err(format!("{} broke", s.model))
        });
        let e = got.unwrap_err();
        assert!(e.contains("a: a broke") && e.contains("b: b broke"), "{e}");
        assert!(e.find("a: a broke") < e.find("b: b broke"));
    }

    #[test]
    fn try_each_with_one_model_returns_its_error_unchanged() {
        let got: Result<(), String> = try_each(Job::ThreadNaming, &[sel("a")], |_| {
            Err("the CLI is missing".into())
        });
        assert_eq!(got.unwrap_err(), "the CLI is missing");
    }
}
