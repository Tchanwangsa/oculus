//! The table against Kokoro's own `tokenizer.json` (a copy pinned by sha256
//! in `data/evals/studio-g2p/golden/MANIFEST.json`).

use std::collections::BTreeMap;

use super::super::vocab::{contains, id, VOCAB};

#[test]
fn the_table_is_kokoros_tokenizer_vocab() {
    let json: serde_json::Value =
        serde_json::from_str(include_str!("kokoro_tokenizer.json")).unwrap();
    let want: BTreeMap<String, i64> = json["model"]["vocab"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_i64().unwrap()))
        .collect();
    let got: BTreeMap<String, i64> = VOCAB.iter().map(|&(c, i)| (c.to_string(), i)).collect();
    assert_eq!(got.len(), VOCAB.len(), "a symbol is listed twice");
    assert_eq!(got, want);
}

#[test]
fn lookups() {
    assert_eq!(id('$'), Some(0));
    assert_eq!(id('ˈ'), Some(156));
    assert_eq!(id('ᵻ'), Some(177));
    assert!(contains(' '));
    assert!(!contains('☃'));
    assert!(!contains('ɝ'));
}
