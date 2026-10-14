//! Which agent runs which job, on what model, at what reasoning level.
//!
//! Nothing is defaulted out of sight: a non-chat job names an ordered list of
//! models (provider, model, level), set in Settings → Jobs with the composer's
//! `ModelPicker`. The first runs; when its run fails for any reason the next
//! one does ([`try_each`]). An empty list switches the job off, and never
//! falls back to the default, which can be an expensive model. The two Studio
//! jobs read their list differently: it holds one model per agent, and a chat
//! agent's subagents take the first entry for its own agent ([`for_agent`]).
//! Stored as one
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
    /// Turns dictated speech into a readable message.
    DictationCleanup,
    /// A chat agent's subagent that reads lecture material for a video
    /// (`harness::studio_agents`).
    StudioResearch,
    /// A chat agent's subagent that builds one scene of a video.
    StudioScene,
}

impl Job {
    /// The key inside the stored object (camelCase, like the webview's JSON).
    pub fn key(self) -> &'static str {
        match self {
            Job::LectureChapters => "lectureChapters",
            Job::LectureEnd => "lectureEnd",
            Job::ThreadNaming => "threadNaming",
            Job::DocumentSuggestions => "documentSuggestions",
            Job::DictationCleanup => "dictationCleanup",
            Job::StudioResearch => "studioResearch",
            Job::StudioScene => "studioScene",
        }
    }

    /// Its name in Settings → Jobs (`JOBS` in `jobModels.ts`).
    pub fn label(self) -> &'static str {
        match self {
            Job::LectureChapters => "Lecture chapters",
            Job::LectureEnd => "Lecture end",
            Job::ThreadNaming => "Chat thread names",
            Job::DocumentSuggestions => "Document suggestions",
            Job::DictationCleanup => "Dictation cleanup",
            Job::StudioResearch => "Studio research",
            Job::StudioScene => "Studio scenes",
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

/// What a job runs on until someone says otherwise. The Studio jobs name one
/// model per agent whose subagents take a model (`studio_agents::takes_model`).
/// Native Claude Code is disabled on Windows (docs/harness.md), so there the
/// small jobs run on Codex and the Studio jobs drop their Claude entry.
pub fn default_chain(job: Job) -> Vec<JobSelection> {
    let one = |provider, model: &str, effort: Option<&str>| JobSelection {
        provider,
        model: model.into(),
        reasoning_effort: effort.map(Into::into),
    };
    if cfg!(windows)
        && matches!(
            job,
            Job::LectureEnd | Job::ThreadNaming | Job::DocumentSuggestions | Job::DictationCleanup
        )
    {
        return vec![one(Provider::Codex, "gpt-5.6-luna", Some("medium"))];
    }
    let chain = match job {
        Job::LectureChapters => vec![one(Provider::Codex, "gpt-5.6-luna", Some("xhigh"))],
        // Provisional, until the lecture-end eval picks the default.
        Job::LectureEnd => vec![one(Provider::Claude, "claude-haiku-4-5-20251001", None)],
        // Haiku declares no effort levels, so none is asked for.
        Job::ThreadNaming => vec![one(Provider::Claude, "claude-haiku-4-5-20251001", None)],
        // Sonnet 5.5: `sonnet` in the CLI's catalogue, stored resolved.
        Job::DocumentSuggestions => {
            vec![one(Provider::Claude, "claude-sonnet-5-5", Some("medium"))]
        }
        Job::DictationCleanup => vec![one(Provider::Claude, "claude-haiku-5-5", None)],
        // Small and fast: notes from a slice of a lecture.
        Job::StudioResearch => vec![
            one(Provider::Claude, "claude-haiku-5-5", None),
            one(Provider::Codex, "gpt-6-luna", Some("medium")),
        ],
        // Strong: a scene's animation is the hard part of a video.
        Job::StudioScene => vec![
            one(Provider::Claude, "claude-opus-5-5", Some("high")),
            one(Provider::Codex, "gpt-6.1-sol", Some("high")),
        ],
    };
    if cfg!(windows) {
        chain
            .into_iter()
            .filter(|s| s.provider != Provider::Claude)
            .collect()
    } else {
        chain
    }
}

/// The entry an agent's subagents run on: the first for that agent, or none,
/// which switches the job off for that agent.
pub fn for_agent(chain: &[JobSelection], provider: Provider) -> Option<&JobSelection> {
    chain.iter().find(|s| s.provider == provider)
}

/// [`chain`] for a caller with no pool, such as a spawn: read through
/// `store::setting_blocking`. No database is the default, like no row.
pub fn chain_blocking(job: Job) -> Vec<JobSelection> {
    match crate::db::store::setting_blocking(SETTINGS_KEY) {
        Some(raw) => from_json(&raw, job),
        None => default_chain(job),
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
        None => default_chain(job),
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
            one(item).map_or_else(|| default_chain(job), |s| vec![s])
        }
        _ => default_chain(job),
    }
}

