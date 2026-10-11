//! The model's reply, cleaned down to the text to insert.

use super::prompt::CARET;

/// The longest suggestion kept: the brief asks for one sentence.
const MAX_WORDS: usize = 30;
const MAX_CHARS: usize = 240;

/// The text to insert, out of whatever the model wrapped it in: no quotes or
/// fences, no echo of the text either side of the caret, one line, one
/// sentence's length, and spacing that fits the caret. Empty means none.
pub(super) fn clean(raw: &str, before: &str, after: &str) -> String {
    let raw = raw.replace(CARET, "");
    let (lead, body) = unwrap(&raw);
    // A suggestion continues the line it is on.
    let body = body.split('\n').next().unwrap_or("").trim_end_matches('\r');
    let mut s = strip_echo(&lead, body, before);
    s = strip_after_echo(&s, after);

    // Spacing: never double the space before the caret, and leave one
    // between the suggestion and a word right after it.
    let at_gap = before.is_empty() || before.ends_with(char::is_whitespace);
    let s = if at_gap {
        s.trim_start().to_string()
    } else {
        collapse_lead(&s)
    };
    let mut out = cap(s.trim_end());
    if out.trim().is_empty() {
        return String::new();
    }
    if after.starts_with(|c: char| c.is_alphanumeric()) && !out.ends_with(char::is_whitespace) {
        out.push(' ');
    }
    out
}

/// The reply with a label, code fence or matching quotes taken off, and its
/// leading whitespace apart, since that decides the spacing.
fn unwrap(raw: &str) -> (String, String) {
    let mut t = raw.trim();
    let lead: String = raw
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    // A model that set the text on a line of its own meant a new word.
    let lead = if lead.is_empty() && raw.starts_with(['\n', '\r']) && !t.is_empty() {
        " ".to_string()
    } else {
        lead
    };
    for label in ["completion:", "suggestion:", "continuation:"] {
        if t.get(..label.len())
            .is_some_and(|h| h.eq_ignore_ascii_case(label))
        {
            t = t[label.len()..].trim();
        }
    }
    if let Some(rest) = t.strip_prefix("```") {
        // The fence's own line may name a language.
        let inner = match rest.split_once('\n') {
            Some((_, body)) => body,
            None => rest,
        };
        let inner = inner.trim_end();
        let inner = inner.strip_suffix("```").unwrap_or(inner);
        return (lead, inner.trim().to_string());
    }
    for (open, close) in [('"', '"'), ('“', '”'), ('\'', '\''), ('«', '»')] {
        let mut cs = t.chars();
        if t.chars().count() >= 2 && cs.next() == Some(open) && cs.next_back() == Some(close) {
            let inner = cs.as_str();
            if !inner.contains(open) && !inner.contains(close) {
                return (lead, inner.trim().to_string());
            }
        }
    }
    (lead, t.to_string())
}

