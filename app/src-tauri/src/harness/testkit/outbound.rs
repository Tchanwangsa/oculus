use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::Value;

pub(in crate::harness) type Wire = Arc<dyn Fn(Value) -> Result<(), String> + Send + Sync>;
pub(in crate::harness) type Http = Arc<dyn Fn(&str, &str, Value) -> Value + Send + Sync>;

#[derive(Clone, Deserialize)]
pub(super) struct Expected {
    pub frame: Value,
    pub response: Value,
    #[serde(default)]
    pub error: Option<String>,
    /// Where the recording sent its request, any prompt is accepted, and
    /// kept for the test to read ([`Outbound::prompts`]).
    #[serde(default)]
    pub any_prompt: bool,
}

/// What every recording sent as its request.
pub(super) const REQUEST: &str = "Replay request";

/// Whether `actual` is `expected`, with any string where the recording
/// sent its request; that string is the prompt.
fn same_but_prompt(actual: &Value, expected: &Value, prompt: &mut Option<String>) -> bool {
    match (actual, expected) {
        (Value::String(said), Value::String(recorded)) if recorded == REQUEST => {
            *prompt = Some(said.clone());
            true
        }
        (Value::Array(a), Value::Array(e)) => {
            a.len() == e.len() && a.iter().zip(e).all(|(a, e)| same_but_prompt(a, e, prompt))
        }
        (Value::Object(a), Value::Object(e)) => {
            a.len() == e.len()
                && a.iter()
                    .all(|(key, a)| e.get(key).is_some_and(|e| same_but_prompt(a, e, prompt)))
        }
        (a, e) => a == e,
    }
}

#[derive(Clone)]
pub(in crate::harness) struct Outbound {
    state: Arc<Mutex<State>>,
    scratch: String,
}

struct State {
    expected: VecDeque<Expected>,
    captured: Vec<Value>,
    prompts: Vec<String>,
}

impl Outbound {
    pub(super) fn new(expected: Vec<Expected>, scratch: &std::path::Path) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                expected: expected.into(),
                captured: Vec::new(),
                prompts: Vec::new(),
            })),
            scratch: scratch.display().to_string(),
        }
    }

    pub(in crate::harness) fn exchange(&self, frame: Value) -> Value {
        self.try_exchange(frame).expect("recorded outbound failure")
    }

    pub(in crate::harness) fn try_exchange(&self, frame: Value) -> Result<Value, String> {
        let frame = normalize(frame, &self.scratch);
        let mut state = self.state.lock().unwrap();
        let expected = state
            .expected
            .pop_front()
            .unwrap_or_else(|| panic!("unrecorded outbound operation: {frame}"));
        if expected.any_prompt {
            let mut prompt = None;
            assert!(
                same_but_prompt(&frame, &expected.frame, &mut prompt),
                "outbound frame differs from manifest beyond its prompt: {frame}"
            );
            state.prompts.extend(prompt);
        } else {
            assert_eq!(
                frame, expected.frame,
                "outbound frame differs from manifest"
            );
        }
        state.captured.push(frame);
        match expected.error {
            Some(error) => Err(error),
            None => Ok(expected.response),
        }
    }

    /// More operations the replay is expected to make, after those already
    /// listed.
    pub(super) fn expect(&self, more: impl IntoIterator<Item = Expected>) {
        self.state.lock().unwrap().expected.extend(more);
    }

    /// The prompts sent where an expected operation accepted any.
    pub(super) fn prompts(&self) -> Vec<String> {
        self.state.lock().unwrap().prompts.clone()
    }

    /// The model a recorded Codex session starts under. Its replay omits
    /// `thread/start`, so the first turn supplies that spawn configuration.
    pub(in crate::harness) fn codex_model(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap()
            .expected
            .front()?
            .frame
            .pointer("/params/model")?
            .as_str()
            .map(String::from)
    }

    pub(super) fn wire(&self) -> Wire {
        let this = self.clone();
        Arc::new(move |frame| this.try_exchange(frame).map(|_| ()))
    }

    pub(super) fn finish(&self, all_used: bool) -> Vec<Value> {
        let state = self.state.lock().unwrap();
        assert!(
            !all_used || state.expected.is_empty(),
            "recorded outbound operation never happened"
        );
        state.captured.clone()
    }
}

fn normalize(value: Value, scratch: &str) -> Value {
    match value {
        Value::String(s) => Value::String(s.replace(scratch, "$SCRATCH")),
        Value::Array(a) => a.into_iter().map(|v| normalize(v, scratch)).collect(),
        Value::Object(o) => o
            .into_iter()
            .map(|(k, v)| (k, normalize(v, scratch)))
            .collect(),
        v => v,
    }
}

#[test]
#[should_panic(expected = "unrecorded outbound operation")]
fn unrecorded_operations_fail_closed() {
    Outbound::new(Vec::new(), std::path::Path::new("/scratch"))
        .exchange(serde_json::json!({"method":"hidden resend"}));
}
