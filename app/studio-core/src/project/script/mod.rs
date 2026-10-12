//! `script.md`: a video's narration, one `## <id>` section per narrated
//! scene, written as it is said. A section's text is its prose with the
//! emphasis markers (`*`, `_`, backticks) and pause markers (`[pause]`)
//! removed and whitespace collapsed; nothing else is interpreted. That text,
//! with its pauses written back at their lengths (`clip_text`), is what the
//! narration manifest must match.

mod pause;

pub use pause::MAX_PAUSE;
pub(crate) use pause::markers;

use super::video::valid_id;

#[derive(Clone, Debug, PartialEq)]
pub struct Script {
    /// In file order.
    pub sections: Vec<ScriptSection>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptSection {
    /// The heading's id, matching a scene's `id` in video.json.
    pub id: String,
    /// What is said: the prose, emphasis and pause markers stripped and
    /// whitespace collapsed.
    pub text: String,
    /// Its pauses, in order.
    pub pauses: Vec<ScriptPause>,
    /// The prose as written: the lines after the heading, joined by `\n`.
    pub raw: String,
    /// The heading's line (1-based); `raw`'s first line is the next one.
    pub line: usize,
}

/// A `[pause]` in a section.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScriptPause {
    /// How many of the text's whitespace-separated words come before it.
    pub words: usize,
    /// Its seconds, or `None` for the theme's `narration.pause`.
    pub seconds: Option<f64>,
}

impl Script {
    /// Parses `script.md`. Text before the first heading, a heading that is
    /// not an id, a repeated id and an empty section are errors naming
    /// their line.
    pub fn parse(src: &str) -> Result<Script, String> {
        let mut sections: Vec<ScriptSection> = Vec::new();
        let mut raw: Vec<&str> = Vec::new();
        for (i, line) in src.lines().enumerate() {
            let n = i + 1;
            let Some(id) = heading(line) else {
                if sections.is_empty() && !line.trim().is_empty() {
                    return Err(format!(
                        "line {n}: text before the first `## <id>` heading; every paragraph belongs to a scene's section"
                    ));
                }
                raw.push(line);
                continue;
            };
            close(&mut sections, &mut raw)?;
            if !valid_id(id) {
                return Err(format!(
                    "line {n}: `## {id}` is not a scene id; ids are lowercase letters, digits and `-`"
                ));
            }
            if let Some(first) = sections.iter().find(|s| s.id == id) {
                return Err(format!(
                    "line {n}: `## {id}` repeats the section at line {}; each scene has one section",
                    first.line
                ));
            }
            sections.push(ScriptSection {
                id: id.to_owned(),
                text: String::new(),
                pauses: Vec::new(),
                raw: String::new(),
                line: n,
            });
        }
        close(&mut sections, &mut raw)?;
        Ok(Script { sections })
    }

    pub fn section(&self, id: &str) -> Option<&ScriptSection> {
        self.sections.iter().find(|s| s.id == id)
    }
}

impl ScriptSection {
    /// The text with each pause written back as `[pause <seconds>]`, a
    /// bare one at `pause` (the theme's): what a clip is made from, and
    /// so what its manifest entry must match. Without pauses, `text`.
    pub fn clip_text(&self, pause: f64) -> String {
        let mut out: Vec<String> = Vec::new();
        let mut pauses = self.pauses.iter().peekable();
        for (i, word) in self.text.split_whitespace().enumerate() {
            while let Some(p) = pauses.next_if(|p| p.words == i) {
                out.push(pause::written(p.length(pause)));
            }
            out.push(word.to_owned());
        }
        out.extend(pauses.map(|p| pause::written(p.length(pause))));
        out.join(" ")
    }
}

impl ScriptPause {
    /// Its seconds: its own, or `pause` (the theme's) for a bare one.
    pub fn length(&self, pause: f64) -> f64 {
        self.seconds.unwrap_or(pause)
    }
}

/// The id of a `## id` line; `#` and `###` lines are text.
fn heading(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix("##")?;
    (!rest.starts_with('#')).then(|| rest.trim())
}

/// Gives the last section its collected lines.
fn close(sections: &mut [ScriptSection], raw: &mut Vec<&str>) -> Result<(), String> {
    let lines = std::mem::take(raw);
    let Some(s) = sections.last_mut() else {
        return Ok(());
    };
    s.raw = lines.join("\n");
    (s.text, s.pauses) = spoken_with_pauses(&s.raw);
    if s.text.is_empty() {
        return Err(format!(
            "line {}: `## {}` has no text; write what is said, or remove the heading to keep the scene silent",
            s.line, s.id
        ));
    }
    Ok(())
}

/// Prose as it is narrated, and its pauses. Every pause marker, malformed
/// ones too, is cut from the text and splits the words either side of it;
/// lint names the malformed ones.
fn spoken_with_pauses(raw: &str) -> (String, Vec<ScriptPause>) {
    let mut plain = String::new();
    let mut pauses = Vec::new();
    for line in raw.lines() {
        let mut from = 0;
        for m in markers(line) {
            plain.push_str(&line[from..m.start]);
            plain.push(' ');
            from = m.end;
            if let Ok(seconds) = m.seconds {
                let words = spoken(&plain).split_whitespace().count();
                pauses.push(ScriptPause { words, seconds });
            }
        }
        plain.push_str(&line[from..]);
        plain.push('\n');
    }
    (spoken(&plain), pauses)
}

/// Prose as it is narrated: emphasis markers removed, whitespace collapsed.
fn spoken(raw: &str) -> String {
    let plain: String = raw
        .chars()
        .filter(|c| !matches!(c, '*' | '_' | '`'))
        .collect();
    plain.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests;
