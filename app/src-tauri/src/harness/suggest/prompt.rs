//! The user turn: the note's title and subject, and the text either side of
//! the caret, clipped.

/// Where the caret is, in the prompt. Never in the note itself.
pub(super) const CARET: &str = "<CARET/>";
/// How much of the note the prompt carries either side of the caret; the
/// webview already sends about this much.
const BEFORE_CHARS: usize = 4000;
const AFTER_CHARS: usize = 1500;

/// The user turn, or None when no turn should be spent: the caret sits inside
/// a word (letters on both sides), where any insertion splits it.
pub(super) fn prompt(path: &str, before: &str, after: &str) -> Option<String> {
    let last = before.chars().next_back();
    let next = after.chars().next();
    if last.is_some_and(char::is_alphanumeric) && next.is_some_and(char::is_alphanumeric) {
        return None;
    }
    let (title, subject) = note_names(path);
    let subject = subject
        .map(|c| format!(", in the subject {c}"))
        .unwrap_or_default();
    let (before, after) = (tail(before, BEFORE_CHARS), head(after, AFTER_CHARS));
    Some(format!(
        "Complete the student's note at the caret.\n\n\
         The note is \"{title}\"{subject} (`{path}`). {}\n\n\
         <note>\n{before}{CARET}{after}\n</note>\n\n\
         Reply with only the text to insert at {CARET}.",
        caret_note(&before),
    ))
}

/// The note's title (its file stem) and subject code, from a path like
/// `courses/<code>/documents/<name>.md`.
fn note_names(path: &str) -> (String, Option<String>) {
    let mut parts = path.split('/');
    let subject = match (parts.next(), parts.next()) {
        (Some("courses"), Some(code)) if !code.is_empty() && path.matches('/').count() >= 2 => {
            Some(code.to_string())
        }
        _ => None,
    };
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.strip_suffix(".md").unwrap_or(name);
    (stem.replace('_', " "), subject)
}

/// What the text just before the caret means for the first character of
/// the reply — the one thing a model cannot see from the marker alone.
fn caret_note(before: &str) -> String {
    let Some(last) = before.chars().next_back() else {
        return "The caret is at the start of the note.".into();
    };
    if before.trim().is_empty() {
        return "The caret is at the start of the note.".into();
    }
    if last == '\n' {
        return "The caret is at the start of a new line.".into();
    }
    if last.is_whitespace() {
        return "The caret follows a space, so do not start with one.".into();
    }
    if last.is_alphanumeric() {
        let fragment: String = {
            let rev: Vec<char> = before
                .chars()
                .rev()
                .take_while(|c| c.is_alphanumeric())
                .take(40)
                .collect();
            rev.into_iter().rev().collect()
        };
        return format!(
            "The caret is directly after \"{fragment}\", with no space. If that word is unfinished, \
             continue it with no leading space; if it is complete, start with a space."
        );
    }
    format!(
        "The caret is directly after \"{last}\". Start with a space if the next word needs one."
    )
}

/// The last `n` characters of `s`, marked when cut.
fn tail(s: &str, n: usize) -> String {
    match s.char_indices().rev().nth(n.saturating_sub(1)) {
        Some((i, _)) if i > 0 => format!("…{}", &s[i..]),
        _ => s.to_string(),
    }
}

/// The first `n` characters of `s`, marked when cut.
fn head(s: &str, n: usize) -> String {
    match s.char_indices().nth(n) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_names_the_note_and_marks_the_caret() {
        let p = prompt(
            "courses/COMP3121/documents/week_3_notes.md",
            "Greedy algorithms ",
            "\n\nNext",
        )
        .unwrap();
        assert!(p.contains("\"week 3 notes\", in the subject COMP3121"));
        assert!(p.contains("`courses/COMP3121/documents/week_3_notes.md`"));
        assert!(p.contains("Greedy algorithms <CARET/>\n\nNext"));
        assert!(p.contains("follows a space"));
        let mid = prompt("documents/x.md", "The algori", "").unwrap();
        assert!(mid.contains("directly after \"algori\""));
        assert!(!mid.contains("in the subject"));
    }

    #[test]
    fn no_turn_is_spent_inside_a_word() {
        assert!(prompt("courses/X/documents/a.md", "algo", "rithm").is_none());
        assert!(prompt("courses/X/documents/a.md", "algo", " rithm").is_some());
    }

    #[test]
    fn the_prompt_carries_only_the_text_near_the_caret() {
        let before = format!("START{}", "a".repeat(BEFORE_CHARS));
        let after = format!("{}END", "b".repeat(AFTER_CHARS));
        let p = prompt(
            "courses/X/documents/a.md",
            &format!("{before} "),
            &format!(" {after}"),
        )
        .unwrap();
        assert!(!p.contains("START") && !p.contains("END"));
        assert!(p.contains("…a") && p.contains("b…"));
        // Characters, not bytes: a cut never lands inside one.
        assert_eq!(tail("éééé", 2), "…éé");
        assert_eq!(head("éééé", 2), "éé…");
        assert_eq!(tail("éé", 5), "éé");
    }
}
