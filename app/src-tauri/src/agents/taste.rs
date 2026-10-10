use std::path::Path;

use super::docs::TASTE_DOC;

/// What [`refresh_taste`] decided to do.
pub enum Refresh {
    /// The guidance was behind; it has been replaced and the bullets kept.
    Rewritten,
    /// Already the current text, or empty of anything to keep.
    Current,
    /// Edited past what a merge can be sure about, so left alone.
    Diverged,
}

/// Bring `TASTE.md`'s instructions up to date without touching what the user
/// put in it.
///
///
/// The template is re-rendered and the user's bullets are carried under their
/// headings. It refuses rather than guesses: prose under a heading, or no
/// known heading left, is reported and left alone.
pub fn refresh_taste(path: &Path) -> Result<Refresh, String> {
    let Ok(current) = std::fs::read_to_string(path) else {
        return Ok(Refresh::Current);
    };
    if current == TASTE_DOC {
        return Ok(Refresh::Current);
    }

    let mut kept: Vec<(String, Vec<String>)> = TASTE_SECTIONS
        .iter()
        .map(|h| ((*h).to_string(), Vec::new()))
        .collect();
    let mut section: Option<usize> = None;
    let mut seen = 0;
    for line in current.lines() {
        if let Some(name) = line.strip_prefix("## ") {
            section = kept
                .iter()
                .position(|(h, _)| h.eq_ignore_ascii_case(name.trim()));
            if section.is_some() {
                seen += 1;
            }
            continue;
        }
        let Some(idx) = section else { continue };
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if !t.starts_with("- ") && !t.starts_with("* ") {
            // Prose under a heading: not a shape the merge can take apart.
            return Ok(Refresh::Diverged);
        }
        kept[idx]
            .1
            .push(format!("- {}", t.trim_start_matches(['-', '*']).trim()));
    }
    if seen == 0 {
        return Ok(Refresh::Diverged);
    }

    let mut out = TASTE_DOC.to_string();
    for (heading, bullets) in &kept {
        if bullets.is_empty() {
            continue;
        }
        let marker = format!("## {heading}\n");
        let Some(at) = out.find(&marker) else {
            continue;
        };
        let at = at + marker.len();
        out.insert_str(at, &format!("\n{}\n", bullets.join("\n")));
    }
    if out == current {
        return Ok(Refresh::Current);
    }
    std::fs::write(path, out).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(Refresh::Rewritten)
}

/// The headings `TASTE.md` ships with; the only ones a bullet carries under.
const TASTE_SECTIONS: [&str; 3] = ["Writing", "Working", "Study"];
