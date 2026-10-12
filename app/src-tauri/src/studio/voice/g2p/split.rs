//! Words into sub-tokens and back: misaki's `SUBTOKEN_REGEX` (hand-written,
//! since it needs lookahead), `retokenize`, `merge_tokens` and
//! `resolve_tokens`. A hyphenated or camel-case word becomes a list of
//! sub-tokens that the word path reads as one or in parts.

use super::chars::{is_digit, is_letter, is_lower, is_upper};
use super::stress::{apply_stress, stress_weight, PRIMARY};
use super::token::{Token, NON_QUOTE_PUNCTS};

/// Sub-tokens that read as nothing when nothing else reads them.
pub const SUBTOKEN_JUNKS: &str = "',-._‘’/";
const PUNCTS: &str = ";:,.!?—…\"“”";
const PUNCT_TAGS: [&str; 11] = [
    ".", ",", "-LRB-", "-RRB-", "``", "\"\"", "''", ":", "$", "#", "NFP",
];

/// Stands in for a part nothing read, so a word with one is caught.
pub const UNK: char = '❓';

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Word {
    One(Token),
    Parts(Vec<Token>),
}

fn is_quote(c: char) -> bool {
    matches!(c, '\'' | '‘' | '’')
}

fn run(cs: &[char], i: usize, f: impl Fn(char) -> bool) -> usize {
    i + cs[i..].iter().take_while(|&&c| f(c)).count()
}

/// `(?:\d?[,.]?\d)+`, each repetition taking the first option that fits.
fn digits(cs: &[char], start: usize) -> Option<usize> {
    let d = |p: usize| cs.get(p).is_some_and(|&c| is_digit(c));
    let sep = |p: usize| cs.get(p).is_some_and(|&c| c == ',' || c == '.');
    let mut p = start;
    loop {
        let next = if d(p) && sep(p + 1) && d(p + 2) {
            p + 3
        } else if (d(p) || sep(p)) && d(p + 1) {
            p + 2
        } else if d(p) {
            p + 1
        } else {
            break;
        };
        p = next;
    }
    (p > start).then_some(p)
}

/// `\p{L}*?(?:['‘’]\p{L})*?\p{Ll}(?=\p{Lu})`: the shortest run ending in a
/// lower-case letter followed by a capital, tried in the regex's order.
fn camel(cs: &[char], i: usize) -> Option<usize> {
    let mut a = 0;
    loop {
        if a > 0 && !cs.get(i + a - 1).is_some_and(|&c| is_letter(c)) {
            return None;
        }
        let mut p = i + a;
        loop {
            if cs.get(p).is_some_and(|&c| is_lower(c))
                && cs.get(p + 1).is_some_and(|&c| is_upper(c))
            {
                return Some(p + 1);
            }
            if cs.get(p).is_some_and(|&c| is_quote(c))
                && cs.get(p + 1).is_some_and(|&c| is_letter(c))
            {
                p += 2;
            } else {
                break;
            }
        }
        a += 1;
    }
}

/// The end of the sub-token starting at `i`, trying `SUBTOKEN_REGEX`'s
/// alternatives in order; `None` skips the character, as `findall` does.
fn subtoken_at(cs: &[char], i: usize) -> Option<usize> {
    let c = cs[i];
    if i == 0 && is_quote(c) {
        return Some(run(cs, i, is_quote));
    }
    if is_upper(c)
        && cs.get(i + 1).is_some_and(|&c| is_upper(c))
        && cs.get(i + 2).is_some_and(|&c| is_lower(c))
    {
        return Some(i + 1);
    }
    let number = if i == 0 && c == '-' {
        digits(cs, 1).or_else(|| digits(cs, 0))
    } else {
        digits(cs, i)
    };
    if number.is_some() {
        return number;
    }
    if c == '-' || c == '_' {
        return Some(run(cs, i, |c| c == '-' || c == '_'));
    }
    if is_quote(c) && run(cs, i, is_quote) >= i + 2 {
        return Some(run(cs, i, is_quote));
    }
    if let Some(end) = camel(cs, i) {
        return Some(end);
    }
    if is_letter(c) {
        let mut p = run(cs, i, is_letter);
        while cs.get(p).is_some_and(|&c| is_quote(c))
            && cs.get(p + 1).is_some_and(|&c| is_letter(c))
        {
            p += 2;
        }
        return Some(p);
    }
    if !(c == '-' || c == '_' || is_letter(c) || is_quote(c) || is_digit(c)) {
        return Some(i + 1);
    }
    if is_quote(c) && run(cs, i, is_quote) == cs.len() {
        return Some(cs.len());
    }
    None
}

/// misaki's `subtokenize`: `O'Brien` is `O'B` + `rien`, `PostgreSQL` is
/// `Postgre` + `SQL`, `well-known` is `well` + `-` + `known`.
pub fn subtokenize(word: &str) -> Vec<String> {
    let cs: Vec<char> = word.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        match subtoken_at(&cs, i) {
            Some(end) => {
                out.push(cs[i..end].iter().collect());
                i = end;
            }
            None => i += 1,
        }
    }
    out
}

/// What a punctuation token reads as: a spaced hyphen or en dash as an em
/// dash, brackets and quotes as themselves (curly), anything else as its
/// clause marks.
fn punctuation(tk: &Token) -> Option<String> {
    if tk.tag == ":" && (tk.text == "-" || tk.text == "–") {
        return Some("—".to_string());
    }
    let letters = tk
        .text
        .chars()
        .flat_map(char::to_lowercase)
        .all(|c| c.is_ascii_lowercase());
    if !PUNCT_TAGS.contains(&tk.tag.as_str()) || letters {
        return None;
    }
    Some(match tk.tag.as_str() {
        "-LRB-" => "(".to_string(),
        "-RRB-" => ")".to_string(),
        "``" => "“".to_string(),
        "\"\"" | "''" => "”".to_string(),
        _ => tk.text.chars().filter(|&c| PUNCTS.contains(c)).collect(),
    })
}