/// One stored selection, if it parses and names a model.
fn one(value: serde_json::Value) -> Option<JobSelection> {
    let picked: JobSelection = serde_json::from_value(value).ok()?;
    // Native Claude Code is disabled on Windows (docs/harness.md), so a
    // stored Claude pick reads as unreadable there.
    #[cfg(windows)]
    if picked.provider == Provider::Claude {
        return None;
    }
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
                    elog!(
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
        models(chain) == models(&default_chain(job))
    }

    fn first(job: Job) -> JobSelection {
        default_chain(job).remove(0)
    }

    fn sel(model: &str) -> JobSelection {
        JobSelection {
            provider: Provider::Claude,
            model: model.into(),
            reasoning_effort: None,
        }
    }

    #[test]
    #[cfg(not(windows))]
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
    #[cfg(not(windows))]
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
    #[cfg(not(windows))]
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
    #[cfg(not(windows))]
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
        let c = first(Job::LectureChapters);
        assert_eq!(
            (c.provider, c.model.as_str(), c.effort()),
            (Provider::Codex, "gpt-5.6-luna", Some("xhigh"))
        );
        #[cfg(windows)]
        for job in [Job::ThreadNaming, Job::LectureEnd, Job::DocumentSuggestions] {
            let picked = first(job);
            assert_eq!(
                (picked.provider, picked.model.as_str(), picked.effort()),
                (Provider::Codex, "gpt-5.6-luna", Some("medium"))
            );
        }
        #[cfg(not(windows))]
        {
            let n = first(Job::ThreadNaming);
            assert_eq!(n.provider, Provider::Claude);
            assert_eq!(
                (n.model.as_str(), n.effort()),
                ("claude-haiku-4-5-20251001", None)
            );
            let e = first(Job::LectureEnd);
            assert_eq!(
                (e.provider, e.model.as_str(), e.effort()),
                (Provider::Claude, "claude-haiku-4-5-20251001", None)
            );
            assert_eq!(Job::LectureEnd.key(), "lectureEnd");
            let d = first(Job::DocumentSuggestions);
            assert_eq!(
                (d.provider, d.model.as_str(), d.effort()),
                (Provider::Claude, "claude-sonnet-5-5", Some("medium"))
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn native_windows_jobs_do_not_resolve_a_stored_claude_selection() {
        for job in [
            Job::LectureChapters,
            Job::LectureEnd,
            Job::ThreadNaming,
            Job::DocumentSuggestions,
        ] {
            let raw = format!(
                r#"{{"{}":{{"provider":"claude","model":"claude-opus-5","reasoningEffort":"max"}}}}"#,
                job.key()
            );
            assert!(is_default(&from_json(&raw, job), job));
            let list = format!(
                r#"{{"{}":[{{"provider":"claude","model":"claude-opus-5","reasoningEffort":"max"}}]}}"#,
                job.key()
            );
            assert!(from_json(&list, job).is_empty());
        }
    }

    #[test]
    #[cfg(not(windows))]
    fn the_studio_defaults_name_one_model_per_agent_that_takes_one() {
        for job in [Job::StudioResearch, Job::StudioScene] {
            let chain = default_chain(job);
            let agents: Vec<Provider> = chain.iter().map(|s| s.provider).collect();
            assert_eq!(agents, [Provider::Claude, Provider::Codex], "{job:?}");
        }
        let r = default_chain(Job::StudioResearch);
        assert_eq!(
            (r[0].model.as_str(), r[0].effort()),
            ("claude-haiku-5-5", None)
        );
        let s = default_chain(Job::StudioScene);
        assert_eq!(
            (s[0].model.as_str(), s[0].effort()),
            ("claude-opus-5-5", Some("high"))
        );
        assert_eq!(Job::StudioScene.key(), "studioScene");
    }

    #[test]
    fn an_agent_takes_its_own_first_entry_or_none() {
        let chain = from_json(
            r#"{"studioScene":[
                {"provider":"codex","model":"gpt-6.1-sol","reasoningEffort":"high"},
                {"provider":"opencode","model":"openrouter/a","reasoningEffort":null},
                {"provider":"opencode","model":"openrouter/b","reasoningEffort":null}]}"#,
            Job::StudioScene,
        );
        assert_eq!(
            for_agent(&chain, Provider::Opencode).map(|s| s.model.as_str()),
            Some("openrouter/a")
        );
        assert_eq!(
            for_agent(&chain, Provider::Codex).map(|s| s.model.as_str()),
            Some("gpt-6.1-sol")
        );
        assert!(for_agent(&chain, Provider::Claude).is_none());
        assert!(from_json(r#"{"studioScene":[]}"#, Job::StudioScene).is_empty());
    }

    #[test]
    fn dictation_cleanup_uses_its_own_chain_and_respects_off() {
        assert_eq!(Job::DictationCleanup.key(), "dictationCleanup");
        assert_eq!(Job::DictationCleanup.label(), "Dictation cleanup");
        assert!(from_json(r#"{"dictationCleanup":[]}"#, Job::DictationCleanup).is_empty());
        let chain = from_json(
            r#"{"dictationCleanup":[{"provider":"opencode","model":"small","reasoningEffort":null}]}"#,
            Job::DictationCleanup,
        );
        assert_eq!(chain[0].model, "small");
        assert_eq!(chain[0].provider, Provider::Opencode);
        assert!(is_default(
            &from_json(r#"{"dictationCleanup":[]}"#, Job::ThreadNaming),
            Job::ThreadNaming
        ));
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
