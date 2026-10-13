//! Terminals on the workspace server, over the session socket. A terminal is
//! opened `persist`, so it outlives the socket: `init` lists the ones the
//! server still runs, and opening one of those ids again with
//! `start_if_stopped` attaches to it rather than starting another, which is
//! how Ed's web app gets its terminal tabs back after a reload. The pane gets
//! a listed one first (`resumed`); each connect re-attaches every terminal
//! the pane holds, so its output keeps flowing across a dropped socket.

use std::collections::BTreeMap;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use serde_json::{json, Value};

use super::context::Context;
use super::events::{Body, TermState, TerminalOpened};
use super::frames::{Incoming, Init};
use super::session::Session;
use super::socket::Out;

/// A terminal's size in character cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtySize {
    pub cols: u16,
    pub rows: u16,
}

const PREFIX: &str = "term-";

#[derive(Debug, Default)]
pub struct Terminals {
    /// Persisting terminals the server runs: from `init`, then `program_init`.
    listed: Vec<String>,
    /// The ones handed to the pane, with their last size.
    held: BTreeMap<String, PtySize>,
}

impl Terminals {
    /// A connect's `init` arrived. Returns the frames that re-attach the
    /// terminals the pane holds, and the held ones the server no longer has.
    pub fn connected(&mut self, init: &Init) -> (Vec<String>, Vec<String>) {
        self.listed = listed_in(init);
        let gone: Vec<String> = self
            .held
            .keys()
            .filter(|id| !self.listed.contains(id))
            .cloned()
            .collect();
        for id in &gone {
            self.held.remove(id);
        }
        let attach = self
            .held
            .iter()
            .map(|(id, size)| open_frame(id, *size))
            .collect();
        (attach, gone)
    }

    /// A terminal for the pane: a listed one nobody holds, else a new id.
    pub fn open(&mut self, size: PtySize) -> TerminalOpened {
        let listed = self
            .listed
            .iter()
            .find(|id| !self.held.contains_key(*id))
            .cloned();
        let resumed = listed.is_some();
        let id = listed.unwrap_or_else(|| self.new_id());
        self.held.insert(id.clone(), size);
        TerminalOpened { id, resumed }
    }

    fn new_id(&self) -> String {
        let mut n = crate::runtime::clock::now_nanos() as u32;
        loop {
            let id = format!("{PREFIX}{n}");
            if !self.listed.contains(&id) && !self.held.contains_key(&id) {
                return id;
            }
            n = n.wrapping_add(1);
        }
    }

    pub fn holds(&self, id: &str) -> bool {
        self.held.contains_key(id)
    }

    pub fn resized(&mut self, id: &str, size: PtySize) {
        if let Some(held) = self.held.get_mut(id) {
            *held = size;
        }
    }

    /// The terminal ended or was killed: it is neither listed nor held.
    pub fn ended(&mut self, id: &str) {
        self.listed.retain(|l| l != id);
        self.held.remove(id);
    }

    /// Forget every terminal (a reset ends them all on Ed). Returns the
    /// ones to kill, the held ones first, and the held ones alone.
    pub fn end_all(&mut self) -> (Vec<String>, Vec<String>) {
        let held: Vec<String> = self.held.keys().cloned().collect();
        let mut every = held.clone();
        every.extend(self.listed.iter().filter(|id| !held.contains(id)).cloned());
        self.held.clear();
        self.listed.clear();
        (every, held)
    }

    fn started(&mut self, id: &str, persist: bool) {
        if persist && !self.listed.iter().any(|l| l == id) {
            self.listed.push(id.to_string());
        }
    }
}

/// The persisting terminal ids `init` names, in its order. `programs` and
/// `program_state` are read alike: an object keyed by id, or a list of
/// entries carrying `id` (or bare ids). One marked `persist: false` is left out.
pub fn listed_in(init: &Init) -> Vec<String> {
    let mut ids = Vec::new();
    for source in [&init.programs, &init.program_state] {
        let entries: Vec<(String, &Value)> = match source {
            Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v)).collect(),
            Value::Array(list) => list
                .iter()
                .filter_map(|v| match v {
                    Value::String(id) => Some((id.clone(), v)),
                    _ => Some((v["id"].as_str()?.to_string(), v)),
                })
                .collect(),
            _ => Vec::new(),
        };
        for (id, entry) in entries {
            let kept = entry["persist"].as_bool() != Some(false);
            if id.starts_with(PREFIX) && kept && !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids
}

