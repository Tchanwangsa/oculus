//! The run socket's frames (`wss://<host>/run?ticket=`): JSON `{type, data}`
//! both ways, and what an exit status means to the pane.

use serde_json::{json, Value};

/// A frame from the run server, of the kinds a run uses.
#[derive(Debug, Clone, PartialEq)]
pub enum RunIncoming {
    /// The build step finished; `output` is its text.
    Built {
        ok: bool,
        output: String,
    },
    /// Program output, base64 (`stdout` and `stderr` both: a PTY merges them).
    Output(String),
    Exit(Exit),
    /// `dump_files`: what the program left, as Ed sent it (`dump::decode`).
    /// It follows `run_exit`, and the run is over only once it is in.
    Files(Value),
}

/// How a run ended, as the `runExit` event carries it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Exit {
    pub code: Option<i64>,
    pub signal: Option<String>,
    pub message: Option<String>,
}

impl Exit {
    pub fn because(message: &str) -> Self {
        Exit {
            message: Some(message.to_string()),
            ..Exit::default()
        }
    }
}

/// The close code the run server sends when it has no capacity.
pub const NO_CAPACITY: u16 = 1013;
pub const NO_RESOURCES: &str = "No resources available";
pub const STOPPED: &str = "Stopped";
pub const BUILD_FAILED: &str = "The build failed";
pub const CUT_OFF: &str = "The run ended before the program exited";

/// One text frame; `None` for kinds a run does not use (`status_created`,
/// stdin echoes) and shapes this does not know.
pub fn parse(text: &str) -> Option<RunIncoming> {
    let frame: Value = serde_json::from_str(text).ok()?;
    let data = &frame["data"];
    Some(match frame["type"].as_str()? {
        "status_built" => RunIncoming::Built {
            ok: data["ok"].as_bool().unwrap_or(false),
            output: data["output"].as_str().unwrap_or("").to_string(),
        },
        "run_frame" => match data["type"].as_str()? {
            "stdout" | "stderr" => RunIncoming::Output(data["data"].as_str()?.to_string()),
            _ => return None,
        },
        "run_exit" => RunIncoming::Exit(exit_of(&data["exit_status"])),
        "dump_files" => RunIncoming::Files(data.clone()),
        _ => return None,
    })
}

/// `run_exit`'s `exit_status {type, value}`: `normal` carries the code,
/// `signal` the signal; `killed` (Ed stopped it) and `exec` (it never
/// started) are said in words.
pub fn exit_of(status: &Value) -> Exit {
    let value = &status["value"];
    let text = || match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    };
    match status["type"].as_str() {
        Some("signal") => Exit {
            signal: text(),
            ..Exit::default()
        },
        Some("killed") => Exit::because("The program was killed"),
        Some("exec") => Exit::because(&match text() {
            Some(why) => format!("The program could not start: {why}"),
            None => "The program could not start".to_string(),
        }),
        _ => Exit {
            code: value.as_i64(),
            ..Exit::default()
        },
    }
}

/// Keystrokes for the program, base64 of the UTF-8 text.
pub fn stdin_frame(data_b64: &str) -> String {
    json!({ "type": "run_frame", "data": { "pid": 0, "type": "stdin", "data": data_b64 } })
        .to_string()
}

pub fn size_frame(cols: u16, rows: u16) -> String {
    json!({ "type": "run_set_size", "data": { "cols": cols, "rows": rows } }).to_string()
}
