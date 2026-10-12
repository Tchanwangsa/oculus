use super::super::split::{retokenize, subtokenize, Word};
use super::super::tag::tokenize;

fn parts(word: &str) -> Vec<String> {
    subtokenize(word)
}

#[test]
fn camel_case_and_acronyms() {
    assert_eq!(parts("iPhone"), ["i", "Phone"]);
    assert_eq!(parts("JavaScript"), ["Java", "Script"]);
    assert_eq!(parts("PostgreSQL"), ["Postgre", "SQL"]);
    assert_eq!(parts("OpenAI"), ["Open", "AI"]);
    assert_eq!(parts("McDonald"), ["Mc", "Donald"]);
    assert_eq!(parts("GPUs"), ["GPUs"]);
}

#[test]
fn an_apostrophe_and_one_letter_stay_with_the_word() {
    assert_eq!(parts("O'Brien"), ["O'B", "rien"]);
    assert_eq!(parts("d'Alembert"), ["d'A", "lembert"]);
    assert_eq!(parts("don't"), ["don't"]);
    assert_eq!(parts("they're"), ["they'r", "e"]);
    assert_eq!(parts("L'Hôpital's"), ["L'H", "ôpital's"]);
}

#[test]
fn hyphens_dots_and_quotes() {
    assert_eq!(parts("well-known"), ["well", "-", "known"]);
    assert_eq!(parts("U.S.A"), ["U", ".", "S", ".", "A"]);
    assert_eq!(parts("'hello'"), ["'", "hello", "'"]);
    assert_eq!(parts("''x"), ["''", "x"]);
    assert_eq!(parts("snake_case"), ["snake", "_", "case"]);
}

#[test]
fn digits_group_with_separators() {
    assert_eq!(parts("-1,000.5"), ["-1,000.5"]);
    assert_eq!(parts("a2b"), ["a", "2", "b"]);
}

#[test]
fn punctuation_reads_as_itself_and_parts_stay_together() {
    let words = retokenize(tokenize("He said, \"x-ray\" (now) – yes…").tokens);
    let shape: Vec<String> = words
        .iter()
        .map(|w| match w {
            Word::One(t) => format!("{}={}", t.text, t.phonemes.clone().unwrap_or_default()),
            Word::Parts(p) => p
                .iter()
                .map(|t| t.text.as_str())
                .collect::<Vec<_>>()
                .join("|"),
        })
        .collect();
    assert_eq!(
        shape,
        [
            "He=", "said=", ",=,", "\"=“", "x|-|ray", "\"=”", "(=(", "now=", ")=)", "–=—", "yes=",
            "…=…"
        ]
    );
}
