//! Confirmed long-running cards reuse the app's job paths and persist completion.

use serde_json::Value;

fn text<'a>(state: &'a Value, key: &str) -> Result<&'a str, String> {
    state
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("{key} is required"))
}
fn strings(state: &Value, key: &str) -> Result<Vec<String>, String> {
    let items = state
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{key} must be a list"))?;
    if items.is_empty() {
        return Err(format!("{key} must not be empty"));
    }
    items
        .iter()
        .map(|v| {
            v.as_str()
                .filter(|s| !s.trim().is_empty())
                .map(str::to_owned)
                .ok_or_else(|| format!("{key} needs nonempty strings"))
        })
        .collect()
}
fn validate(kind: &str, state: &Value) -> Result<(), String> {
    match kind {
        "Sync" => {
            strings(state, "subjects")?;
            if state
                .get("lectures")
                .is_some_and(|v| !v.is_null() && !v.is_boolean())
            {
                return Err("lectures must be boolean".into());
            }
        }
        "Reparse" => {
            strings(state, "files")?;
        }
        "Transcribe" => {
            text(state, "file")?;
        }
        "LectureChapters" | "LectureEnd" => {
            text(state, "lecture")?;
        }
        "Download" => {
            let item = text(state, "item")?;
            if item != "aligner"
                && item != "voices"
                && crate::studio::tts::kokoro::voices::find(item).is_none()
            {
                return Err(format!("unknown download {item}"));
            }
        }
        _ => return Err(format!("{kind} is not a job card")),
    }
    Ok(())
}

pub mod lifecycle;
pub mod metadata;
mod runner;
pub(super) use lifecycle::reconcile;
