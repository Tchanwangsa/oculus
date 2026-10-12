use super::super::phrases;
use super::{pause_after, shown, spoken};

#[test]
fn a_short_sentence_is_one_phrase_on_one_line() {
    let w = spoken("We follow the gradient downhill.", 0.25, 0.3, &[]);
    let p = phrases(&w, &[], 42, 2);
    assert_eq!(shown(&w, &p), ["We follow the gradient downhill."]);
}

#[test]
fn sentence_ends_pauses_and_long_silences_always_break() {
    let w = spoken(
        "One step. Then another step and a pause then more",
        0.0,
        0.2,
        &[(6, 0.7)],
    );
    let p = phrases(&w, &[], 42, 2);
    assert_eq!(
        shown(&w, &p),
        ["One step.", "Then another step and a", "pause then more"]
    );
    let w = spoken("left right up down", 0.0, 0.2, &[]);
    let p = phrases(&w, &[pause_after(1, &w)], 42, 2);
    assert_eq!(shown(&w, &p), ["left right", "up down"]);
}

#[test]
fn a_long_sentence_breaks_at_a_clause_when_it_leaves_the_phrase_half_full() {
    let text = "When the step size is too large, the iterates overshoot the minimum and bounce from side to side forever";
    let w = spoken(text, 0.0, 0.2, &[]);
    let p = phrases(&w, &[], 20, 2);
    let s = shown(&w, &p);
    // A tie between 13/18 and 18/13 puts more on the later line.
    assert_eq!(s[0], "When the step / size is too large,");
    for phrase in &p {
        for line in &phrase.lines {
            assert!(super::super::phrase::chars(&w, line.clone()) <= 20, "{s:?}");
        }
    }
    // Every word is shown once, in order.
    let all: Vec<usize> = p.iter().flat_map(|p| p.words.clone()).collect();
    assert_eq!(all, (0..w.len()).collect::<Vec<_>>());
}

#[test]
fn no_phrase_leaves_one_word_alone_at_the_end() {
    // 41 characters fill one 42-character line, so "end" would be alone.
    let w = spoken(
        "aaaa bbbb cccc dddd eeee ffff gggg hhhh i end",
        0.0,
        0.2,
        &[],
    );
    let p = phrases(&w, &[], 42, 1);
    let s = shown(&w, &p);
    assert_eq!(s, ["aaaa bbbb cccc dddd eeee ffff gggg hhhh", "i end"]);
}

#[test]
fn lines_are_balanced() {
    let w = spoken(
        "the quick brown fox jumps over the lazy dog today",
        0.0,
        0.2,
        &[],
    );
    let p = phrases(&w, &[], 30, 2);
    assert_eq!(
        shown(&w, &p),
        ["the quick brown fox jumps / over the lazy dog today"]
    );
}

#[test]
fn a_word_longer_than_a_line_gets_its_own_phrase() {
    let w = spoken("see supercalifragilisticexpialidocious here", 0.0, 0.2, &[]);
    let p = phrases(&w, &[], 12, 1);
    assert_eq!(
        shown(&w, &p),
        ["see", "supercalifragilisticexpialidocious", "here"]
    );
}

#[test]
fn spoken_forms_keep_the_scripts_spelling() {
    let w = spoken("thirteen mod twelve is one.", 0.0, 0.2, &[]);
    let p = phrases(&w, &[], 42, 2);
    assert_eq!(shown(&w, &p), ["thirteen mod twelve is one."]);
}
