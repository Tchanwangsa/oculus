use std::collections::HashMap;

use super::super::dict::grow;
use super::super::{Dictionary, Entry};

fn word(s: &str) -> Entry {
    Entry::Word(s.to_string())
}

#[test]
fn growing_adds_the_other_case_and_the_file_wins() {
    let d: HashMap<String, Entry> = [
        ("hello", word("h")),
        ("Gradient", word("g")),
        ("Paris", word("p1")),
        ("paris", word("p2")),
        ("x", word("x")),
        ("NASA", word("n")),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    let g = grow(d);
    assert_eq!(g["Hello"], word("h"));
    assert_eq!(g["gradient"], word("g"));
    assert_eq!(g["Paris"], word("p1"));
    assert_eq!(g["paris"], word("p2"));
    assert!(!g.contains_key("X"));
    assert!(!g.contains_key("nasa"));
}

#[test]
fn heteronym_readings_keep_their_nulls() {
    let d = Dictionary::from_json(r#"{"AM":{"DEFAULT":"æm","NOUN":null}}"#, "{}").unwrap();
    match d.gold("AM").unwrap() {
        Entry::Tags(t) => {
            assert_eq!(t["NOUN"], None);
            assert_eq!(t["DEFAULT"].as_deref(), Some("æm"));
        }
        e => panic!("{e:?}"),
    }
}

#[test]
fn a_heteronym_without_default_is_refused() {
    let e = Dictionary::from_json(r#"{"read":{"VERB":"ɹˈid"}}"#, "{}")
        .err()
        .unwrap();
    assert!(e.contains("read"), "{e}");
}

#[test]
fn a_missing_file_is_named() {
    let e = Dictionary::load(std::path::Path::new("/nonexistent-g2p-pack"))
        .err()
        .unwrap();
    assert!(e.contains("us_gold.json"), "{e}");
}
