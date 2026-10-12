//! A `Brief` as the compact text `oculus studio brief` prints: no colour,
//! one line per sentence of narration, each word as `start word` and each
//! pause as `start [pause 1.0 s]` after the word it follows.

use std::fmt::{self, Display, Write};

use studio_core::CuedPause;

use super::{Brief, Narration, Neighbour};

impl Display for Brief {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let props = serde_json::to_string(&self.props).unwrap_or_default();
        writeln!(
            f,
            "Scene {} ({} of {}), component {}, props {props}",
            name(self.id.as_deref(), self.index),
            self.index,
            self.count,
            self.component
        )?;
        writeln!(f, "Folder {}", self.dir.display())?;
        writeln!(f)?;
        neighbour(f, "Previous", self.previous.as_ref(), "first scene")?;
        neighbour(f, "Next", self.next.as_ref(), "last scene")?;
        writeln!(f)?;
        match (&self.storyboard, &self.storyboard_missing) {
            (Some(entry), _) => writeln!(f, "{entry}")?,
            (None, why) => writeln!(f, "Storyboard: {}", why.as_deref().unwrap_or("no entry"))?,
        }
        writeln!(f)?;
        let notes = &self.notes;
        let what = if notes.named {
            "Notes (named in the entry)"
        } else {
            "Notes (all; the entry names none)"
        };
        if notes.paths.is_empty() {
            writeln!(f, "Notes: none (no notes/ folder)")?;
        } else {
            writeln!(f, "{what}: {}", notes.paths.join(" "))?;
        }
        if !notes.missing.is_empty() {
            writeln!(f, "  no such file: {}", notes.missing.join(" "))?;
        }
        writeln!(f)?;
        narration(f, &self.narration)?;
        writeln!(f)?;
        writeln!(f, "Check:")?;
        for c in &self.commands {
            writeln!(f, "{c}")?;
        }
        Ok(())
    }
}

fn name(id: Option<&str>, index: usize) -> String {
    id.map_or_else(|| format!("#{index}"), str::to_owned)
}

fn neighbour(
    f: &mut fmt::Formatter<'_>,
    which: &str,
    n: Option<&Neighbour>,
    none: &str,
) -> fmt::Result {
    let Some(n) = n else {
        return writeln!(f, "{which}: none ({none})");
    };
    let written = if n.written {
        "written"
    } else {
        "not written yet"
    };
    writeln!(
        f,
        "{which}: {} ({}), component {} ({written})",
        name(n.id.as_deref(), n.index),
        n.index,
        n.component
    )?;
    if !n.entry {
        return writeln!(f, "  no storyboard entry");
    }
    match (&n.carries_in, &n.hands_on) {
        (None, None) => writeln!(f, "  its entry has no Carries in or Hands on"),
        (c, h) => {
            let mut line = String::from(" ");
            if let Some(c) = c {
                let _ = write!(line, " Carries in: {c}");
            }
            if let Some(h) = h {
                let _ = write!(line, " Hands on: {h}");
            }
            writeln!(f, "{line}")
        }
    }
}

fn narration(f: &mut fmt::Formatter<'_>, n: &Narration) -> fmt::Result {
    match n {
        Narration::Silent { seconds } => {
            writeln!(f, "Narration: none, a silent scene of {seconds:.2} s")
        }
        Narration::Unavailable { error, script } => {
            writeln!(f, "Narration: unavailable: {error}")?;
            match script {
                Some(text) => writeln!(f, "Script: {text}"),
                None => Ok(()),
            }
        }
        Narration::Spoken {
            seconds,
            sentences,
            pauses,
        } => {
            let and_pauses = match pauses.is_empty() {
                true => "",
                false => ", each [pause] at the time pause() returns",
            };
            writeln!(
                f,
                "Narration: {seconds:.2} s; each word at the time word() returns{and_pauses}, in scene seconds"
            )?;
            let pause = |p: &CuedPause| format!("{:.2} [pause {:.1} s]", p.start, p.end - p.start);
            let mut line: Vec<String> = pauses
                .iter()
                .filter(|p| p.after.is_none())
                .map(pause)
                .collect();
            let mut index = 0;
            for sentence in sentences {
                for w in sentence {
                    line.push(format!("{:.2} {}", w.start, w.text));
                    let after = pauses.iter().filter(|p| p.after == Some(index));
                    line.extend(after.map(pause));
                    index += 1;
                }
                writeln!(f, "{}", line.join("  "))?;
                line.clear();
            }
            Ok(())
        }
    }
}
