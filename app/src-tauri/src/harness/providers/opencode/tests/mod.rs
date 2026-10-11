//! Replays of recorded streams, and the live end-to-end run.

use serde_json::Value;

use crate::harness::event::HarnessEvent;

use super::events::{session_of, translate, translate_server};
use super::session::SessionState;

mod live;
mod recorded;

/// Routes like [`OpencodeServer::dispatch`]: session-less events first,
/// then the first session the stream names is ours and any other is not.
pub(super) fn replay(raw: &str) -> Vec<HarnessEvent> {
    let mut st = SessionState::default();
    let mut ours: Option<String> = None;
    let mut events = Vec::new();
    for line in raw.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(ty) = v["type"].as_str() else {
            continue;
        };
        match session_of(&v) {
            None => events.extend(translate_server(ty, &v["properties"])),
            Some(id) => {
                if ours.get_or_insert_with(|| id.to_string()) == id {
                    events.extend(translate(ty, &v["properties"], &mut st));
                }
            }
        }
    }
    events
}

pub(super) fn finishes(events: &[HarnessEvent]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::TurnFinished { status } => Some(status.as_str()),
            _ => None,
        })
        .collect()
}

pub(super) fn errors(events: &[HarnessEvent]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::Error { message, .. } => Some(message.as_str()),
            _ => None,
        })
        .collect()
}

pub(super) fn deltas(events: &[HarnessEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::AssistantDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

pub(super) fn messages(events: &[HarnessEvent]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::AssistantMessage { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

pub(super) fn count(events: &[HarnessEvent], f: impl Fn(&HarnessEvent) -> bool) -> usize {
    events.iter().filter(|e| f(e)).count()
}
