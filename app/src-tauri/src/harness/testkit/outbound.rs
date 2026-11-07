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
}

#[derive(Clone)]
pub(in crate::harness) struct Outbound {
    state: Arc<Mutex<State>>,
    scratch: String,
}

struct State {
    expected: VecDeque<Expected>,
    captured: Vec<Value>,
}

impl Outbound {
    pub(super) fn new(expected: Vec<Expected>, scratch: &std::path::Path) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                expected: expected.into(),
                captured: Vec::new(),
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
        assert_eq!(
            frame, expected.frame,
            "outbound frame differs from manifest"
        );
        state.captured.push(frame);
        match expected.error {
            Some(error) => Err(error),
            None => Ok(expected.response),
        }
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
