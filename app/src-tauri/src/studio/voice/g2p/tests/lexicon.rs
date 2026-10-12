use std::path::Path;

use super::super::{Dictionary, Lexicon};

fn dict() -> Dictionary {
    Dictionary::from_json(
        r#"{"soft":"sˈɔft","max":"mˈæks","deep":"dˈip","mind":"mˈInd"}"#,
        "{}",
    )
    .unwrap()
}

fn lex(layers: &[&str]) -> Lexicon {
    let layers: Vec<(&str, &str)> = layers.iter().map(|t| ("lexicon.json", *t)).collect();
    Lexicon::from_json(&layers, &dict()).unwrap()
}

#[test]
fn raw_phonemes_and_respellings() {
    let l = lex(&[
        r#"{"_about":"comment","Cauchy":"/kˈOʃi/","softmax":"SOFT-max","DeepMind":"DEEP mind"}"#,
    ]);
    assert_eq!(l.phonemes("Cauchy").unwrap(), "kˈOʃi");
    assert_eq!(l.phonemes("softmax").unwrap(), "sˈɔftmˌæks");
    assert_eq!(l.phonemes("DeepMind").unwrap(), "dˈipmˌInd");
    assert_eq!(l.phonemes("_about"), None);
}

#[test]
fn exact_then_lower_then_possessive_then_any_case() {
    let l = lex(&[r#"{"Euler":"/ˈYləɹ/","nyquist":"/nˈIkwɪst/"}"#]);
    assert_eq!(l.phonemes("Euler").unwrap(), "ˈYləɹ");
    assert_eq!(l.phonemes("Nyquist").unwrap(), "nˈIkwɪst");
    assert_eq!(l.phonemes("Euler's").unwrap(), "ˈYləɹz");
    assert_eq!(l.phonemes("EULER").unwrap(), "ˈYləɹ");
    assert_eq!(l.phonemes("Gauss"), None);
}

#[test]
fn the_subject_layer_overrides_the_library() {
    let l = lex(&[
        r#"{"Hadamard":"/hˈædəmˌɑɹd/","Euler":"/ˈYləɹ/"}"#,
        r#"{"Hadamard":"/hɑdəmˈɑɹd/"}"#,
    ]);
    assert_eq!(l.phonemes("Hadamard").unwrap(), "hɑdəmˈɑɹd");
    assert_eq!(l.phonemes("Euler").unwrap(), "ˈYləɹ");
}

#[test]
fn a_missing_file_is_an_empty_layer_and_a_bad_one_is_named() {
    let d = dict();
    let none = Path::new("/nonexistent-studio-lexicon.json");
    assert!(Lexicon::load(Some(none), None, &d).unwrap().is_empty());
    let e = Lexicon::from_json(&[("sub/lexicon.json", "[1]")], &d).unwrap_err();
    assert_eq!(e.kind(), "malformed");
    assert!(e.to_string().contains("sub/lexicon.json"), "{e}");
    let e = Lexicon::from_json(&[("lexicon.json", r#"{"Foo": 3}"#)], &d).unwrap_err();
    assert!(e.to_string().contains("\"Foo\""), "{e}");
}

#[test]
fn the_pad_is_a_bad_phoneme() {
    let e = Lexicon::from_json(&[("lexicon.json", r#"{"Cash":"/kˈæ$/"}"#)], &dict()).unwrap_err();
    assert_eq!(e.kind(), "bad-phoneme");
    assert!(e.to_string().contains("'$'"), "{e}");
}

#[test]
fn a_curly_apostrophe_reads_as_a_straight_one() {
    let l = lex(&[r#"{"Euler":"/ˈYləɹ/","Bayes’":"/bˈAz/"}"#]);
    assert_eq!(l.phonemes("Euler’s").unwrap(), "ˈYləɹz");
    assert_eq!(l.phonemes("Bayes'").unwrap(), "bˈAz");
    assert_eq!(l.phonemes("Bayes’").unwrap(), "bˈAz");
}
