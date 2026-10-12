//! Ed Lessons: ordering, the lesson renderer, the workspace zip and its calls.
//! The fixtures are trimmed answers from the live API.

use super::lesson_md::{lesson_md, lesson_url, sorted_slides, LinkedFile, SlideExtras};
use super::lessons::{order_lessons, workspace_host};
use super::workspace::{unpack, MAX_ENTRY_BYTES};
use super::Ed;
use crate::test_support::{FakeKeyd, Scratch};
use serde_json::{json, Value};

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::rc::Rc;

const LESSON: &str = include_str!("fixtures/lesson.json");
const LESSONS: &str = include_str!("fixtures/lessons-list.json");
const QUESTIONS: &str = include_str!("fixtures/quiz-questions.json");

fn lesson() -> Value {
    serde_json::from_str::<Value>(LESSON).unwrap()["lesson"].clone()
}

fn questions() -> Vec<Value> {
    serde_json::from_str::<Value>(QUESTIONS).unwrap()["questions"]
        .as_array()
        .unwrap()
        .clone()
}

/// A lesson holding just these slides.
fn lesson_of(slides: Value) -> Value {
    json!({"id": 1, "title": "L", "slides": slides})
}

#[test]
fn lessons_follow_module_order_then_index_with_unset_last() {
    let list: Value = serde_json::from_str(LESSONS).unwrap();
    let mut lessons = list["lessons"].as_array().unwrap().clone();
    let modules = vec![(23757, "Examples".into()), (24732, "Assignments".into())];
    order_lessons(&mut lessons, &modules);
    let ids: Vec<i64> = lessons.iter().map(|l| l["id"].as_i64().unwrap()).collect();
    assert_eq!(ids, [115326, 119094, 116662]);

    // A lesson in no listed module goes after every module.
    let mut lessons = vec![
        json!({"id": 3, "module_id": null, "index": 0}),
        json!({"id": 2, "module_id": 24732, "index": null}),
        json!({"id": 1, "module_id": 24732, "index": null}),
    ];
    order_lessons(&mut lessons, &modules);
    let ids: Vec<i64> = lessons.iter().map(|l| l["id"].as_i64().unwrap()).collect();
    assert_eq!(ids, [1, 2, 3]);
}

