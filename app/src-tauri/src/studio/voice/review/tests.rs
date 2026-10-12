use std::path::Path;

use serde_json::json;
use studio_core::{Library, Project};

use super::*;
use crate::studio::voice::g2p::{Phonemized, Source, WordRecord};
use crate::test_support::Scratch;

fn dict() -> Dictionary {
    Dictionary::from_json(
        r#"{"ness":"nˈɛs","ter":"tˈɜɹ","off":"ˈɔf","soft":"sˈɔft","max":"mˈæks"}"#,
        "{}",
    )
    .unwrap()
}

fn store(dir: &Path) -> Store {
    Store {
        root: dir.join("pronunciations"),
        library: dir.join("library"),
        subjects: dir.join("subjects"),
    }
}

fn located(word: &str) -> Located {
    Located {
        word: word.into(),
        original: "nˈɛstəɹɔv".into(),
        guessed: true,
        sentence: format!("We meet {word} here."),
        scene: "intro".into(),
        line: 2,
    }
}

fn suggestion(word: &str, reading: &str, subject: Option<&str>) -> Suggestion {
    Suggestion {
        reading: reading.into(),
        subject: subject.map(str::to_owned),
        video: "momentum".into(),
        located: located(word),
    }
}

#[test]
fn a_key_is_file_safe_and_one_per_word() {
    assert_eq!(key::key("Nesterov"), "nesterov");
    assert_eq!(key::key("don’t"), key::key("don't"));
    assert_eq!(key::key("don't"), "don_27_t");
    assert_ne!(key::key("a_b"), key::key("a b"));
    assert_eq!(key::id(None, "Euler").unwrap(), "_library/euler");
    assert_eq!(
        key::id(Some("COMP30027"), "Euler").unwrap(),
        "COMP30027/euler"
    );
    assert!(key::id(Some("../x"), "Euler").is_err());
    assert!(key::parse("COMP30027/euler").is_ok());
    for bad in [
        "euler",
        "../x/euler",
        "a/../b",
        "a/b/c",
        "a/Euler",
        "/euler",
    ] {
        assert!(key::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn a_suggestion_is_one_file_and_a_new_one_replaces_it() {
    let dir = Scratch::new("review-suggest");
    let s = store(&dir);
    let first = s
        .suggest(suggestion("Nesterov", "NESS-ter-off", Some("COMP30027")))
        .unwrap();
    assert_eq!(first.id, "COMP30027/nesterov");
    let file = dir.join("pronunciations/COMP30027/nesterov.json");
    let on_disk: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(on_disk["word"], "Nesterov");
    assert_eq!(on_disk["status"], "pending");
    assert_eq!(on_disk["note"], serde_json::Value::Null);
    assert_eq!(on_disk["video"], "momentum");

    s.mark_wrong(&first.id, Some("  stress the second syllable ".into()))
        .unwrap();
    let wrong = s.get(&first.id).unwrap().record;
    assert_eq!(wrong.status, Status::Wrong);
    assert_eq!(wrong.note.as_deref(), Some("stress the second syllable"));

    let again = s
        .suggest(suggestion("nesterov", "ness-TER-off", Some("COMP30027")))
        .unwrap();
    assert_eq!(again.id, first.id);
    let all = s.list(|_| true).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].record.status, Status::Pending);
    assert_eq!(all[0].record.note, None);
    assert_eq!(all[0].record.reading, "ness-TER-off");
}

#[test]
fn the_list_is_newest_first_filters_and_skips_a_broken_file() {
    let dir = Scratch::new("review-list");
    let s = store(&dir);
    s.suggest(suggestion("Euler", "/ˈYləɹ/", None)).unwrap();
    s.suggest(suggestion("Cauchy", "/kˈOʃi/", Some("MAST20004")))
        .unwrap();
    let mut older = s.get("_library/euler").unwrap().record;
    older.created = "2000-01-01T00:00:00".into();
    atomic_write::json(&dir.join("pronunciations/_library/euler.json"), &older).unwrap();
    std::fs::write(dir.join("pronunciations/_library/broken.json"), "{").unwrap();
    std::fs::write(dir.join("pronunciations/_library/notes.txt"), "x").unwrap();

    let ids: Vec<String> = s
        .list(|_| true)
        .unwrap()
        .into_iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(ids, ["MAST20004/cauchy", "_library/euler"]);
    let only = s
        .list(|r| r.subject.as_deref() == Some("MAST20004"))
        .unwrap();
    assert_eq!(only.len(), 1);
    assert!(Store {
        root: dir.join("none"),
        ..s.clone()
    }
    .list(|_| true)
    .unwrap()
    .is_empty());
}

#[test]
fn an_item_serialises_with_its_id_beside_its_fields() {
    let dir = Scratch::new("review-json");
    let s = store(&dir);
    let item = s.suggest(suggestion("Euler", "/ˈYləɹ/", None)).unwrap();
    let value = serde_json::to_value(&item).unwrap();
    assert_eq!(value["id"], "_library/euler");
    assert_eq!(value["reading"], "/ˈYləɹ/");
    assert_eq!(value["subject"], serde_json::Value::Null);
    assert_eq!(value["sentence"], "We meet Euler here.");
}

#[test]
fn deciding_writes_the_lexicon_and_deletes_the_item() {
    let dir = Scratch::new("review-decide");
    let s = store(&dir);
    let d = dict();
    let subject = dir.join("subjects/COMP30027/lexicon.json");
    std::fs::create_dir_all(subject.parent().unwrap()).unwrap();
    std::fs::write(&subject, r#"{"_about": "comment", "softmax": "SOFT-max"}"#).unwrap();

    let item = s
        .suggest(suggestion("Nesterov", "NESS-ter-off", Some("COMP30027")))
        .unwrap();
    let decided = s.decide(&item.id, Decision::Correct, &d).unwrap();
    assert_eq!(decided.lexicon, subject);
    assert!(s.list(|_| true).unwrap().is_empty());
    let text = std::fs::read_to_string(&subject).unwrap();
    let keys: Vec<String> =
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&text)
            .unwrap()
            .keys()
            .cloned()
            .collect();
    assert_eq!(keys, ["_about", "softmax", "Nesterov"]);

    let item = s.suggest(suggestion("Euler", "EU-ler", None)).unwrap();
    let decided = s.decide(&item.id, Decision::Fine, &d).unwrap();
    assert_eq!(decided.reading, "/nˈɛstəɹɔv/");
    assert_eq!(decided.lexicon, dir.join("library/lexicon.json"));
    let library: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&decided.lexicon).unwrap()).unwrap();
    assert_eq!(library, json!({ "Euler": "/nˈɛstəɹɔv/" }));

    let e = s.decide(&item.id, Decision::Fine, &d).unwrap_err();
    assert!(e.contains("no pronunciation suggestion"), "{e}");
}

