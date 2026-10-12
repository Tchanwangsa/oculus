//! The golden files against the full dictionary from a pack folder, which
//! proves the trimmed fixture answers every lookup the port makes. Ignored
//! by default:
//!
//! STUDIO_G2P_DATA=<dir with us_gold.json + us_silver.json> cargo test --lib studio::voice::g2p::tests::full -- --ignored --nocapture

use std::path::PathBuf;

use super::super::Dictionary;
use super::fixtures::{check, trimmed_dictionary};

#[test]
#[ignore]
fn full_dictionary_matches_the_trimmed_one() {
    let dir = PathBuf::from(
        std::env::var("STUDIO_G2P_DATA").expect("set STUDIO_G2P_DATA to run this test"),
    );
    let full = check(&Dictionary::load(&dir).unwrap());
    let trimmed = check(&trimmed_dictionary());
    assert!(
        full.failures.is_empty(),
        "{} rows differ",
        full.failures.len()
    );
    assert_eq!(full.whole, trimmed.whole);
    assert_eq!(full.dict_words, trimmed.dict_words);
    assert_eq!(full.deferred, trimmed.deferred);
}