#[test]
fn slides_go_by_index_then_id() {
    let l = lesson_of(json!([
        {"id": 9, "index": 2}, {"id": 4, "index": null}, {"id": 7, "index": 2}, {"id": 8, "index": 1}
    ]));
    let ids: Vec<i64> = sorted_slides(&l)
        .iter()
        .map(|s| s["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, [8, 7, 9, 4]);
}

#[test]
fn the_fixture_lesson_renders_every_slide_type() {
    let mut extras = HashMap::new();
    extras.insert(
        739598,
        SlideExtras {
            pdf: Some("lessons/120619-assignment-2/01-week-7-sockets.pdf".into()),
            ..SlideExtras::default()
        },
    );
    extras.insert(
        831115,
        SlideExtras {
            files: vec![LinkedFile {
                path: "q1a.py".into(),
                rel: "lessons/120619-assignment-2/03-q1a-tutorial-dfa/q1a.py".into(),
                text: Some("dfa = DFA(\n    states={'q0'},\n)\n".into()),
            }],
            ..SlideExtras::default()
        },
    );
    extras.insert(
        499780,
        SlideExtras {
            questions: questions(),
            ..SlideExtras::default()
        },
    );
    extras.insert(
        582857,
        SlideExtras {
            note: Some("The challenge's files could not be fetched.".into()),
            ..SlideExtras::default()
        },
    );
    let url = lesson_url("au", 38775, 120619);
    let md = lesson_md(&lesson(), Some(&url), Some("Assignments"), &extras);

    assert!(
        md.starts_with("# Assignment 2\n\n**Module:** Assignments  \n"),
        "{md}"
    );
    assert!(
        md.contains("**Available:** 2026-10-05 10:00 · **Due:** 2026-10-19 23:59  \n"),
        "{md}"
    );
    assert!(
        !md.contains("**Locks:**"),
        "an unset date is left out: {md}"
    );
    assert!(md.contains("[Open in Ed](https://edstem.org/au/courses/38775/lessons/120619)"));
    // document
    assert!(
        md.contains("## Q1: Tutorial [3 marks]\n\nThe player starts"),
        "{md}"
    );
    assert!(md.contains("![](https://static.au.edusercontent.com/files/Sv7OzG2sHzY0oggSqIfjiPfM)"));
    // pdf, linked from the lesson's folder
    assert!(
        md.contains("[Week 7 - Sockets (PDF)](120619-assignment-2/01-week-7-sockets.pdf)"),
        "{md}"
    );
    // code: the brief, the file list, the file inline
    assert!(md.contains("Fill in the given scaffold"), "{md}");
    assert!(
        md.contains("- [q1a.py](120619-assignment-2/03-q1a-tutorial-dfa/q1a.py)"),
        "{md}"
    );
    assert!(
        md.contains("```python\ndfa = DFA(\n    states={'q0'},\n)\n```"),
        "{md}"
    );
    // video, webpage
    assert!(md.contains("[▶ W1.3: Introduction to gdb \\[Solution Video\\]](https://www.youtube.com/watch?v=1s5bXPqEcIE)"), "{md}");
    assert!(md.contains("[Lecture 18 Missed Segment](https://echo360.net.au/media/"));
    // html, through the page converter
    assert!(md.contains("\n### Week 2 Workshop\n"), "{md}");
    assert!(!md.contains("MathJax"), "{md}");
    // quiz
    assert!(
        md.contains("\n### 9.1. gdb - Crosswords - Questions\n"),
        "{md}"
    );
    assert!(md.contains("### Question 1"), "{md}");
    // jupyter with no files: the note
    assert!(
        md.contains("## Text Processing & BoW\n\n_The challenge's files could not be fetched._"),
        "{md}"
    );

    // In index order: pdf (2) < document (2, larger id) < code (4) < …
    let at = |s: &str| md.find(s).unwrap();
    assert!(at("## Week 7 - Sockets") < at("## Q1: Tutorial"));
    assert!(at("## Q1: Tutorial") < at("## Q1a: Tutorial DFA"));
    assert!(at("## Tutorial Questions") < at("## Text Processing"));
    assert!(at("## Lecture 18") < at("## W1.3"));
    assert!(at("## W1.3") < at("## W1.9.1"));
}

#[test]
fn a_released_quiz_shows_its_answers_collapsed() {
    let quiz =
        lesson_of(json!([{"id": 5, "type": "quiz", "title": "Q", "index": 0, "passage": ""}]));
    let mut extras = HashMap::new();
    extras.insert(
        5,
        SlideExtras {
            questions: questions(),
            ..SlideExtras::default()
        },
    );
    let md = lesson_md(&quiz, None, None, &extras);

    assert!(md.contains("- [ ] Yes, the solved crossword was found.\n- [ ] No, the program terminated incorrectly."), "{md}");
    assert!(
        md.contains("<details>\n<summary>Solution</summary>\n\n**Answer:** Yes, the solved crossword was found.\n\n</details>"),
        "an empty explanation adds nothing: {md}"
    );
    assert!(
        md.contains("<details>\n<summary>Solution</summary>\n\n**Answers:**\n\n- Checking of potential answers\n- Reporting the solution\n\nThe program has issues in its assumptions."),
        "{md}"
    );
    // The third question has no `solution`: its section has no details.
    let third = &md[md.find("### Question 3").unwrap()..];
    assert!(third.contains("Unreleased question"), "{md}");
    assert!(!third.contains("<details>"), "{md}");
    assert_eq!(md.matches("<details>").count(), 2, "{md}");
}

#[test]
fn a_short_answer_solution_may_be_text() {
    let quiz = lesson_of(json!([{"id": 5, "type": "quiz", "title": "Q", "index": 0}]));
    let mut extras = HashMap::new();
    extras.insert(
        5,
        SlideExtras {
            questions: vec![json!({"data": {
                "type": "short-answer", "content": "<document><paragraph>2+2?</paragraph></document>",
                "answers": [], "solution": ["4", "four"], "explanation": ""
            }})],
            ..SlideExtras::default()
        },
    );
    let md = lesson_md(&quiz, None, None, &extras);
    assert!(md.contains("**Answers:**\n\n- 4\n- four"), "{md}");
    assert!(!md.contains("- [ ]"), "{md}");
}

#[test]
fn unknown_slides_say_so_and_links_are_percent_encoded() {
    let l = lesson_of(json!([
        {"id": 1, "type": "surprise", "title": "A", "index": 0},
        {"id": 2, "type": "code", "title": "B", "index": 1, "content": ""}
    ]));
    let mut extras = HashMap::new();
    extras.insert(
        2,
        SlideExtras {
            files: vec![LinkedFile {
                path: "my (draft).py".into(),
                rel: "lessons/1-l/02-b/my (draft).py".into(),
                text: None,
            }],
            ..SlideExtras::default()
        },
    );
    let md = lesson_md(&l, None, None, &extras);
    assert!(md.contains("_Unsupported Ed slide type surprise._"), "{md}");
    assert!(
        md.contains("- [my (draft).py](1-l/02-b/my%20%28draft%29.py)"),
        "{md}"
    );
}

#[test]
fn a_slides_own_headings_sit_below_its_title() {
    let l = lesson_of(json!([
        {"id": 1, "type": "document", "title": "Intro", "index": 0, "content":
            "<document><heading level=\"1\">Fireboy</heading><heading level=\"2\">Submission</heading><heading level=\"5\">Fine print</heading><snippet># not a heading\n## nor this</snippet></document>"},
        {"id": 2, "type": "html", "title": "Sheet", "index": 1,
            "html": "<html><body><h1>Week 2</h1><h2>Part A</h2></body></html>"}
    ]));
    let md = lesson_md(&l, None, None, &HashMap::new());
    let headings: Vec<&str> = md.lines().filter(|l| l.starts_with('#')).collect();
    assert_eq!(
        headings,
        [
            "# L",
            "## Intro",
            "### Fireboy",
            "#### Submission",
            "###### Fine print",
            "# not a heading",
            "## nor this",
            "## Sheet",
            "### Week 2",
            "#### Part A",
        ],
        "{md}"
    );
    assert!(
        md.contains("```\n# not a heading\n## nor this\n```"),
        "{md}"
    );
}

fn zip_of(entries: &[(&str, Option<&[u8]>)]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, body) in entries {
        match body {
            Some(body) => {
                w.start_file(*name, opts).unwrap();
                w.write_all(body).unwrap();
            }
            None => w.add_directory(*name, opts).unwrap(),
        }
    }
    w.finish().unwrap().into_inner()
}

#[test]
fn the_workspace_keeps_safe_small_visible_files() {
    let big = vec![0u8; MAX_ENTRY_BYTES as usize + 1];
    let zip = zip_of(&[
        ("src/", None),
        ("src/main.py", Some(b"print(1)\n")),
        ("../escape.py", Some(b"x")),
        ("/abs.py", Some(b"x")),
        (".env", Some(b"x")),
        ("src/.git/config", Some(b"x")),
        ("__MACOSX/src/._main.py", Some(b"x")),
        ("data/big.bin", Some(&big)),
        ("notes.txt", Some(b"hi")),
    ]);
    let (files, skipped) = unpack(&zip).unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["src/main.py", "notes.txt"]);
    assert_eq!(files[0].bytes, b"print(1)\n");
    assert!(
        skipped
            .iter()
            .any(|s| s.contains("escape.py") && s.contains("unsafe")),
        "{skipped:?}"
    );
    assert!(
        skipped
            .iter()
            .any(|s| s == "data/big.bin: over 20 MB, skipped"),
        "{skipped:?}"
    );
    assert!(unpack(b"not a zip").is_err());
}

