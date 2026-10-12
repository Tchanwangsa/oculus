//! One word (or merged sub-tokens) to phonemes through the dictionary:
//! misaki's `Lexicon.__call__`, `get_word`, `is_known` and `lookup`, plus
//! two rules for when misaki finds nothing: an `-izer` agent noun read as
//! its `-ize` verb + `əɹ` ("optimizers"), and the British `-ise` family
//! read as `-ize` ("renormalise", "factorisation", "optimisers"). Numbers and currency are not ported:
//! narration reaches here in spoken form.

use std::sync::LazyLock;

use regex::Regex;
use unicode_normalization::UnicodeNormalization;

use super::chars::{char_len, has_letter, is_alpha, lower, tail, upper};
use super::dict::{Dictionary, Entry};
use super::special::{nnp, special_case};
use super::stem::{plural, stem_ed, stem_ing, stem_s};
use super::stress::{apply_stress, PRIMARY};
use super::token::Ctx;

/// What misaki reads as a word on its own: `&` is "and", and so on.
pub const SYMBOLS: [(&str, &str); 4] = [("%", "percent"), ("&", "and"), ("+", "plus"), ("@", "at")];

/// `-ise` endings with a `-ize` twin in the dictionary.
static ISE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"is(?:e|es|ed|ing|ation|ations|er|ers|able|ability)$").unwrap());

/// `-izer` agent nouns: 177 of the dictionary's 181 `-izer` entries that
/// have an `-ize` entry read as it + `əɹ`.
static IZER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"izers?$").unwrap());

/// A word's tag as misaki's dictionary keys readings: `VB*` reads `VERB`,
/// `NN*` `NOUN`, `RB*` `ADV`, `JJ*` `ADJ`.
pub fn parent_tag(tag: &str) -> &str {
    if tag.starts_with("VB") {
        "VERB"
    } else if tag.starts_with("NN") {
        "NOUN"
    } else if tag.starts_with("ADV") || tag.starts_with("RB") {
        "ADV"
    } else if tag.starts_with("ADJ") || tag.starts_with("JJ") {
        "ADJ"
    } else {
        tag
    }
}

/// misaki's `Lexicon.__call__`: curly apostrophes straightened, NFKC, and a
/// capitalised word looked up with secondary stress (all caps: primary).
pub fn word_phonemes(dict: &Dictionary, text: &str, tag: &str, ctx: &Ctx) -> Option<String> {
    let word: String = text.replace(['‘', '’'], "'").nfkc().collect();
    let stress = if word == lower(&word) {
        None
    } else if word == upper(&word) {
        Some(2.0)
    } else {
        Some(0.5)
    };
    get_word(dict, &word, tag, stress, ctx)
}

/// misaki's `get_word`, then the `-izer` rule, then both again with an
/// `-ise` ending read as `-ize`.
fn get_word(
    dict: &Dictionary,
    word: &str,
    tag: &str,
    stress: Option<f64>,
    ctx: &Ctx,
) -> Option<String> {
    if let Some(ps) = misaki_get_word(dict, word, tag, stress, ctx)
        .or_else(|| stem_izer(dict, word, tag, stress, ctx))
    {
        return Some(ps);
    }
    let m = ISE.find(word)?;
    let alt = format!("{}iz{}", &word[..m.start()], &word[m.start() + 2..]);
    misaki_get_word(dict, &alt, tag, stress, ctx)
        .or_else(|| stem_izer(dict, &alt, tag, stress, ctx))
}

/// `-izer(s)` as the `-ize` verb + `əɹ` (+ the plural); the verb in lower
/// case when only that is known ("Tokenizers").
fn stem_izer(
    dict: &Dictionary,
    word: &str,
    tag: &str,
    stress: Option<f64>,
    ctx: &Ctx,
) -> Option<String> {
    let m = IZER.find(word)?;
    let mut stem = format!("{}ize", &word[..m.start()]);
    if !is_known(dict, &stem) && is_known(dict, &lower(&stem)) {
        stem = lower(&stem);
    }
    if !is_known(dict, &stem) {
        return None;
    }
    let ps = lookup(dict, &stem, tag, stress, Some(ctx))? + "əɹ";
    if word.ends_with('s') {
        plural(&ps)
    } else {
        Some(ps)
    }
}

