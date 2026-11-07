//! `oculus widget check`: a widget's TSX through the same gate as the chat
//! and `artifact save` (`transpile`), then a lint on its oxc AST for what
//! the frame would refuse or the theme would lose. One file per rule family;
//! `docs/widgets.md` ("Checking a widget before it is sent") states each rule.
//!
//! - `title`: the first line is `// title: …`.
//! - `colour`, `font`, `tex`: literals (`literals.rs`).
//! - `sandbox`: network, storage, cookies and `eval` (`sandbox.rs`).
//! - `cleanup`: timers and listeners only in an effect that cancels them
//!   (`cleanup.rs`).

mod cleanup;
mod literals;
mod sandbox;
#[cfg(test)]
mod tests;

use super::transpile::{line_col, transpile};
use oxc::allocator::Allocator;
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::span::SourceType;

/// One problem: the rule it breaks, where (1-based), and what to do instead.
#[derive(serde::Serialize, Debug, Clone, PartialEq)]
pub struct Finding {
    pub rule: &'static str,
    /// `None` for a transpile error that names no place (an import, a
    /// missing default export).
    pub line: Option<usize>,
    pub col: Option<usize>,
    pub message: String,
}

impl Finding {
    /// A finding at byte `offset` of `src`.
    fn at(src: &str, offset: u32, rule: &'static str, message: String) -> Self {
        let (line, col) = line_col(src, offset as usize);
        Finding {
            rule,
            line: Some(line),
            col: Some(col),
            message,
        }
    }
}

/// Every finding in `tsx`, in source order. A widget that doesn't transpile
/// gets that one finding and no lint.
pub fn check(tsx: &str) -> Vec<Finding> {
    if let Err(error) = transpile(tsx) {
        return vec![transpile_finding(&error)];
    }
    let alloc = Allocator::default();
    let program = Parser::new(&alloc, tsx, SourceType::tsx()).parse().program;
    let scoping = SemanticBuilder::new()
        .build(&program)
        .semantic
        .into_scoping();

    let mut out = Vec::new();
    if let Some(finding) = title(tsx) {
        out.push(finding);
    }
    out.extend(literals::check(&program, tsx));
    out.extend(sandbox::check(&program, &scoping, tsx));
    out.extend(cleanup::check(&program, &scoping, tsx));
    out.sort_by_key(|f| (f.line, f.col));
    out
}

/// `transpile`'s `"<line>:<col> <message>"`, or a message with no place.
fn transpile_finding(error: &str) -> Finding {
    let place = error.split_once(' ').and_then(|(head, message)| {
        let (line, col) = head.split_once(':')?;
        Some((line.parse().ok()?, col.parse().ok()?, message))
    });
    match place {
        Some((line, col, message)) => Finding {
            rule: "transpile",
            line: Some(line),
            col: Some(col),
            message: message.to_string(),
        },
        None => Finding {
            rule: "transpile",
            line: None,
            col: None,
            message: error.to_string(),
        },
    }
}

/// The card's title is the first non-blank line's `// title: …`, as the
/// frontend reads it (`widget/title.ts`); without one the card says "Widget".
fn title(tsx: &str) -> Option<Finding> {
    let first = tsx.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let named = first
        .trim_start()
        .strip_prefix("//")
        .map(str::trim_start)
        .and_then(|rest| {
            let (key, value) = rest.split_once(':')?;
            Some(key.trim().eq_ignore_ascii_case("title") && !value.trim().is_empty())
        })
        .unwrap_or(false);
    (!named).then(|| Finding {
        rule: "title",
        line: Some(1),
        col: Some(1),
        message:
            "the first line names the widget: `// title: …` (the card says \"Widget\" without it)"
                .into(),
    })
}
