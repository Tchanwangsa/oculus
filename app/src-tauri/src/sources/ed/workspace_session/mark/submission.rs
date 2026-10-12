//! Ed's submission, as `mark` answers it and `GET /challenges/submissions/<id>`
//! reads it back, shaped for the feedback panel: camelCase, a testcase's
//! `run.output` as `output`, and its `check_diff` checks' word diffs as
//! `diffs` (`diff`). An empty text is `null`.

use serde::Serialize;
use serde_json::Value;

use super::diff::{self, Run};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Submission {
    pub id: i64,
    pub status: String,
    pub created_at: String,
    pub marked_at: Option<String>,
    pub testcase_pass_count: Option<i64>,
    pub testcase_total_count: Option<i64>,
    pub feedback: Option<String>,
    pub result: Option<MarkResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MarkResult {
    pub ok: bool,
    pub passed: bool,
    pub feedback: Option<String>,
    pub build: Option<Build>,
    pub testcases: Vec<Testcase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Build {
    pub ok: bool,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Testcase {
    pub name: String,
    pub description: Option<String>,
    pub hidden: bool,
    pub private: bool,
    pub score: Option<f64>,
    pub max_score: Option<f64>,
    pub passed: bool,
    pub skip: bool,
    pub feedback: Option<String>,
    /// `run.output`.
    pub output: Option<String>,
    /// Every `check_diff` check's lines, in check order (`diff`).
    pub diffs: Vec<Vec<Run>>,
}

/// Shape one submission; only a missing `id` is refused.
pub fn normalise(raw: &Value) -> Result<Submission, String> {
    let id = raw["id"].as_i64().ok_or("Ed's submission has no id")?;
    Ok(Submission {
        id,
        status: raw["status"].as_str().unwrap_or_default().to_string(),
        created_at: raw["created_at"].as_str().unwrap_or_default().to_string(),
        marked_at: text(&raw["marked_at"]),
        testcase_pass_count: raw["testcase_pass_count"].as_i64(),
        testcase_total_count: raw["testcase_total_count"].as_i64(),
        feedback: text(&raw["feedback"]),
        result: raw["result"].as_object().map(|_| result(&raw["result"])),
    })
}

fn result(raw: &Value) -> MarkResult {
    MarkResult {
        ok: flag(&raw["ok"]),
        passed: flag(&raw["passed"]),
        feedback: text(&raw["feedback"]),
        build: raw["build"].as_object().map(|_| Build {
            ok: flag(&raw["build"]["ok"]),
            output: raw["build"]["output"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        }),
        testcases: raw["testcases"]
            .as_array()
            .into_iter()
            .flatten()
            .map(testcase)
            .collect(),
    }
}

fn testcase(raw: &Value) -> Testcase {
    let diffs = raw["checks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|check| is_diff(check))
        .flat_map(|check| diff::lines(&check["value"]["diff"]))
        .collect();
    Testcase {
        name: raw["name"].as_str().unwrap_or_default().to_string(),
        description: text(&raw["description"]),
        hidden: flag(&raw["hidden"]),
        private: flag(&raw["private"]),
        score: raw["score"].as_f64(),
        max_score: raw["max_score"].as_f64(),
        passed: raw["passed"].as_bool().unwrap_or_else(|| flag(&raw["ok"])),
        skip: flag(&raw["skip"]),
        feedback: text(&raw["feedback"]),
        output: text(&raw["run"]["output"]),
        diffs,
    }
}

/// A `check_diff` check, by its type when it names one, else by carrying a diff.
fn is_diff(check: &Value) -> bool {
    match check["type"].as_str().or_else(|| check["kind"].as_str()) {
        Some(kind) => kind == "check_diff",
        None => check["value"]["diff"].is_array(),
    }
}

fn flag(v: &Value) -> bool {
    v.as_bool().unwrap_or(false)
}

fn text(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}
