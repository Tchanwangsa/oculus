//! A command from JS: `FieldCommand`'s JSON, named as in
//! `oculus_math_edit::Command` (`{"insert": "x"}`, `"backspace"`,
//! `{"left": {"extend": true}}`, `{"up": [12.5, null]}`).

use oculus_math_edit::Command;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum Json {
    Insert(String),
    Template(String),
    Paste(String),
    Backspace,
    Delete,
    DeleteLine,
    Left(Extend),
    Right(Extend),
    /// Each stop's rendered x by stop id; `null` (what `JSON.stringify`
    /// writes for NaN) where the view has none.
    Up(Vec<Option<f64>>),
    Down(Vec<Option<f64>>),
    Home(Extend),
    End(Extend),
    SelectAll,
    Tab,
    ShiftTab,
    Enter,
    Escape,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Extend {
    #[serde(default)]
    extend: bool,
}

fn xs(xs: Vec<Option<f64>>) -> Vec<f64> {
    xs.into_iter().map(|x| x.unwrap_or(f64::NAN)).collect()
}

/// The command `json` names, or why it names none.
pub fn command(json: &str) -> Result<Command, String> {
    let json: Json = serde_json::from_str(json).map_err(|e| format!("bad field command: {e}"))?;
    Ok(match json {
        Json::Insert(text) => Command::Insert(text),
        Json::Template(latex) => Command::Template(latex),
        Json::Paste(latex) => Command::Paste(latex),
        Json::Backspace => Command::Backspace,
        Json::Delete => Command::Delete,
        Json::DeleteLine => Command::DeleteLine,
        Json::Left(Extend { extend }) => Command::Left { extend },
        Json::Right(Extend { extend }) => Command::Right { extend },
        Json::Up(x) => Command::Up { xs: xs(x) },
        Json::Down(x) => Command::Down { xs: xs(x) },
        Json::Home(Extend { extend }) => Command::Home { extend },
        Json::End(Extend { extend }) => Command::End { extend },
        Json::SelectAll => Command::SelectAll,
        Json::Tab => Command::Tab,
        Json::ShiftTab => Command::ShiftTab,
        Json::Enter => Command::Enter,
        Json::Escape => Command::Escape,
    })
}
