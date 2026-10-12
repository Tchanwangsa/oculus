use super::super::chunk::{chunks, chunks_under, MAX_TOKENS};
use super::super::vocab;
use crate::studio::voice::g2p::{Phonemized, Source, WordRecord};

/// `parts` joined by single spaces as the phoneme string; each part that
/// starts with a letter-like phoneme is a word named `w<i>`.
fn read(parts: &[&str]) -> Phonemized {
    let phonemes = parts.join(" ");
    let words = parts
        .iter()
        .enumerate()
        .map(|(i, p)| WordRecord {
            word: format!("w{i}"),
            phonemes: p
                .trim_end_matches([',', '.', '?', '!', ';', ':', '”'])
                .to_owned(),
            source: Source::Dict,
            guessed: Vec::new(),
        })
        .collect();
    Phonemized { phonemes, words }
}

fn ids(ps: &str) -> Vec<i64> {
    ps.chars().map(|c| vocab::id(c).unwrap()).collect()
}

#[test]
fn whole_sentences_are_packed_under_the_limit() {
    let p = read(&["hˈɛlO.", "ðˈɛɹ!", "wˈʌn", "mˈɔɹ?", "ˈɛnd."]);
    let all = chunks(&p).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].ids, ids("hˈɛlO. ðˈɛɹ! wˈʌn mˈɔɹ? ˈɛnd."));
    assert_eq!(all[0].words, "w0 w1 w2 w3 w4");

    let cut = chunks_under(&p, 14).unwrap();
    let words: Vec<&str> = cut.iter().map(|c| c.words.as_str()).collect();
    assert_eq!(words, ["w0 w1", "w2 w3", "w4"]);
    assert_eq!(cut[0].ids, ids("hˈɛlO. ðˈɛɹ!"));
    assert!(cut.iter().all(|c| c.ids.len() <= 14));
}

#[test]
fn a_sentence_too_long_alone_is_cut_at_commas() {
    let p = read(&["ˈɛnd.", "wˈʌn,", "tˈu;", "θɹˈi:", "fˈɔɹ."]);
    let cut = chunks_under(&p, 12).unwrap();
    let words: Vec<&str> = cut.iter().map(|c| c.words.as_str()).collect();
    assert_eq!(
        words,
        ["w0 w1", "w2 w3", "w4"],
        "clauses pack like sentences"
    );
    assert_eq!(cut[1].ids, ids("tˈu; θɹˈi:"));
}

#[test]
fn a_clause_too_long_is_refused_naming_its_sentence() {
    let p = read(&["ˈɛnd.", "wˈʌn", "tˈu", "θɹˈi", "fˈɔɹ."]);
    let e = chunks_under(&p, 12).unwrap_err();
    assert!(e.contains("\"w1 w2 w3 w4\""), "{e}");
    assert!(e.contains("split it"), "{e}");
}

#[test]
fn the_real_limit_is_509_tokens() {
    assert_eq!(MAX_TOKENS, 509);
    let sentence = format!("{}.", "ə".repeat(300));
    let p = read(&[&sentence, &sentence]);
    let cut = chunks(&p).unwrap();
    assert_eq!(cut.len(), 2);
    assert_eq!(cut[0].ids.len(), 301);
    let long = format!("{}.", "ə".repeat(MAX_TOKENS));
    assert!(chunks(&read(&[&long])).is_err());
    let fits = format!("{}.", "ə".repeat(MAX_TOKENS - 1));
    assert_eq!(chunks(&read(&[&fits])).unwrap()[0].ids.len(), MAX_TOKENS);
}

#[test]
fn a_closing_quote_after_the_stop_stays_with_its_sentence() {
    let p = read(&["“hˈɛlO.”", "ðˈɛɹ."]);
    let cut = chunks_under(&p, 9).unwrap();
    assert_eq!(cut.len(), 2);
    assert_eq!(cut[0].ids, ids("“hˈɛlO.”"));
}

#[test]
fn whitespace_runs_read_as_one_space() {
    let p = Phonemized {
        phonemes: "  hˈɛlO,\n\n ðˈɛɹ.\n".to_owned(),
        words: Vec::new(),
    };
    assert_eq!(chunks(&p).unwrap()[0].ids, ids("hˈɛlO, ðˈɛɹ."));
}

#[test]
fn an_unknown_phoneme_or_the_pad_is_refused_naming_the_word() {
    let p = read(&["hˈɛlO", "ðˈɝ."]);
    let e = chunks(&p).unwrap_err();
    assert!(e.contains("'ɝ'") && e.contains("\"w1\""), "{e}");
    let p = read(&["hˈɛlO", "ð$ɛɹ."]);
    let e = chunks(&p).unwrap_err();
    assert!(
        e.contains("`$`") && e.contains("pad") && e.contains("\"w1\""),
        "{e}"
    );
}

#[test]
fn no_words_is_nothing_to_say() {
    let p = Phonemized {
        phonemes: " \n".to_owned(),
        words: Vec::new(),
    };
    assert!(chunks(&p).unwrap_err().contains("nothing to say"));
}
