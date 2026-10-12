//! Spoken form: narration is read aloud, so the script says notation
//! instead of writing it. Digits, maths and other symbols, Greek letters
//! and dotted abbreviations are flagged with what to write instead;
//! letters, plain punctuation and `[pause]` markers pass, and a malformed
//! marker is flagged with the forms there are.

use crate::project::script::markers;
use std::collections::HashSet;

/// A word of script text that is not spoken form.
#[derive(Debug, PartialEq)]
pub struct Unspoken {
    /// The 0-based line within the text.
    pub line: usize,
    pub token: String,
    /// What to write instead, one entry per problem in the word.
    pub hints: Vec<String>,
    /// A malformed `[pause]` marker, whose one hint says it all.
    pub marker: bool,
}

/// Punctuation a voice reads as pauses or intonation.
const PUNCTUATION: &str = ",.;:!?'\"-\u{2014}\u{2013}()\u{2018}\u{2019}\u{201c}\u{201d}\u{2026}";
/// Markdown emphasis, stripped from the spoken text when inside a word.
const EMPHASIS: [char; 3] = ['*', '_', '`'];

const ABBREVIATIONS: &[(&str, &str)] = &[
    ("e.g.", "for example"),
    ("i.e.", "that is"),
    ("etc.", "and so on"),
    ("vs.", "versus"),
    ("cf.", "compare"),
    ("approx.", "approximately"),
    ("a.k.a.", "also known as"),
    ("w.r.t.", "with respect to"),
    ("resp.", "respectively"),
    ("s.t.", "such that"),
];

const SAID: &[(char, &str)] = &[
    ('%', "percent"),
    ('^', "to the power of (or squared, cubed)"),
    ('=', "equals"),
    ('/', "over, per or divided by"),
    ('+', "plus"),
    ('<', "less than"),
    ('>', "greater than"),
    ('*', "times"),
    ('\u{d7}', "times"),
    ('\u{b7}', "times"),
    ('\u{f7}', "divided by"),
    ('\u{b1}', "plus or minus"),
    ('\u{221a}', "the square root of"),
    ('\u{2211}', "the sum of"),
    ('\u{222b}', "the integral of"),
    ('\u{2208}', "in"),
    ('\u{2209}', "not in"),
    ('\u{2229}', "intersect"),
    ('\u{222a}', "union"),
    ('\u{2264}', "less than or equal to"),
    ('\u{2265}', "greater than or equal to"),
    ('\u{2260}', "is not equal to"),
    ('\u{2248}', "approximately"),
    ('\u{221e}', "infinity"),
    ('\u{2192}', "tends to, or maps to"),
    ('\u{2202}', "partial"),
    ('\u{2207}', "grad"),
    ('&', "and"),
    ('@', "at"),
    ('#', "number"),
    ('~', "about"),
    ('\u{3c0}', "pi"),
    ('\u{3b8}', "theta"),
    ('\u{3bb}', "lambda"),
    ('\u{3bc}', "mu"),
    ('\u{3c3}', "sigma"),
    ('\u{3a3}', "sigma"),
    ('\u{3b1}', "alpha"),
    ('\u{3b2}', "beta"),
    ('\u{3b3}', "gamma"),
    ('\u{3b4}', "delta"),
    ('\u{394}', "delta"),
    ('\u{3b5}', "epsilon"),
    ('\u{3b7}', "eta"),
    ('\u{3c1}', "rho"),
    ('\u{3c4}', "tau"),
    ('\u{3c6}', "phi"),
    ('\u{3c9}', "omega"),
];

/// Every word of `text` that is not spoken form, each token once.
pub fn scan(text: &str) -> Vec<Unspoken> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (line, l) in text.lines().enumerate() {
        let found = markers(l);
        for m in &found {
            let token = &l[m.start..m.end];
            if let Err(hint) = &m.seconds
                && seen.insert(token.to_owned())
            {
                out.push(Unspoken {
                    line,
                    token: token.to_owned(),
                    hints: vec![hint.clone()],
                    marker: true,
                });
            }
        }
        let mut said = l.to_owned();
        for m in found.iter().rev() {
            said.replace_range(m.start..m.end, " ");
        }
        for word in said.split_whitespace() {
            let abbreviation = abbreviation(word);
            let mut hints: Vec<String> = abbreviation.iter().cloned().collect();
            hints.extend(symbols(word));
            if hints.is_empty() {
                continue;
            }
            let token = token(word, abbreviation.is_some());
            if seen.insert(token.clone()) {
                out.push(Unspoken {
                    line,
                    token,
                    hints,
                    marker: false,
                });
            }
        }
    }
    out
}

/// The word without the punctuation around it.
fn token(word: &str, abbreviation: bool) -> String {
    let around = |c: char| c != '.' && PUNCTUATION.contains(c);
    let t = word.trim_matches(around);
    let t = if abbreviation {
        t
    } else {
        t.trim_end_matches('.')
    };
    t.to_owned()
}

/// A dotted abbreviation and how to say it.
fn abbreviation(word: &str) -> Option<String> {
    let w = word
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '.')
        .trim_start_matches('.')
        .to_lowercase();
    let key = if w.ends_with('.') { w } else { format!("{w}.") };
    if let Some((a, said)) = ABBREVIATIONS.iter().find(|(a, _)| *a == key) {
        return Some(format!("write `{said}`, not `{a}`"));
    }
    let chars: Vec<char> = key.chars().collect();
    let dotted = chars
        .windows(3)
        .any(|w| w[0].is_alphabetic() && w[1] == '.' && w[2].is_alphabetic());
    dotted.then(|| "write the abbreviation out as it is said".to_owned())
}

/// Hints for the digits and symbols in a word. Emphasis markers inside a
/// word are emphasis; a word made only of them (`a * b`) is a symbol.
fn symbols(word: &str) -> Vec<String> {
    let alone = word.chars().all(|c| EMPHASIS.contains(&c));
    let mut hints = Vec::new();
    if word.chars().any(char::is_numeric) {
        hints.push(
            "write numbers in words, as they are said (`zero point zero one`, not `0.01`)"
                .to_owned(),
        );
    }
    let mut flagged: Vec<char> = Vec::new();
    for c in word.chars() {
        let greek = ('\u{370}'..='\u{3ff}').contains(&c);
        let fine = c.is_numeric()
            || PUNCTUATION.contains(c)
            || (c.is_alphabetic() && !greek)
            || (EMPHASIS.contains(&c) && !alone);
        if fine || flagged.contains(&c) {
            continue;
        }
        flagged.push(c);
        hints.push(match SAID.iter().find(|(s, _)| *s == c) {
            Some((_, said)) => format!("say `{c}` as `{said}`"),
            None if greek => format!("say `{c}` by the letter's name"),
            None => format!("write `{c}` as it is said"),
        });
    }
    hints
}

#[cfg(test)]
mod tests;