#[test]
fn a_bad_reading_is_refused_and_writes_nothing() {
    let dir = Scratch::new("review-bad");
    let s = store(&dir);
    let d = dict();
    let item = s
        .suggest(suggestion("Nesterov", "NESS-tur-off", Some("COMP30027")))
        .unwrap();
    let e = s.decide(&item.id, Decision::Correct, &d).unwrap_err();
    assert!(e.contains("respelling part \"tur\""), "{e}");
    assert!(!dir.join("subjects/COMP30027/lexicon.json").exists());
    assert_eq!(s.list(|_| true).unwrap().len(), 1, "the item stays");

    let e = check("the reading", "Nesterov", "/nˈɛs$/", &d).unwrap_err();
    assert!(e.starts_with("the reading: lexicon \"Nesterov\""), "{e}");
    let ps = check("the reading", "Nesterov", "NESS-ter-off", &d).unwrap();
    assert!(ps.starts_with("nˈɛs"), "{ps}");
}

#[test]
fn the_writer_replaces_a_key_in_place_and_refuses_a_file_that_is_not_an_object() {
    let dir = Scratch::new("review-writer");
    let d = dict();
    let path = dir.join("lexicon.json");
    std::fs::write(&path, r#"{"b": "/bˈi/", "_c": "x", "a": "/ˈA/"}"#).unwrap();
    write_entry(&path, "b", "SOFT", &d).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&text).unwrap();
    assert_eq!(map.keys().collect::<Vec<_>>(), ["b", "_c", "a"]);
    assert_eq!(map["b"], "SOFT");

    std::fs::write(&path, "[1]").unwrap();
    let e = write_entry(&path, "b", "SOFT", &d).unwrap_err();
    assert!(e.contains("not a JSON object"), "{e}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[1]");
}

#[test]
fn concurrent_writes_to_one_lexicon_keep_every_entry() {
    let dir = Scratch::new("review-writer-race");
    let d = dict();
    let path = dir.join("lexicon.json");
    std::thread::scope(|scope| {
        for i in 0..8 {
            let (path, d) = (&path, &d);
            scope.spawn(move || write_entry(path, &format!("word{i}"), "SOFT", d).unwrap());
        }
    });
    let text = std::fs::read_to_string(&path).unwrap();
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&text).unwrap();
    assert_eq!(map.len(), 8, "{text}");
}

fn review(status: Status, note: Option<&str>) -> Review {
    Review {
        status,
        reading: "NESS-ter-off".into(),
        note: note.map(str::to_owned),
    }
}