/// Drop a restatement of the text before the caret from the reply's start:
/// the longest run that `before` ends with, starting on a word boundary
/// there. A reply that opened with a space meant a new word, so only a long
/// overlap counts as an echo then.
fn strip_echo(lead: &str, body: &str, before: &str) -> String {
    let min = if lead.is_empty() { 3 } else { 12 };
    let mut best: Option<usize> = None;
    for (end, _) in body
        .char_indices()
        .skip(1)
        .chain(std::iter::once((body.len(), ' ')))
    {
        let piece = &body[..end];
        if piece.chars().count() < min || !before.ends_with(piece) {
            continue;
        }
        let start = before.len() - piece.len();
        let boundary = before[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        if boundary {
            best = Some(end);
        }
    }
    match best {
        Some(end) => body[end..].to_string(),
        None => format!("{lead}{body}"),
    }
}

/// Drop a restatement of the text after the caret from the reply's end.
fn strip_after_echo(s: &str, after: &str) -> String {
    let after = after.trim_start();
    let trimmed = s.trim_end();
    let mut cut = None;
    for (start, _) in trimmed.char_indices() {
        let piece = &trimmed[start..];
        if piece.trim().chars().count() >= 4 && after.starts_with(piece.trim_start()) {
            cut = Some(start);
            break;
        }
    }
    match cut {
        Some(start) => trimmed[..start].to_string(),
        None => s.to_string(),
    }
}

/// At most one space ahead of the text.
fn collapse_lead(s: &str) -> String {
    let trimmed = s.trim_start();
    if trimmed.len() == s.len() {
        s.to_string()
    } else {
        format!(" {trimmed}")
    }
}

/// The first [`MAX_WORDS`] words and [`MAX_CHARS`] characters, cut at a word.
fn cap(s: &str) -> String {
    let mut words = 0;
    let mut in_word = false;
    let mut end = s.len();
    for (i, c) in s.char_indices() {
        if c.is_whitespace() {
            in_word = false;
        } else if !in_word {
            in_word = true;
            words += 1;
            if words > MAX_WORDS {
                end = i;
                break;
            }
        }
    }
    let s = &s[..end];
    if s.chars().count() <= MAX_CHARS {
        return s.trim_end().to_string();
    }
    let limit = s
        .char_indices()
        .nth(MAX_CHARS)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    let cut = s[..limit]
        .rfind(char::is_whitespace)
        .filter(|i| *i > 0)
        .unwrap_or(limit);
    s[..cut].trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_quotes_and_fences_come_off() {
        assert_eq!(
            clean("\"the shortest path\"", "Dijkstra finds ", ""),
            "the shortest path"
        );
        assert_eq!(
            clean("“the shortest path”", "Dijkstra finds ", ""),
            "the shortest path"
        );
        assert_eq!(
            clean("```\nthe shortest path\n```", "Dijkstra finds ", ""),
            "the shortest path"
        );
        assert_eq!(
            clean("```markdown\nthe shortest path\n```", "Dijkstra finds ", ""),
            "the shortest path"
        );
        assert_eq!(
            clean("Completion: the shortest path", "Dijkstra finds ", ""),
            "the shortest path"
        );
        assert_eq!(
            clean("```the shortest path```", "Dijkstra finds ", ""),
            "the shortest path"
        );
        // A short non-ASCII reply is not sliced through while looking for a label.
        assert_eq!(clean("é", "caf", ""), "é");
    }

    #[test]
    fn a_word_is_continued_without_a_space_and_a_new_one_gets_one() {
        assert_eq!(
            clean("thm runs in $O(n^2)$", "The algori", ""),
            "thm runs in $O(n^2)$"
        );
        assert_eq!(
            clean(" theory of computation", "the", ""),
            " theory of computation"
        );
        assert_eq!(clean("   of computation", "theory", ""), " of computation");
        // The caret already follows a space.
        assert_eq!(clean(" the cat", "I saw ", ""), "the cat");
        assert_eq!(clean("\nthe cat", "", ""), "the cat");
    }

    #[test]
    fn a_restatement_of_the_text_before_the_caret_is_dropped() {
        assert_eq!(clean("algorithm runs in", "The algori", ""), "thm runs in");
        assert_eq!(
            clean(
                "Dijkstra's algorithm finds the shortest path.",
                "Dijkstra's algorithm finds the",
                ""
            ),
            " shortest path."
        );
        assert_eq!(
            clean(" quick brown fox jumps", "the quick brown fox", ""),
            " jumps"
        );
        // Starts on a word boundary only: "cat" is not an echo of "concat".
        assert_eq!(clean("cat", "concat", ""), "cat");
        // Too short to be an echo once the reply opened with a space.
        assert_eq!(clean(" the end", "of the", ""), " the end");
    }

    #[test]
    fn a_restatement_of_the_text_after_the_caret_is_dropped() {
        assert_eq!(clean("big and the rest.", "A ", " and the rest."), "big");
    }

    #[test]
    fn a_word_right_after_the_caret_keeps_its_space() {
        assert_eq!(clean("big", "The ", "cat sat"), "big ");
    }

    #[test]
    fn only_the_first_line_and_one_sentence_s_length_survive() {
        assert_eq!(
            clean("the shortest path\nSecond paragraph", "Finds ", ""),
            "the shortest path"
        );
        let long = (1..=60)
            .map(|i| format!("w{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let out = clean(&long, "x ", "");
        assert_eq!(out.split_whitespace().count(), MAX_WORDS);
        assert!(out.starts_with("w1 w2"));
        let wordy = "abcdefghij ".repeat(40);
        assert!(clean(&wordy, "x ", "").chars().count() <= MAX_CHARS);
    }

    #[test]
    fn nothing_useful_is_nothing() {
        assert_eq!(clean("", "x ", ""), "");
        assert_eq!(clean("  \n ", "x ", ""), "");
        assert_eq!(clean("\"\"", "x ", ""), "");
        assert_eq!(clean("<CARET/>", "x ", ""), "");
    }
}
