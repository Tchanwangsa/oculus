//! Inflections read from their stem: misaki's `stem_s`/`_s`,
//! `stem_ed`/`_ed`, `stem_ing`/`_ing`, plus `-ied` read from `-y`
//! ("multiplied" from "multiply"), which misaki lacks. Words are sliced by
//! character and phonemes inspected by character: both hold non-ASCII.

use super::chars::{char_len, drop_last, nth_last};
use super::dict::Dictionary;
use super::token::Ctx;
use super::word::{is_known, lookup};

/// Vowels (and `ɹ`) after which a final `t` flaps before `-ed`/`-ing`.
const US_TAUS: &str = "AIOWYiuæɑəɛɪɹʊʌ";

/// `-s`: `s` after a voiceless stop or fricative, `ᵻz` after a sibilant,
/// else `z`.
pub fn plural(stem: &str) -> Option<String> {
    let last = nth_last(stem, 1)?;
    Some(if "ptkfθ".contains(last) {
        format!("{stem}s")
    } else if "szʃʒʧʤ".contains(last) {
        format!("{stem}ᵻz")
    } else {
        format!("{stem}z")
    })
}

/// `-ed`: `t` after a voiceless consonant, `ᵻd` after `d`, `d` after
/// anything but `t`; after `t`, a vowel before it flaps (`ɾᵻd`).
pub fn past(stem: &str) -> Option<String> {
    let last = nth_last(stem, 1)?;
    Some(if "pkfθʃsʧ".contains(last) {
        format!("{stem}t")
    } else if last == 'd' {
        format!("{stem}ᵻd")
    } else if last != 't' {
        format!("{stem}d")
    } else if char_len(stem) < 2 {
        format!("{stem}ɪd")
    } else if nth_last(stem, 2).is_some_and(|c| US_TAUS.contains(c)) {
        format!("{}ɾᵻd", drop_last(stem, 1))
    } else {
        format!("{stem}ᵻd")
    })
}

/// `-ing`, with a final `t` after a vowel flapped.
pub fn progressive(stem: &str) -> Option<String> {
    nth_last(stem, 1)?;
    Some(
        if char_len(stem) > 1
            && nth_last(stem, 1) == Some('t')
            && nth_last(stem, 2).is_some_and(|c| US_TAUS.contains(c))
        {
            format!("{}ɾɪŋ", drop_last(stem, 1))
        } else {
            format!("{stem}ɪŋ")
        },
    )
}

pub fn stem_s(
    dict: &Dictionary,
    word: &str,
    tag: &str,
    stress: Option<f64>,
    ctx: &Ctx,
) -> Option<String> {
    let n = char_len(word);
    if n < 3 || !word.ends_with('s') {
        return None;
    }
    let stem = if !word.ends_with("ss") && is_known(dict, drop_last(word, 1)) {
        drop_last(word, 1).to_string()
    } else if (word.ends_with("'s") || (n > 4 && word.ends_with("es") && !word.ends_with("ies")))
        && is_known(dict, drop_last(word, 2))
    {
        drop_last(word, 2).to_string()
    } else if n > 4 && word.ends_with("ies") && is_known(dict, &format!("{}y", drop_last(word, 3)))
    {
        format!("{}y", drop_last(word, 3))
    } else {
        return None;
    };
    plural(&lookup(dict, &stem, tag, stress, Some(ctx))?)
}

/// misaki's `stem_ed`, then `-ied` from `-y`.
pub fn stem_ed(
    dict: &Dictionary,
    word: &str,
    tag: &str,
    stress: Option<f64>,
    ctx: &Ctx,
) -> Option<String> {
    if let Some(ps) = misaki_stem_ed(dict, word, tag, stress, ctx) {
        return Some(ps);
    }
    let y = format!("{}y", drop_last(word, 3));
    if char_len(word) > 4 && word.ends_with("ied") && is_known(dict, &y) {
        return past(&lookup(dict, &y, tag, stress, Some(ctx))?);
    }
    None
}

fn misaki_stem_ed(
    dict: &Dictionary,
    word: &str,
    tag: &str,
    stress: Option<f64>,
    ctx: &Ctx,
) -> Option<String> {
    let n = char_len(word);
    if n < 4 || !word.ends_with('d') {
        return None;
    }
    let stem = if !word.ends_with("dd") && is_known(dict, drop_last(word, 1)) {
        drop_last(word, 1)
    } else if n > 4
        && word.ends_with("ed")
        && !word.ends_with("eed")
        && is_known(dict, drop_last(word, 2))
    {
        drop_last(word, 2)
    } else {
        return None;
    };
    past(&lookup(dict, stem, tag, stress, Some(ctx))?)
}

/// A doubled consonant before `-ing` ("running", "bidding"), or `-cking`.
fn doubled_ing(word: &str) -> bool {
    if word.ends_with("cking") {
        return true;
    }
    let Some(rest) = word.strip_suffix("ing") else {
        return false;
    };
    match (nth_last(rest, 2), nth_last(rest, 1)) {
        (Some(a), Some(b)) => a == b && "bcdgklmnprstvxz".contains(a),
        _ => false,
    }
}

pub fn stem_ing(
    dict: &Dictionary,
    word: &str,
    tag: &str,
    stress: Option<f64>,
    ctx: &Ctx,
) -> Option<String> {
    let n = char_len(word);
    if n < 5 || !word.ends_with("ing") {
        return None;
    }
    let base = drop_last(word, 3);
    let stem = if n > 5 && is_known(dict, base) {
        base.to_string()
    } else if is_known(dict, &format!("{base}e")) {
        format!("{base}e")
    } else if n > 5 && doubled_ing(word) && is_known(dict, drop_last(word, 4)) {
        drop_last(word, 4).to_string()
    } else {
        return None;
    };
    progressive(&lookup(dict, &stem, tag, stress, Some(ctx))?)
}
