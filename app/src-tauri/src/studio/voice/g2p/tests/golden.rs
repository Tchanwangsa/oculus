//! The oracle's golden files against the trimmed dictionary.
//!
//! cargo test --lib studio::voice::g2p::tests::golden -- --nocapture

use super::fixtures::{check, trimmed_dictionary};

#[test]
fn golden_rows_match_the_oracle() {
    let report = check(&trimmed_dictionary());
    println!(
        "whole rows matched: {}; rows left to the heteronym table or lexicon: {} \
         ({} dictionary words in them matched, {} before a lexicon or guessed word not compared); \
         skipped: {}",
        report.whole,
        report.deferred.len() + report.skipped.len(),
        report.dict_words,
        report.before_lexicon,
        report.skipped.len()
    );
    for row in &report.deferred {
        println!("  deferred {row}");
    }
    for row in &report.skipped {
        println!("  skipped {row}");
    }
    assert!(
        report.failures.is_empty(),
        "{} rows differ",
        report.failures.len()
    );
}