#[test]
fn the_workspace_is_connected_and_ticketed_through_the_ed_route() {
    let dir = Scratch::new("ed-workspace");
    let keyd = FakeKeyd::start(&dir, |header, _| {
        let body: &[u8] = match header["path"].as_str().unwrap() {
            "/api/challenges/281410/connect" => br#"{"ticket":"c","workspace_id":"W1"}"#,
            "/api/workspaces/W1/download" => br#"{"ticket":"T0"}"#,
            _ => b"{}",
        };
        (json!({"status": 200, "headers": []}), body.to_vec())
    });
    /// Logs its own drop, which is when the live session would close.
    struct Session(Rc<RefCell<Vec<String>>>);
    impl Drop for Session {
        fn drop(&mut self) {
            self.0.borrow_mut().push("closed".into());
        }
    }
    let events = Rc::new(RefCell::new(Vec::<String>::new()));
    let profile = crate::sources::institution::Institution {
        ed_region: Some("au".to_string()),
        ..crate::sources::institution::unimelb()
    };
    crate::sources::institution::save(&dir, &profile).unwrap();
    let zip = Ed::open(&dir)
        .workspace_zip(
            281410,
            |host, ticket| {
                let posts = keyd.requests().len();
                events
                    .borrow_mut()
                    .push(format!("live {host} {ticket} after {posts}"));
                Ok(Session(events.clone()))
            },
            |url| {
                events.borrow_mut().push(format!("download {url}"));
                Ok(b"PK".to_vec())
            },
        )
        .unwrap();

    assert_eq!(zip, b"PK");
    // The session is live from just after connect until the zip is in.
    assert_eq!(
        *events.borrow(),
        [
            "live sahara.au.edstem.org c after 1".to_string(),
            "download https://sahara.au.edstem.org/download/T0".to_string(),
            "closed".to_string(),
        ]
    );
    let requests = keyd.requests();
    assert_eq!(requests.len(), 2);
    for (header, _) in &requests {
        assert_eq!(header["secret"], "ed");
        assert_eq!(header["method"], "POST");
        assert_eq!(
            header["headers"],
            json!([["content-type", "application/json"]])
        );
    }
    assert_eq!(requests[0].0["path"], "/api/challenges/281410/connect");
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[0].1).unwrap(),
        json!({})
    );
    assert_eq!(requests[1].0["path"], "/api/workspaces/W1/download");
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[1].1).unwrap(),
        json!({"path": "."})
    );
}

