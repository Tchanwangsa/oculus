//! Every row of the oracle's `guesser.jsonl` (the 305 verify words and every
//! word or sub-token the oracle guessed) through the real model. Ignored by
//! default; run with a `g2p-en-us` pack folder:
//!
//! STUDIO_G2P_GUESSER=<dir> cargo test --lib studio::voice::g2p::guess::tests::golden -- --ignored
//!
//! STUDIO_G2P_GOLDEN=<guesser.jsonl> reads another golden file than the
//! fixture. The goldens came from ORT 1.31 and the app links 1.28, so a
//! differing word prints the logit margin at its first differing step.

use std::path::PathBuf;

use serde::Deserialize;

use super::super::vocab::{normalise, STOP};
use super::super::{Guess, Guesser};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/studio/voice/g2p/tests/golden/guesser.jsonl"
);

#[derive(Deserialize)]
struct Row {
    word: String,
    normalised: String,
    input_ids: Vec<i64>,
    phonemes: String,
}

#[test]
#[ignore = "needs STUDIO_G2P_GUESSER=<g2p-en-us pack folder>"]
fn every_golden_word_decodes_as_the_oracle_did() {
    let dir = std::env::var("STUDIO_G2P_GUESSER").expect("set STUDIO_G2P_GUESSER to run this test");
    let golden = std::env::var_os("STUDIO_G2P_GOLDEN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(FIXTURE));
    let text =
        std::fs::read_to_string(&golden).unwrap_or_else(|e| panic!("{}: {e}", golden.display()));
    let mut guesser = Guesser::load(std::path::Path::new(&dir)).unwrap();
    let rows: Vec<Row> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let mut differ = Vec::new();
    let mut tightest = f32::INFINITY;
    for row in &rows {
        assert_eq!(normalise(&row.word), row.normalised, "{}", row.word);
        let ids = guesser.vocab.encode(&row.normalised);
        assert_eq!(ids, row.input_ids, "{}", row.word);
        let ours = guesser.guess(&row.word).unwrap();
        let steps = guesser.decode(&ids).unwrap();
        tightest = steps.iter().map(|s| s.margin).fold(tightest, f32::min);
        if ours != row.phonemes {
            // One phoneme per step after the start id, so the first
            // differing character is the first differing step.
            let at = 1 + ours
                .chars()
                .zip(row.phonemes.chars())
                .take_while(|(a, b)| a == b)
                .count();
            let step = steps[at.min(steps.len() - 1)];
            differ.push(format!(
                "{}: ours {ours:?}, oracle {:?}; step {at} took id {}{} by a margin of {}",
                row.word,
                row.phonemes,
                step.id,
                if step.id == STOP { " (stop)" } else { "" },
                step.margin
            ));
        }
    }
    println!(
        "{} rows, {} differ; tightest top-2 margin {tightest}",
        rows.len(),
        differ.len()
    );
    assert!(differ.is_empty(), "{}", differ.join("\n"));
}
