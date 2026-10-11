//! `script.md`: a video's narration, one `## <id>` section per narrated
//! scene, written as it is said. A section's text is its prose with the
//! emphasis markers (`*`, `_`, backticks) removed and whitespace collapsed;
//! nothing else is interpreted. That text is what the narration manifest
//! must match.

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
    /// What is said: the prose, emphasis stripped and whitespace collapsed.
    pub text: String,
    /// The prose as written: the lines after the heading, joined by `\n`.
    pub raw: String,
    /// The heading's line (1-based); `raw`'s first line is the next one.
    pub line: usize,
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
    s.text = spoken(&s.raw);
    if s.text.is_empty() {
        return Err(format!(
            "line {}: `## {}` has no text; write what is said, or remove the heading to keep the scene silent",
            s.line, s.id
        ));
    }
    Ok(())
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
