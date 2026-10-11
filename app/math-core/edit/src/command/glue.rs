//! Control-word glue: a control word ends at the first non-letter, so a
//! letter written right after one (`\alpha` then `x`) would extend it
//! (`\alphax`). Whatever an edit puts next to a control word gets a space
//! between them.

/// What goes between `before` and `after` in place of `insert`, and how
/// many bytes at the end of `before` go with it.
///
/// A space goes in front when `before` ends in a control word and what
/// follows starts with a letter, and behind when `insert` ends in one and
/// `after` starts with a letter. A deletion (`deleting`, `insert` empty)
/// that leaves a control word, one space, and then neither a letter nor a
/// space drops that space: it was the glue for what was deleted, as far
/// as the source can tell (`\alpha x` less `x` is `\alpha`).
pub fn glue(before: &str, insert: &str, after: &str, deleting: bool) -> Glued {
    let follows = if insert.is_empty() { after } else { insert };
    let lead = usize::from(ends_with_word(before) && starts_with_letter(follows));
    let mut text = " ".repeat(lead);
    text.push_str(insert);
    if !insert.is_empty() && ends_with_word(insert) && starts_with_letter(after) {
        text.push(' ');
    }
    let drop = usize::from(
        deleting
            && insert.is_empty()
            && before.strip_suffix(' ').is_some_and(ends_with_word)
            && after
                .chars()
                .next()
                .is_none_or(|c| !c.is_ascii_alphabetic() && !c.is_whitespace()),
    );
    Glued { drop, text, lead }
}

pub struct Glued {
    /// Bytes at the end of `before` to remove (a space).
    pub drop: usize,
    /// What to write: `insert` with any spaces it needs.
    pub text: String,
    /// Bytes of `text` before `insert`.
    pub lead: usize,
}

/// Whether `text` ends in a control word (`\alpha`, not `\\alpha` or `\,`).
pub fn ends_with_word(text: &str) -> bool {
    let rest = text.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    if rest.len() == text.len() {
        return false;
    }
    let slashes = rest.len() - rest.trim_end_matches('\\').len();
    slashes % 2 == 1
}

fn starts_with_letter(text: &str) -> bool {
    text.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::{ends_with_word, glue};

    #[test]
    fn control_words() {
        assert!(ends_with_word(r"a\alpha"));
        assert!(!ends_with_word(r"a\\alpha"));
        assert!(!ends_with_word(r"\,"));
        assert!(!ends_with_word(r"\alpha "));
        assert!(!ends_with_word("abc"));
    }

    #[test]
    fn spaces_where_a_word_meets_a_letter() {
        let text = |before, insert, after| glue(before, insert, after, false).text;
        assert_eq!(text(r"\alpha", "x", ""), " x");
        assert_eq!(text(r"\alpha", "2", ""), "2");
        assert_eq!(text("", r"\sin", "x"), r"\sin ");
        assert_eq!(text(r"\alpha", "", "x"), " ");
        let deleted = glue(r"\alpha ", "", "", true);
        assert_eq!((deleted.drop, deleted.text.as_str()), (1, ""));
        assert_eq!(glue(r"\alpha ", "", "y", true).drop, 0);
    }
}
