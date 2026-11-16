//! The steering probe recordings through the production command, effect
//! and bridge write paths. Each manifest's `settings.steers` says where in
//! the recording the steer was written (`gate`) and what it said; the steer
//! goes in there, through the real `Send` command, and the recorded echo
//! frames get the client id Oculus wrote under in place of the probe's.

use serde_json::Value;

use crate::harness::core::{Command, Kind};
use crate::harness::manager::policy::Mode;
use crate::harness::store::steer::Written;
use crate::harness::SendOptions;

use super::super::{replay::Replay, rig::Rig, scenario::Scenario};

pub(in super::super) const NAMES: [&str; 5] = [
    "codex-steer-tool",
    "codex-steer-text-finish",
    "claude-steer-tool",
    "claude-steer-text",
    "claude-steer-finish",
];

/// Whether the steer `steer` is written just before `frames[at]` is read.
pub(super) fn gates(steer: &Value, frames: &[Value], at: usize) -> bool {
    let frame = &frames[at];
    let before = at.checked_sub(1).map(|i| &frames[i]);
    if let Some(after) = steer.get("after") {
        return before.is_some_and(|b| {
            b["method"] == after["method"] && b["params"]["item"]["type"] == after["type"]
        });
    }
    if let Some(count) = steer.get("after_delta").and_then(Value::as_u64) {
        let deltas = frames[..at]
            .iter()
            .filter(|f| f["method"] == "item/agentMessage/delta")
            .count() as u64;
        return deltas == count && before.is_some_and(|b| b["method"] == "item/agentMessage/delta");
    }
    if steer["gate"] == "terminal" {
        return frame["method"] == "turn/completed";
    }
    if let Some(state) = steer.get("before_lifecycle") {
        return frame["type"] == "command_lifecycle" && frame["state"] == *state;
    }
    if steer.get("before_result").is_some() {
        return frame["type"] == "result";
    }
    false
}

/// A student's message under `mode`, as `harness_send` makes it.
pub(super) fn say(
    rig: &mut Rig,
    id: &str,
    text: &str,
    mode: Mode,
) -> crate::harness::core::Outcome {
    let thread = rig.thread;
    rig.command(Command {
        id: id.into(),
        thread_id: Some(thread),
        kind: Kind::Send {
            provider: rig.provider,
            text: text.into(),
            options: SendOptions::default(),
            mode,
        },
    })
    .unwrap()
}

/// The id a steer was written under, read from the provider write Oculus
/// made: a steer that missed its turn has lost its row by now.
pub(super) fn client(rig: &Rig) -> Option<String> {
    let written = rig.writes();
    let last = written.last()?;
    last.pointer("/params/clientUserMessageId")
        .or_else(|| last.get("uuid"))
        .and_then(Value::as_str)
        .map(String::from)
}

/// Put the id Oculus wrote a steer under where the recording has the
/// probe's: Codex's `userMessage` echo, Claude's lifecycle frames.
fn echo(frame: &mut Value, ids: &[(String, String)]) {
    for (recorded, ours) in ids {
        for pointer in ["/params/item/clientId", "/command_uuid"] {
            if let Some(value) = frame.pointer_mut(pointer) {
                if value == recorded {
                    *value = Value::String(ours.clone());
                }
            }
        }
    }
}

/// Replay `name`, writing each steer at its gate. `after_terminal` runs
/// once the turn's terminal is read, for a check that acts then.
pub(super) fn drive(
    name: &str,
    mut after_terminal: impl FnMut(&mut Rig),
) -> (Replay, Vec<Written>) {
    let scenario = Scenario::recorded(name);
    let frames = scenario.frames.clone();
    let steers = scenario.fixture.settings["steers"]
        .as_array()
        .unwrap()
        .clone();
    let mut rig = Rig::start(scenario);
    rig.open();
    let (mut next, mut written, mut ids) = (0, Vec::new(), Vec::new());
    let mut ended = false;
    for at in 0..frames.len() {
        while next < steers.len() && gates(&steers[next], &frames, at) {
            let steer = &steers[next];
            let text = steer["text"].as_str().unwrap();
            let outcome = say(&mut rig, &format!("steer-{next}"), text, Mode::Auto);
            let w = outcome.steered.expect("the message steers");
            rig.drain();
            let recorded = steer["client"].as_str().unwrap().to_string();
            ids.extend(client(&rig).map(|ours| (recorded, ours)));
            written.push(w);
            next += 1;
        }
        let mut frame = frames[at].clone();
        echo(&mut frame, &ids);
        rig.feed(&frame);
        rig.drain();
        let terminal =
            frame["method"] == "turn/completed" || (frame["type"] == "result" && !rig.busy());
        if terminal && !ended {
            ended = true;
            after_terminal(&mut rig);
        }
    }
    assert_eq!(next, steers.len(), "every steer was written: {name}");
    rig.close();
    rig.drain();
    (rig.finish(), written)
}

/// The registry's runner: every recording replays, with the one extra
/// message a Codex manifest expects after its turn.
pub(in super::super) fn recorded(name: &str) -> Replay {
    drive(name, |rig| {
        if name == "codex-steer-tool" {
            say(rig, "next", "Replay request", Mode::Auto);
            rig.drain();
        }
    })
    .0
}
