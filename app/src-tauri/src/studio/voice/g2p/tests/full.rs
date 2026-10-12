//! The golden files against the full dictionary from a pack folder, which
//! proves the trimmed fixture answers every lookup the port makes. Ignored
//! by default:
//!
//! STUDIO_G2P_DATA=<dir with us_gold.json + us_silver.json> cargo test --lib studio::voice::g2p::tests::full -- --ignored --nocapture

use std::path::PathBuf;

use super::super::Dictionary;
use super::fixtures::check;

#[test]
#[ignore]
fn the_full_dictionary_matches_the_oracle() {
    let dir = PathBuf::from(
        std::env::var("STUDIO_G2P_DATA").expect("set STUDIO_G2P_DATA to run this test"),
    );
    let (matched, failures) = check(&Dictionary::load(&dir).unwrap());
    assert!(failures.is_empty(), "{} rows differ", failures.len());
    assert_eq!(matched, 1026);
}
