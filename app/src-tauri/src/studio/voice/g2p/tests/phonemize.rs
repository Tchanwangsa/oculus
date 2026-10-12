use super::super::{phonemize, Dictionary, Guess, Lexicon, Source};
use super::fixtures::trimmed_dictionary;

struct Fixed(&'static str);

impl Guess for Fixed {
    fn guess(&mut self, _part: &str) -> Result<String, String> {
        Ok(self.0.to_string())
    }
}

struct Broken;

impl Guess for Broken {
    fn guess(&mut self, part: &str) -> Result<String, String> {
        Err(format!("cannot guess {part}"))
    }
}

fn dict() -> Dictionary {
    trimmed_dictionary()
}

#[test]
fn a_word_nothing_reads_is_an_error_naming_it() {
    let e = phonemize("the zzqx cat", &dict(), &Lexicon::default(), &mut Fixed("")).unwrap_err();
    assert!(e.contains("\"zzqx\""), "{e}");
}

#[test]
fn a_guesser_error_stops_the_unit() {
    let e = phonemize("the zzqx cat", &dict(), &Lexicon::default(), &mut Broken).unwrap_err();
    assert!(e.contains("cannot guess zzqx"), "{e}");
}

#[test]
fn only_the_unknown_part_of_a_merged_word_is_guessed() {
    let got = phonemize(
        "zzqx-learn",
        &dict(),
        &Lexicon::default(),
        &mut Fixed("zˈIks"),
    )
    .unwrap();
    assert_eq!(got.words.len(), 1);
    assert_eq!(got.words[0].source, Source::Guess);
    assert_eq!(got.words[0].guessed, ["zzqx"]);
}

#[test]
fn empty_and_symbol_only_text_reads_as_nothing() {
    let p = phonemize("", &dict(), &Lexicon::default(), &mut Broken).unwrap();
    assert_eq!(p.phonemes, "");
    assert!(p.words.is_empty());
    let p = phonemize("* =", &dict(), &Lexicon::default(), &mut Broken).unwrap();
    assert_eq!(p.phonemes, " ");
}