/// What a `program_init`/`program_update` state says. Ed's words for a
/// finished program are taken as exited; anything else is still running.
pub fn state_of(state: &Value) -> TermState {
    let word = match state {
        Value::Object(o) => o.get("state").and_then(Value::as_str),
        other => other.as_str(),
    };
    match word {
        Some("exited" | "stopped" | "killed" | "terminated" | "closed" | "dead" | "finished") => {
            TermState::Exited
        }
        _ => TermState::Running,
    }
}

/// A terminal frame from the server: output and state go to the pane.
/// Programs that are not terminals are not the pane's.
pub fn on_frame(core: &Context, frame: Incoming) {
    let (id, body) = match frame {
        Incoming::ProgramData(p) => (
            p.id.clone(),
            Body::Term {
                id: p.id,
                data: p.data,
            },
        ),
        Incoming::ProgramInit(p) => {
            core.shared().terminals.started(&p.id, p.persist);
            let state = state_of(&p.state);
            (p.id.clone(), Body::TermState { id: p.id, state })
        }
        Incoming::ProgramUpdate(p) => {
            let state = state_of(&p.state);
            if state == TermState::Exited {
                core.shared().terminals.ended(&p.id);
            }
            (p.id.clone(), Body::TermState { id: p.id, state })
        }
        Incoming::ProgramClose(p) => {
            core.shared().terminals.ended(&p.id);
            let state = TermState::Exited;
            (p.id.clone(), Body::TermState { id: p.id, state })
        }
        _ => return,
    };
    if id.starts_with(PREFIX) {
        core.emit(body);
    }
}

/// Start a login shell as terminal `id`, or attach to it when it runs.
pub fn open_frame(id: &str, size: PtySize) -> String {
    json!({
        "type": "program_open",
        "data": {
            "id": id,
            "command": "bash --login",
            "pty_size": { "cols": size.cols, "rows": size.rows },
            "start_if_stopped": true,
            "persist": true,
        },
    })
    .to_string()
}

/// Keystrokes for terminal `id`.
pub fn input_frame(id: &str, text: &str) -> String {
    json!({
        "type": "program_data",
        "data": { "id": id, "data": BASE64.encode(text.as_bytes()) },
    })
    .to_string()
}

pub fn resize_frame(id: &str, size: PtySize) -> String {
    json!({
        "type": "program_set_pty",
        "data": { "id": id, "pty_size": { "cols": size.cols, "rows": size.rows } },
    })
    .to_string()
}

pub fn kill_frame(id: &str) -> String {
    json!({ "type": "program_signal", "data": { "id": id, "signal": "kill" } }).to_string()
}

const OFFLINE: &str = "The workspace is offline: the terminal runs on Ed's server";

impl Session {
    /// A terminal for the pane: a persisting one re-attached, else a new
    /// login shell.
    pub fn terminal_open(&self, size: PtySize) -> Result<TerminalOpened, String> {
        self.core.online().ok_or(OFFLINE)?;
        let opened = self.core.shared().terminals.open(size);
        self.send_live(open_frame(&opened.id, size))?;
        Ok(opened)
    }

    pub fn terminal_input(&self, id: &str, text: &str) -> Result<(), String> {
        self.held(id)?;
        self.send_live(input_frame(id, text))
    }

    pub fn terminal_resize(&self, id: &str, size: PtySize) -> Result<(), String> {
        self.held(id)?;
        self.core.shared().terminals.resized(id, size);
        self.send_live(resize_frame(id, size))
    }

    pub fn terminal_kill(&self, id: &str) -> Result<(), String> {
        if !self.core.shared().terminals.holds(id) {
            return Ok(());
        }
        self.send_live(kill_frame(id))?;
        self.core.shared().terminals.ended(id);
        Ok(())
    }

    fn held(&self, id: &str) -> Result<(), String> {
        if self.core.shared().terminals.holds(id) {
            Ok(())
        } else {
            Err(format!("Terminal {id} has ended"))
        }
    }

    /// Queue `frame` for the socket while online; an error while not.
    fn send_live(&self, frame: String) -> Result<(), String> {
        self.core.online().ok_or(OFFLINE)?;
        self.out
            .send(Out::Frame(frame))
            .map_err(|_| "the workspace session has ended".to_string())
    }
}