#[test]
fn lessons_and_questions_are_plain_reads() {
    let dir = Scratch::new("ed-lesson-reads");
    let keyd = FakeKeyd::start(&dir, |header, _| {
        let body = match header["path"].as_str().unwrap() {
            "/api/courses/38775/lessons" => LESSONS,
            "/api/lessons/120619" => LESSON,
            "/api/lessons/slides/499780/questions" => QUESTIONS,
            _ => "{}",
        };
        (
            json!({"status": 200, "headers": []}),
            body.as_bytes().to_vec(),
        )
    });
    let ed = Ed::open(&dir);
    let list = ed.lessons(38775).unwrap();
    assert_eq!(list.lessons.len(), 3);
    assert_eq!(list.module_name(Some(24732)), Some("Assignments"));
    assert_eq!(ed.lesson(120619).unwrap()["id"], 120619);
    assert_eq!(ed.quiz_questions(499780).unwrap().len(), 3);
    for (header, _) in keyd.requests() {
        assert_eq!(header["method"], "GET");
        assert!(
            !header["path"].as_str().unwrap().contains("view="),
            "{header}"
        );
    }
}

#[test]
fn the_workspace_host_and_lesson_link_follow_eds_region() {
    assert_eq!(workspace_host("us"), "sahara.us.edstem.org");
    assert_eq!(
        lesson_url("us", 1, 2),
        "https://edstem.org/us/courses/1/lessons/2"
    );
    let md = lesson_md(&json!({"id": 2, "title": "L"}), None, None, &HashMap::new());
    assert!(!md.contains("Open in Ed"), "{md}");
}
