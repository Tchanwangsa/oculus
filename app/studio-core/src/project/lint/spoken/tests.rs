use super::*;

fn tokens(text: &str) -> Vec<String> {
    scan(text).into_iter().map(|u| u.token).collect()
}

#[test]
fn plain_prose_and_punctuation_pass() {
    let ok = "We follow the *gradient* downhill — one (small) step: \"slowly\", isn't it? Yes; well-known… \u{2018}fine\u{2019}.";
    assert_eq!(scan(ok), vec![]);
}

#[test]
fn digits_symbols_and_greek_are_flagged() {
    let found = scan("a rate of 0.01, about 50% of x^2\nso a * b = c, and \u{3c0} or \u{3be}");
    let t: Vec<&str> = found.iter().map(|u| u.token.as_str()).collect();
    assert_eq!(t, ["0.01", "50%", "x^2", "*", "=", "\u{3c0}", "\u{3be}"]);
    assert!(found[0].hints[0].contains("`zero point zero one`, not `0.01`"));
    assert_eq!(found[1].hints.len(), 2, "{:?}", found[1].hints);
    assert!(found[1].hints[1].contains("say `%` as `percent`"));
    assert!(found[2].hints.iter().any(|h| h.contains("to the power of")));
    assert_eq!(found[3].line, 1);
    assert!(found[3].hints[0].contains("`times`"));
    assert!(found[5].hints[0].contains("`pi`"));
    assert!(found[6].hints[0].contains("by the letter's name"));
}

#[test]
fn dotted_abbreviations_are_flagged() {
    let found = scan("Many, e.g. this, i.e., that, etc. And the U.S. too.");
    let t: Vec<&str> = found.iter().map(|u| u.token.as_str()).collect();
    assert_eq!(t, ["e.g.", "i.e.", "etc.", "U.S."]);
    assert!(found[0].hints[0].contains("write `for example`, not `e.g.`"));
    assert!(found[3].hints[0].contains("out as it is said"));
}

#[test]
fn each_token_is_reported_once() {
    assert_eq!(tokens("2 and 2 make 4"), ["2", "4"]);
}