/// misaki's `retokenize`: unread tokens split into sub-tokens, punctuation
/// read as itself, and the sub-tokens of one word (no whitespace between
/// them) gathered into one list.
pub fn retokenize(tokens: Vec<Token>) -> Vec<Word> {
    let mut words: Vec<Word> = Vec::new();
    for token in tokens {
        let mut tks: Vec<Token> = if token.phonemes.is_none() {
            subtokenize(&token.text)
                .iter()
                .map(|t| Token::new(t, &token.tag, ""))
                .collect()
        } else {
            Vec::new()
        };
        if tks.is_empty() {
            tks.push(token.clone());
        }
        if let Some(last) = tks.last_mut() {
            last.whitespace = token.whitespace.clone();
        }
        for mut tk in tks {
            if tk.phonemes.is_none() {
                tk.phonemes = punctuation(&tk);
            }
            if tk.phonemes.is_some() {
                words.push(Word::One(tk));
                continue;
            }
            if let Some(Word::Parts(parts)) = words.last_mut() {
                if parts.last().is_some_and(|p| p.whitespace.is_empty()) {
                    parts.push(tk);
                    continue;
                }
            }
            words.push(if tk.whitespace.is_empty() {
                Word::Parts(vec![tk])
            } else {
                Word::One(tk)
            });
        }
    }
    words
        .into_iter()
        .map(|w| match w {
            Word::Parts(mut parts) if parts.len() == 1 => Word::One(parts.remove(0)),
            w => w,
        })
        .collect()
}

/// The text of merged tokens, as written.
pub fn merged_text(parts: &[Token]) -> String {
    let mut text = String::new();
    for (i, tk) in parts.iter().enumerate() {
        text.push_str(&tk.text);
        if i + 1 < parts.len() {
            text.push_str(&tk.whitespace);
        }
    }
    text
}

/// The tag of the part with the most capitals-weighted characters.
pub fn merged_tag(parts: &[Token]) -> String {
    let weight = |tk: &Token| {
        tk.text
            .chars()
            .map(|c| {
                if c.to_lowercase().eq(std::iter::once(c)) {
                    1
                } else {
                    2
                }
            })
            .sum::<usize>()
    };
    let mut best: Option<&Token> = None;
    for tk in parts {
        if best.is_none_or(|b| weight(tk) > weight(b)) {
            best = Some(tk);
        }
    }
    best.map(|t| t.tag.clone()).unwrap_or_default()
}

/// The parts' phonemes joined (a space before a part marked `prespace`),
/// [`UNK`] for a part nothing read.
pub fn merged_phonemes(parts: &[Token]) -> String {
    let mut ps = String::new();
    for tk in parts {
        let own = tk.phonemes.as_deref();
        if tk.prespace
            && ps.chars().last().is_some_and(|c| !c.is_whitespace())
            && own.is_some_and(|p| !p.is_empty())
        {
            ps.push(' ');
        }
        match own {
            Some(p) => ps.push_str(p),
            None => ps.push(UNK),
        }
    }
    ps
}

/// misaki's `resolve_tokens`: junk parts read as nothing; parts of a word
/// with a space, a slash or mixed letters and digits are spoken apart;
/// otherwise the word keeps at most about half its primary stresses.
pub fn resolve_tokens(parts: &mut [Token]) {
    let text = merged_text(parts);
    let classes: std::collections::HashSet<u8> = text
        .chars()
        .filter(|&c| !SUBTOKEN_JUNKS.contains(c))
        .map(|c| {
            if is_letter(c) {
                0
            } else if c.is_ascii_digit() {
                1
            } else {
                2
            }
        })
        .collect();
    let prespace = text.contains(' ') || text.contains('/') || classes.len() > 1;
    let n = parts.len();
    for (i, tk) in parts.iter_mut().enumerate() {
        if tk.phonemes.is_none() {
            let mut one = tk.text.chars();
            let single = one.next().filter(|_| one.next().is_none());
            if i + 1 == n && single.is_some_and(|c| NON_QUOTE_PUNCTS.contains(c)) {
                tk.phonemes = Some(tk.text.clone());
            } else if tk.text.chars().all(|c| SUBTOKEN_JUNKS.contains(c)) {
                tk.phonemes = Some(String::new());
            }
        } else if i > 0 {
            tk.prespace = prespace;
        }
    }
    if prespace {
        return;
    }
    let mut indices: Vec<(bool, usize, usize)> = parts
        .iter()
        .enumerate()
        .filter_map(|(i, tk)| {
            let ps = tk.phonemes.as_deref().filter(|p| !p.is_empty())?;
            Some((ps.contains(PRIMARY), stress_weight(ps), i))
        })
        .collect();
    let demote = |tk: &mut Token| {
        if let Some(ps) = &tk.phonemes {
            tk.phonemes = Some(apply_stress(ps, Some(-0.5)));
        }
    };
    if indices.len() == 2 && parts[indices[0].2].text.chars().count() == 1 {
        demote(&mut parts[indices[1].2]);
        return;
    }
    let primaries = indices.iter().filter(|x| x.0).count();
    if indices.len() < 2 || primaries <= indices.len().div_ceil(2) {
        return;
    }
    indices.sort();
    let half = indices.len() / 2;
    for &(_, _, i) in &indices[..half] {
        demote(&mut parts[i]);
    }
}