fn misaki_get_word(
    dict: &Dictionary,
    word: &str,
    tag: &str,
    stress: Option<f64>,
    ctx: &Ctx,
) -> Option<String> {
    if let Some(ps) = special_case(dict, word, tag, stress, ctx) {
        return Some(ps);
    }
    let wl = lower(word);
    let found = |p: Option<String>| p.is_some_and(|p| !p.is_empty());
    let word = if char_len(word) > 1
        && is_alpha(&word.replace('\'', ""))
        && word != wl
        && (tag != "NNP" || char_len(word) > 7)
        && !dict.in_gold(word)
        && !dict.in_silver(word)
        && (word == upper(word) || tail(word) == lower(tail(word)))
        && (dict.in_gold(&wl)
            || dict.in_silver(&wl)
            || found(stem_s(dict, &wl, tag, stress, ctx))
            || found(stem_ed(dict, &wl, tag, stress, ctx))
            || found(stem_ing(dict, &wl, tag, stress, ctx)))
    {
        wl.as_str()
    } else {
        word
    };
    if is_known(dict, word) {
        return lookup(dict, word, tag, stress, Some(ctx));
    }
    if let Some(stem) = word.strip_suffix("s'") {
        let possessive = format!("{stem}'s");
        if is_known(dict, &possessive) {
            return lookup(dict, &possessive, tag, stress, Some(ctx));
        }
    }
    if let Some(stem) = word.strip_suffix('\'') {
        if is_known(dict, stem) {
            return lookup(dict, stem, tag, stress, Some(ctx));
        }
    }
    if let Some(ps) = stem_s(dict, word, tag, stress, ctx) {
        return Some(ps);
    }
    if let Some(ps) = stem_ed(dict, word, tag, stress, ctx) {
        return Some(ps);
    }
    stem_ing(dict, word, tag, stress.or(Some(0.5)), ctx)
}

/// misaki's `is_known`: in the dictionary, a symbol, a single ASCII letter,
/// or a word whose letters after the first are all capitals (an acronym
/// spelt out by `lookup`).
pub fn is_known(dict: &Dictionary, word: &str) -> bool {
    if dict.in_gold(word) || SYMBOLS.iter().any(|(s, _)| *s == word) || dict.in_silver(word) {
        return true;
    }
    if !is_alpha(word)
        || !word
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c == '\'' || c == '-')
    {
        return false;
    }
    if char_len(word) == 1 {
        return true;
    }
    if word == upper(word) && dict.in_gold(&lower(word)) {
        return true;
    }
    tail(word) == upper(tail(word))
}

/// misaki's `lookup`: gold then silver (gold only for an all-caps proper
/// noun), a heteronym's reading by tag, letters spelt out when there is no
/// reading, then `stress` applied.
pub fn lookup(
    dict: &Dictionary,
    word: &str,
    tag: &str,
    stress: Option<f64>,
    ctx: Option<&Ctx>,
) -> Option<String> {
    let mut word = word.to_string();
    let mut is_nnp = false;
    if word == upper(&word) && !dict.in_gold(&word) {
        word = lower(&word);
        is_nnp = tag == "NNP";
    }
    let mut entry = dict.gold(&word);
    if entry.is_none() && !is_nnp {
        entry = dict.silver(&word);
    }
    let ps = match entry {
        None => None,
        Some(Entry::Word(ps)) => Some(ps.clone()),
        Some(Entry::Tags(tags)) => {
            let key = if ctx.is_some_and(|c| c.future_vowel.is_none()) && tags.contains_key("None")
            {
                "None"
            } else if tags.contains_key(tag) {
                tag
            } else {
                parent_tag(tag)
            };
            match tags.get(key) {
                Some(reading) => reading.clone(),
                None => tags.get("DEFAULT").cloned().flatten(),
            }
        }
    };
    if ps.is_none() || (is_nnp && !ps.as_deref().is_some_and(|p| p.contains(PRIMARY))) {
        if let Some(spelt) = nnp(dict, &word) {
            return Some(spelt);
        }
    }
    ps.map(|p| apply_stress(&p, stress))
}

/// Whether the guesser may be asked: only something with a letter in it.
pub fn guessable(text: &str) -> bool {
    has_letter(text)
}
