//! misaki's `get_special_case` (words whose reading depends on their tag or
//! the next word: a/an/the/to/in/am/I/by/used, symbols, dotted acronyms)
//! and `get_NNP`, which spells a word out letter by letter.

use super::chars::{is_alpha, is_letter, upper};
use super::dict::{Dictionary, Entry};
use super::stress::{apply_stress, PRIMARY, SECONDARY};
use super::token::Ctx;
use super::word::{lookup, parent_tag, SYMBOLS};

/// The word's letters, each read as its capital's gold entry, the last
/// letter taking primary stress and the rest secondary (`GPU` is
/// `ʤˌipˌijˈu`). `None` when a letter has no single reading.
pub fn nnp(dict: &Dictionary, word: &str) -> Option<String> {
    let mut ps = String::new();
    for c in word.chars().filter(|&c| is_letter(c)) {
        match dict.gold(&upper(&c.to_string())) {
            Some(Entry::Word(letter)) => ps.push_str(letter),
            _ => return None,
        }
    }
    let ps = apply_stress(&ps, Some(0.0));
    Some(match ps.rfind(SECONDARY) {
        Some(i) => format!("{}{PRIMARY}{}", &ps[..i], &ps[i + SECONDARY.len_utf8()..]),
        None => ps,
    })
}

/// A dotted abbreviation of short parts (`U.S.A`, `e.g`, `Ph.D`), which is
/// spelt out.
fn dotted(word: &str) -> bool {
    word.trim_matches('.').contains('.')
        && is_alpha(&word.replace('.', ""))
        && word
            .split('.')
            .map(|p| p.chars().count())
            .max()
            .unwrap_or(0)
            < 3
}

pub fn special_case(
    dict: &Dictionary,
    word: &str,
    tag: &str,
    stress: Option<f64>,
    ctx: &Ctx,
) -> Option<String> {
    if let Some((_, name)) = SYMBOLS.iter().find(|(s, _)| *s == word) {
        return lookup(dict, name, "", None, Some(ctx));
    }
    if dotted(word) {
        return nnp(dict, word);
    }
    match word {
        "a" | "A" => {
            return Some(if tag == "DT" { "ɐ" } else { "ˈA" }.to_string());
        }
        "am" | "Am" | "AM" => {
            if tag.starts_with("NN") {
                return nnp(dict, word);
            }
            if ctx.future_vowel.is_none() || word != "am" || stress.is_some_and(|s| s > 0.0) {
                return dict.gold_word("am").map(str::to_string);
            }
            return Some("ɐm".to_string());
        }
        "an" | "An" | "AN" => {
            if word == "AN" && tag.starts_with("NN") {
                return nnp(dict, word);
            }
            return Some("ɐn".to_string());
        }
        "I" if tag == "PRP" => return Some(format!("{SECONDARY}I")),
        "by" | "By" | "BY" if parent_tag(tag) == "ADV" => return Some("bˈI".to_string()),
        _ => {}
    }
    if matches!(word, "to" | "To") || (word == "TO" && matches!(tag, "TO" | "IN")) {
        return match ctx.future_vowel {
            None => dict.gold_word("to").map(str::to_string),
            Some(false) => Some("tə".to_string()),
            Some(true) => Some("tʊ".to_string()),
        };
    }
    if matches!(word, "in" | "In") || (word == "IN" && tag != "NNP") {
        let mark = if ctx.future_vowel.is_none() || tag != "IN" {
            PRIMARY.to_string()
        } else {
            String::new()
        };
        return Some(format!("{mark}ɪn"));
    }
    if matches!(word, "the" | "The") || (word == "THE" && tag == "DT") {
        return Some(
            if ctx.future_vowel == Some(true) {
                "ði"
            } else {
                "ðə"
            }
            .to_string(),
        );
    }
    if tag == "IN" && matches!(word.to_lowercase().as_str(), "vs" | "vs.") {
        return lookup(dict, "versus", "", None, Some(ctx));
    }
    if matches!(word, "used" | "Used" | "USED") {
        let reading = if matches!(tag, "VBD" | "JJ") && ctx.future_to {
            "VBD"
        } else {
            "DEFAULT"
        };
        return match dict.gold("used") {
            Some(Entry::Tags(tags)) => tags.get(reading).cloned().flatten(),
            _ => None,
        };
    }
    None
}
