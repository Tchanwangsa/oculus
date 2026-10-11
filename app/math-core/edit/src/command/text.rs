//! Typing in a text run (`\text{…}`): every character literal, escaped
//! the way LaTeX's text mode needs, spaces included.

use katex::source_map::{SourceRange, continues_cluster};

use super::{
    glue::ends_with_word,
    template::{place, selection_slot},
};
use crate::field::{Field, Outcome};

/// `typed` in place of the selection, as text.
pub fn insert(field: &Field, typed: &str) -> Outcome {
    let (slot, range) = selection_slot(field);
    let escaped = escape(typed, &field.source()[..range.start]);
    if escaped.is_empty() {
        return Outcome::none(field);
    }
    place(field, slot, range, &escaped, "")
}

/// Text-mode LaTeX for `typed`, written after `before`. The commands
/// that end in a letter take `{}` so a space after them is not
/// swallowed; a space after a control word (and any spaces after it,
/// which TeX skips too) or after a control space is `\ ` for the same
/// reason; line breaks and tabs become spaces.
fn escape(typed: &str, before: &str) -> String {
    let mut out = String::new();
    for c in typed.chars() {
        let space_eaten = |out: &str| {
            let tail = if out.is_empty() { before } else { out };
            ends_with_word(tail.trim_end_matches([' ', '\t', '\n', '\r'])) || control_space(tail)
        };
        match c {
            ' ' | '\n' | '\t' | '\r' if space_eaten(&out) => out.push_str(r"\ "),
            '{' | '}' | '$' | '%' | '#' | '&' | '_' => {
                out.push('\\');
                out.push(c);
            }
            '^' => out.push_str(r"\textasciicircum{}"),
            '~' => out.push_str(r"\textasciitilde{}"),
            '\\' => out.push_str(r"\textbackslash{}"),
            '\n' | '\t' | '\r' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Whether `text` ends in `\ ` (an odd run of backslashes, then a space).
fn control_space(text: &str) -> bool {
    text.strip_suffix(' ').is_some_and(|rest| {
        let slashes = rest.len() - rest.trim_end_matches('\\').len();
        slashes % 2 == 1
    })
}

/// Whether a combining mark starts at `at`: it stays with the character
/// before it.
pub fn mark_at(source: &str, at: usize) -> bool {
    let point = Some(SourceRange { start: at, end: at });
    continues_cluster(point, point, &source[at..])
}

/// The start of the cluster that ends at `end`.
pub fn cluster_before(source: &str, end: usize) -> usize {
    let mut at = end;
    while let Some((i, _)) = source[..at].char_indices().next_back() {
        at = i;
        if !mark_at(source, at) {
            break;
        }
    }
    at
}

/// The end of the cluster that starts at `start`.
pub fn cluster_after(source: &str, start: usize) -> usize {
    let mut chars = source[start..].char_indices().skip(1);
    loop {
        match chars.next() {
            Some((i, _)) if mark_at(source, start + i) => {}
            Some((i, _)) => return start + i,
            None => return source.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::non_ascii_literal)]

    use super::{cluster_after, cluster_before, escape};

    #[test]
    fn text_escapes() {
        assert_eq!(escape("a{b}", ""), r"a\{b\}");
        assert_eq!(
            escape("^~\\", ""),
            r"\textasciicircum{}\textasciitilde{}\textbackslash{}"
        );
        assert_eq!(escape("$%#&_", ""), r"\$\%\#\&\_");
        // A space a control word or a control space would swallow.
        assert_eq!(escape(" ", r"\LaTeX"), r"\ ");
        assert_eq!(escape(" ", r"\LaTeX "), r"\ ");
        assert_eq!(escape("  ", r"a\ "), r"\ \ ");
        assert_eq!(escape(" ", r"a\\ "), " ");
        assert_eq!(escape("a b", ""), "a b");
    }

    #[test]
    fn clusters_keep_their_marks() {
        let s = "aดี";
        assert_eq!(cluster_before(s, s.len()), 1);
        assert_eq!(cluster_after(s, 1), s.len());
        assert_eq!(cluster_after(s, 0), 1);
    }
}