#[test]
fn a_note_says_what_became_of_its_suggestion() {
    let dir = Scratch::new("review-notes");
    let s = store(&dir);
    let note = |word: &str| Note {
        scene: 0,
        id: "intro".into(),
        line: 2,
        word: word.into(),
        phonemes: "nˈɛstəɹɔv".into(),
        guessed: vec![word.into()],
        review: None,
    };
    let mut notes = vec![note("Nesterov"), note("Adam"), note("Nesterov’s")];
    assert!(notes[0]
        .message()
        .ends_with("(`oculus studio pronounce suggest`)"));

    s.suggest(suggestion("Nesterov", "NESS-ter-off", Some("COMP30027")))
        .unwrap();
    s.annotate(Some("COMP30027"), &mut notes);
    assert_eq!(notes[0].review, Some(review(Status::Pending, None)));
    assert_eq!(notes[1].review, None);
    assert_eq!(notes[2].review, notes[0].review, "a possessive's stem");
    assert_eq!(
        notes[0].message(),
        "check pronunciation: \"Nesterov\" is guessed as /nˈɛstəɹɔv/ (suggested \"NESS-ter-off\", awaiting review in Settings -> Studio -> Pronunciation)"
    );
    let mut elsewhere = vec![note("Nesterov")];
    s.annotate(None, &mut elsewhere);
    assert_eq!(elsewhere[0].review, None, "another subject's suggestion");

    s.mark_wrong("COMP30027/nesterov", Some("NEST-er-off".into()))
        .unwrap();
    s.annotate(Some("COMP30027"), &mut notes);
    assert!(notes[0]
        .message()
        .ends_with("(suggestion \"NESS-ter-off\" marked wrong: \"NEST-er-off\"; suggest again)"));
    assert_eq!(
        review(Status::Wrong, None).suffix(),
        "(suggestion \"NESS-ter-off\" marked wrong; suggest again)"
    );
    let json = serde_json::to_value(&notes[0]).unwrap();
    assert_eq!(json["review"]["status"], "wrong");
}

const SCRIPT: &str = "## intro\nWe meet *Nesterov* here. Then\nNesterov's method again!\n\n## outro\nThe end, Nesterov.\n";

fn project(dir: &Path) -> Project {
    std::fs::write(
        dir.join("video.json"),
        json!({
            "size": { "width": 64, "height": 36 }, "theme": "default",
            "scenes": [
                { "id": "intro", "component": "Card" },
                { "id": "outro", "component": "Card" }
            ]
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(dir.join("script.md"), SCRIPT).unwrap();
    Project::load_unnarrated_with(dir, &Library::default()).unwrap()
}

/// Each whitespace word, punctuation stripped; `Nesterov` guessed only
/// where `guess` says, its possessive read as itself plus `z`.
fn fake_read(text: &str, guess_from: usize) -> Phonemized {
    let mut nesterovs = 0;
    let words = text
        .split_whitespace()
        .map(|w| {
            let word = w.trim_matches(|c: char| c.is_ascii_punctuation() && c != '\'');
            let is = word.starts_with("Nesterov");
            if is {
                nesterovs += 1;
            }
            let guessed = is && nesterovs > guess_from;
            WordRecord {
                word: word.into(),
                phonemes: if word == "Nesterov's" {
                    "nˈɛstəɹɔvz".into()
                } else if is {
                    "nˈɛstəɹɔv".into()
                } else {
                    "x".into()
                },
                source: if guessed { Source::Guess } else { Source::Dict },
                guessed: if guessed {
                    vec![word.into()]
                } else {
                    Vec::new()
                },
            }
        })
        .collect();
    Phonemized {
        phonemes: if text == "Nesterov" {
            "nˈɛstəɹɔv".into()
        } else {
            String::new()
        },
        words,
    }
}

#[test]
fn locate_prefers_the_first_guessed_place_and_its_sentence() {
    let dir = Scratch::new("review-locate");
    let p = project(&dir);
    let found = locate(&p, |t| Ok(fake_read(t, 1)), "nesterov").unwrap();
    assert_eq!(found.word, "Nesterov");
    assert_eq!(found.original, "nˈɛstəɹɔv");
    assert!(found.guessed);
    assert_eq!(found.scene, "intro");
    assert_eq!(found.line, 3);
    assert_eq!(found.sentence, "Then Nesterov's method again!");

    let never = locate(&p, |t| Ok(fake_read(t, 9)), "Nesterov").unwrap();
    assert!(!never.guessed);
    assert_eq!(
        (never.line, never.sentence.as_str()),
        (2, "We meet Nesterov here.")
    );

    let e = locate(&p, |t| Ok(fake_read(t, 0)), "Adam").unwrap_err();
    assert!(e.contains("\"Adam\" is not in the narration"), "{e}");
}
