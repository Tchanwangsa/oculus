use std::path::Path;

use serde_json::json;
use studio_core::{Library, Project};

use super::*;
use crate::studio::tts::kokoro::LexiconFiles;
use crate::studio::voice::g2p::{guess, WordRecord};
use crate::studio::voice::{KokoroVoice, ServerVoice};
use crate::test_support::Scratch;

const SCRIPT: &str =
    "## intro\nWe meet *Nesterov* here,\nand Nesterov again.\n\n## outro\nThe end.\n";

fn project(dir: &Path, script: &str) -> Project {
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
    std::fs::write(dir.join("script.md"), script).unwrap();
    Project::load_unnarrated_with(dir, &Library::default()).unwrap()
}

fn kokoro() -> Voice {
    Voice::Kokoro(KokoroVoice {
        voice: "af_heart".into(),
        speed: 1.0,
    })
}

fn word(word: &str, source: Source) -> WordRecord {
    WordRecord {
        word: word.into(),
        phonemes: "nˈɛstəɹɔv".into(),
        source,
        guessed: if source == Source::Guess {
            vec![word.into()]
        } else {
            Vec::new()
        },
    }
}

/// The `g2p-en-us` pack in `dir` as far as `Reader::load` reads before
/// the guesser: the trimmed fixture dictionary padded to the pinned sizes,
/// and the model files as sparse files of theirs.
fn fake_g2p_pack(dir: &Path) {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/studio/voice/g2p/tests/golden/dict");
    for f in guess::PACK.files {
        let path = dir.join(f.file);
        match std::fs::read(fixture.join(f.file)) {
            Ok(mut bytes) => {
                bytes.resize(f.bytes as usize, b' ');
                std::fs::write(&path, bytes).unwrap();
            }
            Err(_) => std::fs::File::create(&path)
                .unwrap()
                .set_len(f.bytes)
                .unwrap(),
        }
    }
    assert!(crate::runtime::onnx::is_present(dir, &guess::PACK));
}

#[test]
fn the_server_engine_has_nothing_to_check() {
    let dir = Scratch::new("voice-lint-server");
    let p = project(&dir, SCRIPT);
    let server = Voice::Server(ServerVoice {
        base_url: "http://127.0.0.1:8880".into(),
        model: "kokoro".into(),
        voice: "af_heart".into(),
        speed: 1.0,
    });
    let out = lint(&p, &server, &dir, &LexiconFiles::default());
    assert!(out.issues.is_empty() && out.notes.is_empty());
}

#[test]
fn a_missing_g2p_pack_is_one_issue_saying_how_to_get_it() {
    let dir = Scratch::new("voice-lint-no-pack");
    let p = project(&dir, SCRIPT);
    let out = lint(&p, &kokoro(), &dir.join("none"), &LexiconFiles::default());
    assert_eq!(out.issues.len(), 1);
    let e = &out.issues[0].error;
    assert_eq!(e.kind(), "lint");
    assert!(
        e.to_string().contains("oculus studio voices download"),
        "{e}"
    );
}

#[test]
fn a_bad_lexicon_entry_is_an_issue_naming_its_file_and_key() {
    let dir = Scratch::new("voice-lint-lexicon");
    let p = project(&dir, SCRIPT);
    let pack = dir.join("pack");
    std::fs::create_dir_all(&pack).unwrap();
    fake_g2p_pack(&pack);
    let lexicon = dir.join("lexicon.json");
    std::fs::write(&lexicon, r#"{"Nesterov": "/nˈɛs$/"}"#).unwrap();
    let files = LexiconFiles {
        library: Some(lexicon),
        subject: None,
        pin: None,
    };
    let out = lint(&p, &kokoro(), &pack, &files);
    assert_eq!(out.issues.len(), 1);
    let message = out.issues[0].error.to_string();
    assert!(
        message.contains("lexicon.json") && message.contains("\"Nesterov\""),
        "{message}"
    );
}

#[test]
fn each_guessed_word_is_a_note_on_its_own_line() {
    let dir = Scratch::new("voice-lint-notes");
    let p = project(&dir, SCRIPT);
    let intro = Phonemized {
        phonemes: String::new(),
        words: vec![
            word("We", Source::Dict),
            word("meet", Source::Dict),
            word("Nesterov", Source::Guess),
            word("here", Source::Dict),
            word("and", Source::Dict),
            word("Nesterov", Source::Guess),
            word("again", Source::Dict),
        ],
    };
    let found = notes(&p, |text| {
        text.starts_with("We meet").then(|| intro.clone())
    });
    let lines: Vec<(usize, &str, usize)> = found
        .iter()
        .map(|n| (n.scene, n.id.as_str(), n.line))
        .collect();
    assert_eq!(lines, [(0, "intro", 2), (0, "intro", 3)]);
    assert!(found[0].message().contains("check pronunciation"));
    assert!(found[0].message().contains("/nˈɛstəɹɔv/"));
}

/// Through the real G2P: STUDIO_G2P_GUESSER=<g2p-en-us folder> cargo test
/// --lib studio::voice::lint -- --ignored
#[test]
#[ignore = "needs STUDIO_G2P_GUESSER=<g2p-en-us pack folder>"]
fn a_real_read_notes_guesses_and_refuses_a_sentence_too_long() {
    let pack = std::path::PathBuf::from(std::env::var("STUDIO_G2P_GUESSER").unwrap());
    let dir = Scratch::new("voice-lint-real");
    let long = vec!["gradient"; 80].join(" ");
    let p = project(
        &dir,
        &format!("## intro\nWe meet Nesterov.\n\n## outro\n{long}.\n"),
    );
    let out = lint(&p, &kokoro(), &pack, &LexiconFiles::default());
    let words: Vec<(&str, usize)> = out
        .notes
        .iter()
        .map(|n| (n.word.as_str(), n.line))
        .collect();
    assert_eq!(words, [("Nesterov", 2)]);
    assert_eq!(out.issues.len(), 1);
    let issue = &out.issues[0];
    assert_eq!(issue.id.as_deref(), Some("outro"));
    let message = issue.error.to_string();
    assert!(
        message.starts_with("script.md:4:") && message.contains("too long"),
        "{message}"
    );
}
