use super::super::heteronym::{decide, Decision};

fn d(word: &str, l1: &str, l2: &str, r1: &str) -> Option<Decision> {
    decide(word, l1, l2, r1, &[], true)
}

#[test]
fn read_by_tense() {
    assert_eq!(d("read", "has", "she", "it"), Some(Decision::Tag("VBN")));
    assert_eq!(d("read", "will", "we", "it"), Some(Decision::Tag("VB")));
    assert_eq!(d("read", "me", "let", "it"), Some(Decision::Tag("VB")));
    let past = ["yesterday".to_string()];
    assert_eq!(
        decide("read", "i", "", "the", &past, false),
        Some(Decision::Tag("VBD"))
    );
    assert_eq!(d("read", "i", "", "books"), None);
}

#[test]
fn lead_the_metal_is_given_outright() {
    assert_eq!(d("lead", "", "", "pipe"), Some(Decision::Phonemes("lˈɛd")));
    assert_eq!(
        d("lead", "of", "made", ""),
        Some(Decision::Phonemes("lˈɛd"))
    );
    assert_eq!(d("lead", "we", "", "the"), None);
}

#[test]
fn the_named_words_and_the_generic_rule() {
    assert_eq!(d("live", "we", "", "in"), Some(Decision::Tag("VB")));
    assert_eq!(d("live", "the", "", "stream"), Some(Decision::Tag("JJ")));
    assert_eq!(d("content", "is", "she", ""), Some(Decision::Tag("JJ")));
    assert_eq!(d("content", "the", "", "of"), Some(Decision::Tag("NN")));
    assert_eq!(d("used", "we", "", "to"), Some(Decision::Tag("VBD")));
    assert_eq!(d("used", "was", "", "to"), None);
    assert_eq!(d("that", "know", "i", ""), Some(Decision::Tag("DT")));
    assert_eq!(d("close", "the", "", "of"), Some(Decision::Tag("NN")));
    assert_eq!(d("object", "we", "", "to"), Some(Decision::Tag("VB")));
    assert_eq!(d("object", "that", "", ""), Some(Decision::Tag("NN")));
    assert_eq!(d("present", "is", "it", ""), Some(Decision::Tag("JJ")));
    assert_eq!(decide("permit", "is", "", "", &[], false), None);
}
