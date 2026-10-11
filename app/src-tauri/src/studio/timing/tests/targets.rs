use super::super::targets::{label, targets, Targets, LETTERS};
use super::words;

#[test]
fn script_words_are_upper_cased_letters_and_apostrophes() {
    let script = words(&[
        "You're",
        "—",
        "fog...",
        "well-known",
        "café",
        "42",
        "&",
        "Ünder",
    ]);
    assert_eq!(
        targets(&script),
        Targets {
            words: words(&["YOU'RE", "FOG", "WELLKNOWN", "CAF", "NDER"]),
            index: vec![0, 2, 3, 4, 7],
        }
    );
}

#[test]
fn a_script_of_symbols_has_no_targets() {
    assert!(targets(&words(&["—", "42", "%"])).words.is_empty());
}

#[test]
fn the_vocabulary_is_wav2vec2s() {
    assert_eq!(label('E'), Some(5));
    assert_eq!(label('\''), Some(27));
    assert_eq!(label('Z'), Some(31));
    // The separator and the specials are never spelled from text.
    assert_eq!(label('|'), None);
    assert_eq!(label('?'), None);
    assert_eq!(label('e'), None);
    assert_eq!(
        (b'A'..=b'Z')
            .filter(|&c| label(c as char).is_some())
            .count(),
        26
    );
    assert_eq!(LETTERS[4], b'|');
}
