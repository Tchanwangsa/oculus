//! Tokens and tags without spaCy, as the oracle tags them: whitespace
//! chunks with punctuation peeled off each end and dashes split out; the
//! punctuation gets its Penn tag, `I` is `PRP`, a/an/the `DT`, to `TO`, in
//! `IN`, and an `a` that is the letter rather than the article (a capital
//! `A` after the unit's first word, or an `a` with no word after it) is
//! `NNP`. Every other word is untagged.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use super::chars::lower;
use super::token::Token;

/// misaki's markdown link `[text](feature)`: only the text is read.
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\(([^\)]*)\)").unwrap());
static CHUNK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\S+)(\s*)").unwrap());

const LEAD: &[char] = &['"', '“', '('];
const TRAIL: &[char] = &['.', ',', ';', ':', '!', '?', '"', '”', ')', '…'];
const DASHES: &[char] = &['—', '–'];

pub struct Tagged {
    pub tokens: Vec<Token>,
    /// Words whose reading a heteronym rule chose, by text.
    pub rules: HashSet<String>,
}

/// misaki's `preprocess`: leading whitespace dropped, links read as their
/// text.
pub fn preprocess(text: &str) -> String {
    LINK.replace_all(text.trim_start(), "${1}").into_owned()
}

fn punct_tag(c: char, lead: bool) -> &'static str {
    match c {
        '"' if lead => "``",
        '"' => "''",
        '.' | '!' | '?' => ".",
        ',' => ",",
        ';' | ':' | '—' | '–' | '…' => ":",
        '(' => "-LRB-",
        ')' => "-RRB-",
        '“' => "``",
        '”' => "''",
        _ => "",
    }
}

/// A whitespace chunk as `(text, punctuation tag)` pieces; words have no tag.
fn split_chunk(chunk: &str) -> Vec<(String, Option<&'static str>)> {
    let cs: Vec<char> = chunk.chars().collect();
    let (mut start, mut end) = (0, cs.len());
    while start < end && LEAD.contains(&cs[start]) {
        start += 1;
    }
    while end > start && TRAIL.contains(&cs[end - 1]) {
        end -= 1;
    }
    let mut out: Vec<(String, Option<&'static str>)> = cs[..start]
        .iter()
        .map(|&c| (c.to_string(), Some(punct_tag(c, true))))
        .collect();
    let mut word = String::new();
    for &c in &cs[start..end] {
        if DASHES.contains(&c) {
            if !word.is_empty() {
                out.push((std::mem::take(&mut word), None));
            }
            out.push((c.to_string(), Some(punct_tag(c, false))));
        } else {
            word.push(c);
        }
    }
    if !word.is_empty() {
        out.push((word, None));
    }
    out.extend(
        cs[end..]
            .iter()
            .map(|&c| (c.to_string(), Some(punct_tag(c, false)))),
    );
    out
}

pub fn tokenize(text: &str) -> Tagged {
    let mut pieces: Vec<(String, Option<&'static str>, String)> = Vec::new();
    for m in CHUNK.captures_iter(text) {
        let ws = &m[2];
        let parts = split_chunk(&m[1]);
        let n = parts.len();
        for (j, (t, tag)) in parts.into_iter().enumerate() {
            let ws = if j + 1 == n { ws } else { "" };
            pieces.push((t, tag, ws.to_string()));
        }
    }
    let is_word = |k: usize| pieces.get(k).is_some_and(|p| p.1.is_none());
    let mut words_before = 0;
    let mut tokens = Vec::with_capacity(pieces.len());
    for (k, (t, punct, ws)) in pieces.iter().enumerate() {
        let tag = match punct {
            Some(tag) => tag.to_string(),
            None => {
                let lw = lower(t);
                let mut tag = if t == "I" {
                    "PRP"
                } else {
                    match lw.as_str() {
                        "a" | "an" | "the" => "DT",
                        "to" => "TO",
                        "in" => "IN",
                        _ => "",
                    }
                };
                if lw == "a" && (!is_word(k + 1) || (t == "A" && words_before > 0)) {
                    tag = "NNP";
                }
                words_before += 1;
                tag.to_string()
            }
        };
        tokens.push(Token::new(t, &tag, ws));
    }
    Tagged {
        tokens,
        rules: HashSet::new(),
    }
}
