//! The oracle's golden files against the trimmed dictionary.
//!
//! cargo test --lib studio::voice::g2p::tests::golden -- --nocapture

use super::super::Lexicon;
use super::fixtures::{check, golden_dir, trimmed_dictionary, ROWS};
use serde::Deserialize;

#[test]
fn every_golden_row_matches_the_oracle() {
    let (matched, failures) = check(&trimmed_dictionary());
    println!("rows matched whole: {matched}");
    assert!(failures.is_empty(), "{} rows differ", failures.len());
    assert_eq!(matched, ROWS);
}

#[test]
fn the_oracles_bad_lexicon_entries_are_refused() {
    #[derive(Deserialize)]
    struct Bad {
        key: String,
        value: String,
        error: String,
    }
    let dict = trimmed_dictionary();
    let text = std::fs::read_to_string(golden_dir().join("lexicon-errors.jsonl")).unwrap();
    let bad: Vec<Bad> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(bad.len(), 3);
    for b in bad {
        let json = serde_json::json!({ &b.key: b.value }).to_string();
        let e = Lexicon::from_json(&[("lexicon.json", &json)], &dict).unwrap_err();
        assert_eq!(e.kind(), b.error, "{}: {e}", b.key);
        assert!(e.to_string().contains(&format!("{:?}", b.key)), "{e}");
    }
}
