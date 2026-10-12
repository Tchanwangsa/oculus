use super::super::vocab::{normalise, Vocab, START, STOP, UNKNOWN};
use super::CONFIG;

fn vocab() -> Vocab {
    Vocab::parse(CONFIG).unwrap()
}

#[test]
fn a_word_is_framed_by_start_and_stop() {
    // "marabout" from golden/guesser.jsonl.
    assert_eq!(
        vocab().encode("marabout"),
        [START, 17, 50, 56, 50, 9, 54, 22, 21, STOP]
    );
    assert_eq!(vocab().encode(""), [START, STOP]);
}

#[test]
fn case_is_kept() {
    let v = vocab();
    assert_ne!(v.encode("Micawber")[1], v.encode("micawber")[1]);
    assert_eq!(v.encode("Micawber")[1], 39);
}

#[test]
fn accents_are_stripped_by_nfkd() {
    assert_eq!(normalise("Gödel"), "Godel");
    assert_eq!(normalise("Erdős"), "Erdos");
    assert_eq!(normalise("Poincaré"), "Poincare");
    assert_eq!(normalise("naïve"), "naive");
    // Already decomposed: the combining mark is dropped all the same.
    assert_eq!(normalise("cafe\u{301}"), "cafe");
}

#[test]
fn a_curly_apostrophe_becomes_straight() {
    assert_eq!(normalise("ôpital\u{2019}s"), "opital's");
    let v = vocab();
    assert_eq!(v.encode(&normalise("O\u{2019}Brien")), v.encode("O'Brien"));
    assert_ne!(v.encode("O'Brien")[2], UNKNOWN);
}

#[test]
fn a_character_outside_the_alphabet_is_unknown() {
    let v = vocab();
    // NFKD turns the superscript into a plain digit; digits are not graphemes.
    assert_eq!(normalise("x²"), "x2");
    assert_eq!(v.encode("x2")[2], UNKNOWN);
    // `_` fills the reserved slots in the config but is not a grapheme.
    assert_eq!(v.encode("a_b")[2], UNKNOWN);
    assert_eq!(v.encode("ß")[1], UNKNOWN);
}

#[test]
fn reserved_and_out_of_range_ids_are_not_phonemes() {
    let v = vocab();
    for id in [-1, 0, 1, 2, 3, 50, 62] {
        assert_eq!(v.phoneme(id), None, "{id}");
    }
    assert_eq!(v.phoneme(4), Some('A'));
    assert_eq!(v.phoneme(49), Some('ᵻ'));
}

#[test]
fn a_config_with_too_long_an_alphabet_is_refused() {
    let json = r#"{"grapheme_chars": "____abc", "phoneme_chars": "____abcdef", "vocab_size": 8}"#;
    assert!(Vocab::parse(json).unwrap_err().contains("phoneme_chars"));
}
