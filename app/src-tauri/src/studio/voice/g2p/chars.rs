//! Python's `str` semantics where the port depends on them: letters, cases
//! and digits by Unicode general category (`\p{L}`, `\p{Lu}`, `\p{Ll}`,
//! `\p{Nd}`, as CPython's `isalpha` and the `regex` module read them), and
//! lengths and slices counted in characters, never bytes.

use std::sync::LazyLock;

use regex::Regex;

static LETTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\p{L}$").unwrap());
static UPPER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\p{Lu}$").unwrap());
static LOWER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\p{Ll}$").unwrap());
static DIGIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\p{Nd}$").unwrap());

fn category(re: &Regex, c: char) -> bool {
    let mut buf = [0u8; 4];
    re.is_match(c.encode_utf8(&mut buf))
}

pub fn is_letter(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_alphabetic()
    } else {
        category(&LETTER, c)
    }
}

pub fn is_upper(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_uppercase()
    } else {
        category(&UPPER, c)
    }
}

pub fn is_lower(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_lowercase()
    } else {
        category(&LOWER, c)
    }
}

pub fn is_digit(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_digit()
    } else {
        category(&DIGIT, c)
    }
}

/// `str.isalpha()`: not empty, and every character a letter.
pub fn is_alpha(s: &str) -> bool {
    !s.is_empty() && s.chars().all(is_letter)
}

/// Any letter at all: a chunk with one must come out with phonemes.
pub fn has_letter(s: &str) -> bool {
    s.chars().any(is_letter)
}

pub fn lower(s: &str) -> String {
    s.to_lowercase()
}

pub fn upper(s: &str) -> String {
    s.to_uppercase()
}

/// `str.capitalize()`: the first character upper case, the rest lower.
pub fn capitalize(s: &str) -> String {
    let mut cs = s.chars();
    match cs.next() {
        Some(first) => first
            .to_uppercase()
            .chain(cs.as_str().to_lowercase().chars())
            .collect(),
        None => String::new(),
    }
}

pub fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// `s[1:]`.
pub fn tail(s: &str) -> &str {
    let mut cs = s.chars();
    cs.next();
    cs.as_str()
}

/// `s[:-n]` for `n >= 1` (empty when `s` is shorter).
pub fn drop_last(s: &str, n: usize) -> &str {
    match s.char_indices().rev().nth(n.saturating_sub(1)) {
        Some((i, _)) => &s[..i],
        None => "",
    }
}

/// `s[-n]`, counted from 1 at the end.
pub fn nth_last(s: &str, n: usize) -> Option<char> {
    s.chars().rev().nth(n.checked_sub(1)?)
}
