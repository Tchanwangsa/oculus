//! Ed Lessons: ordering, the index and slide renderers, the workspace zip and
//! its calls.
//! The fixtures are trimmed answers from the live API.

use super::lesson_md::{
    lesson_index_md, lesson_url, slide_md, slide_url, sorted_slides, LessonPlace, LinkedFile,
    SlideExtras, SlideFile,
};
use super::lessons::{order_lessons, workspace_host};
use super::threads::thread_url;
use super::workspace::{unpack, MAX_ENTRY_BYTES};
use super::Ed;
use crate::sync::slug;
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

const STEM: &str = "lessons/120619-assignment-2";

/// Where the fixture lesson goes, linked to Ed's `au` region unless `region`
/// says otherwise.
fn place<'a>(stem: &'a str, region: Option<&'a str>) -> LessonPlace<'a> {
    LessonPlace {
        stem,
        course_id: 38775,
        lesson_id: 120619,
        module: Some("Assignments"),
        order: 4,
        region,
    }
}

/// The lesson's slide files as the sync lays them out under `stem`, each
/// with its entry from `extras`.
fn files_of<'a>(
    lesson: &'a Value,
    stem: &str,
    mut extras: HashMap<i64, SlideExtras>,
) -> Vec<SlideFile<'a>> {
    sorted_slides(lesson)
        .into_iter()
        .enumerate()
        .filter_map(|(i, slide)| {
            let id = slide["id"].as_i64()?;
            Some(SlideFile {
                slide,
                id,
                index: i + 1,
                stem: format!(
                    "{stem}/{:02}-{}",
                    i + 1,
                    slug(slide["title"].as_str().unwrap_or(""))
                ),
                extras: extras.remove(&id).unwrap_or_default(),
            })
        })
        .collect()
}

/// Every slide file of `lesson`, one after the other, with no Ed links.
fn slides_md(lesson: &Value, extras: HashMap<i64, SlideExtras>) -> String {
    let place = place("lessons/1-l", None);
    files_of(lesson, place.stem, extras)
        .iter()
        .filter_map(|f| slide_md(lesson, &place, f))
        .collect()
}

/// Front matter read the way the frontend reads it (`frontmatter.ts`):
/// `key: <json>`, or `key:` then `  - <json>` items.
fn front_of(md: &str) -> (serde_json::Map<String, Value>, &str) {
    let rest = md.strip_prefix("---\n").expect("front matter");
    let end = rest.find("\n---\n").expect("closing fence");
    let mut map = serde_json::Map::new();
    let mut list: Option<String> = None;
    for line in rest[..end].lines() {
        if let (Some(item), Some(key)) = (line.strip_prefix("  - "), &list) {
            let item: Value = serde_json::from_str(item).expect(line);
            map[key.as_str()].as_array_mut().unwrap().push(item);
            continue;
        }
        let (key, value) = line.split_once(':').expect(line);
        if value.is_empty() {
            map.insert(key.into(), json!([]));
            list = Some(key.into());
        } else {
            map.insert(key.into(), serde_json::from_str(value.trim()).expect(line));
            list = None;
        }
    }
    (map, &rest[end + "\n---\n".len()..])
}

fn fixture_extras() -> HashMap<i64, SlideExtras> {
    let mut extras = HashMap::new();
    extras.insert(
        739598,
        SlideExtras {
            pdf: Some(format!("{STEM}/01-week-7-sockets.pdf")),
            ..SlideExtras::default()
        },
    );
    extras.insert(
        831115,
        SlideExtras {
            files: vec![LinkedFile {
                path: "q1a.py".into(),
                rel: format!("{STEM}/03-q1a-tutorial-dfa/q1a.py"),
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
    extras
}

#[test]
fn the_index_lists_every_slide_in_its_front_matter_and_links_them() {
    let l = lesson();
    let place = place(STEM, Some("au"));
    let files = files_of(&l, STEM, fixture_extras());
    let md = lesson_index_md(&l, &place, &files);
    let (front, body) = front_of(&md);

    assert_eq!(front["ed_lesson_id"], 120619);
    assert_eq!(front["title"], "Assignment 2");
    assert_eq!(front["lesson_type"], "general");
    assert_eq!(front["module"], "Assignments");
    assert_eq!(front["order"], 4);
    assert_eq!(front["state"], "scheduled");
    assert_eq!(front["available_at"], "2026-10-05T10:00:00+11:00");
    assert_eq!(front["due_at"], "2026-10-19T23:59:00+11:00");
    assert_eq!(front["locks_at"], Value::Null);
    assert_eq!(
        front["url"],
        "https://edstem.org/au/courses/38775/lessons/120619"
    );
    let slides = front["slides"].as_array().unwrap();
    assert_eq!(slides.len(), 8);
    // In index order: pdf (2) < document (2, larger id) < code (4) < …
    assert_eq!(
        slides[0],
        json!({"id": 739598, "title": "Week 7 - Sockets", "type": "pdf", "status": "unseen",
               "file": null,
               "pdf": "120619-assignment-2/01-week-7-sockets.pdf",
               "url": "https://edstem.org/au/courses/38775/lessons/120619/slides/739598"})
    );
    assert_eq!(slides[1]["id"], 831114);
    assert_eq!(
        slides[1]["file"],
        "120619-assignment-2/02-q1-tutorial-3-marks.md"
    );
    assert_eq!(slides[1]["pdf"], Value::Null);
    assert_eq!(slides[1]["status"], "seen");
    let kinds: Vec<&str> = slides.iter().map(|s| s["type"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        ["pdf", "document", "code", "html", "jupyter", "webpage", "video", "quiz"]
    );
    // Every line of the encoding is one JSON value: no YAML forms beyond it.
    assert!(
        md.contains("\nslides:\n  - {\"id\":739598,\"title\":"),
        "{md}"
    );

    assert!(
        body.starts_with(
            "# Assignment 2\n\n**Module:** Assignments  \n\
             **Available:** 2026-10-05 10:00 · **Due:** 2026-10-19 23:59  \n\
             [Open in Ed](https://edstem.org/au/courses/38775/lessons/120619)\n\n\
             1. [Week 7 - Sockets](120619-assignment-2/01-week-7-sockets.pdf) · pdf\n\
             2. [Q1: Tutorial \\[3 marks\\]](120619-assignment-2/02-q1-tutorial-3-marks.md) · document\n"
        ),
        "{body}"
    );
    assert!(
        body.ends_with(
            "8. [W1.9.1: gdb - Crosswords Quiz \\[Homework\\]](120619-assignment-2/08-w1-9-1-gdb-crosswords-quiz-homework.md) · quiz\n"
        ),
        "{body}"
    );
    assert!(!body.contains("**Locks:**"), "an unset date is left out");
    assert!(!body.contains("The player starts"), "no slide content");
}

#[test]
fn a_pdf_slide_has_no_file_and_the_index_links_its_pdf() {
    // `<NN>-<slug>.md` beside the PDF is where the parser writes its text.
    let l = lesson();
    let place = place(STEM, Some("au"));
    let files = files_of(&l, STEM, fixture_extras());
    assert!(!files[0].has_file());
    assert_eq!(slide_md(&l, &place, &files[0]), None);

    // Not downloaded: the index links Ed's copy and says why.
    let mut extras = HashMap::new();
    extras.insert(
        739598,
        SlideExtras {
            note: Some("The PDF was not downloaded.".into()),
            ..SlideExtras::default()
        },
    );
    let files = files_of(&l, STEM, extras);
    let md = lesson_index_md(&l, &place, &files);
    let (front, body) = front_of(&md);
    assert_eq!(front["slides"][0]["file"], Value::Null);
    assert_eq!(front["slides"][0]["pdf"], Value::Null);
    assert!(
        body.contains("1. [Week 7 - Sockets](https://static.edusercontent.com/files/czEebPI1jIf5vRtDZrV0f3Zt) · pdf · _The PDF was not downloaded._\n"),
        "{body}"
    );
}

#[test]
fn a_slide_file_is_its_front_matter_title_and_content() {
    let l = lesson();
    let place = place(STEM, Some("au"));
    let files = files_of(&l, STEM, fixture_extras());
    assert_eq!(
        slide_md(&l, &place, &files[5]).unwrap(),
        "---\n\
         ed_slide_id: 563686\n\
         ed_lesson_id: 120619\n\
         lesson: \"Assignment 2\"\n\
         title: \"Lecture 18 Missed Segment\"\n\
         type: \"webpage\"\n\
         status: \"completed\"\n\
         index: 6\n\
         pdf: null\n\
         url: \"https://edstem.org/au/courses/38775/lessons/120619/slides/563686\"\n\
         ---\n\
         # Lecture 18 Missed Segment\n\n\
         [Open in Ed](https://edstem.org/au/courses/38775/lessons/120619/slides/563686)\n\n\
         [Lecture 18 Missed Segment](https://echo360.net.au/media/57d34ab4-6eeb-41cd-bb8d-08f47ea8f441/public)\n"
    );
}

#[test]
fn the_fixture_lesson_renders_every_slide_type() {
    let l = lesson();
    let place = place(STEM, Some("au"));
    let files = files_of(&l, STEM, fixture_extras());
    let md = |id: i64| {
        let file = files.iter().find(|f| f.id == id).unwrap();
        slide_md(&l, &place, file).unwrap()
    };

    // document
    let doc = md(831114);
    let (front, body) = front_of(&doc);
    assert_eq!(front["index"], 2);
    assert_eq!(front["type"], "document");
    assert_eq!(front["status"], "seen");
    assert_eq!(front["pdf"], Value::Null);
    assert_eq!(
        front["url"],
        "https://edstem.org/au/courses/38775/lessons/120619/slides/831114"
    );
    assert!(
        body.starts_with(
            "# Q1: Tutorial [3 marks]\n\n\
             [Open in Ed](https://edstem.org/au/courses/38775/lessons/120619/slides/831114)\n\n\
             The player starts"
        ),
        "{body}"
    );
    assert!(
        body.contains("![](https://static.au.edusercontent.com/files/Sv7OzG2sHzY0oggSqIfjiPfM)")
    );
    // code: the brief, the file list beside the slide, the file inline
    let code = md(831115);
    assert!(code.contains("Fill in the given scaffold"), "{code}");
    assert!(
        code.contains("- [q1a.py](./03-q1a-tutorial-dfa/q1a.py)"),
        "{code}"
    );
    assert!(
        code.contains("```python\ndfa = DFA(\n    states={'q0'},\n)\n```"),
        "{code}"
    );
    // video, webpage
    assert!(md(499766).contains("[▶ W1.3: Introduction to gdb \\[Solution Video\\]](https://www.youtube.com/watch?v=1s5bXPqEcIE)"));
    assert!(md(563686).contains("[Lecture 18 Missed Segment](https://echo360.net.au/media/"));
    // html, through the page converter
    let html = md(499794);
    assert!(html.contains("\n### Week 2 Workshop\n"), "{html}");
    assert!(!html.contains("MathJax"), "{html}");
    // quiz
    let quiz = md(499780);
    assert!(
        quiz.contains("\n### 9.1. gdb - Crosswords - Questions\n"),
        "{quiz}"
    );
    assert!(quiz.contains("### Question 1"), "{quiz}");
    // jupyter with no files: the note
    assert!(
        md(582857).contains("/slides/582857)\n\n_The challenge's files could not be fetched._"),
        "{}",
        md(582857)
    );
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
    let md = slides_md(&quiz, extras);

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
    let md = slides_md(&quiz, extras);
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
    let md = slides_md(&l, extras);
    assert!(md.contains("_Unsupported Ed slide type surprise._"), "{md}");
    assert!(
        md.contains("- [my (draft).py](./02-b/my%20%28draft%29.py)"),
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
    let md = slides_md(&l, HashMap::new());
    let headings: Vec<&str> = md.lines().filter(|l| l.starts_with('#')).collect();
    assert_eq!(
        headings,
        [
            "# Intro",
            "### Fireboy",
            "#### Submission",
            "###### Fine print",
            "# not a heading",
            "## nor this",
            "# Sheet",
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

#[test]
fn an_untitled_slide_is_named_by_its_position() {
    let l = lesson_of(json!([{"id": 1, "type": "document", "title": " ", "index": 0}]));
    let place = place("lessons/1-l", None);
    let files = files_of(&l, place.stem, HashMap::new());
    assert_eq!(files[0].stem, "lessons/1-l/01-untitled");
    let md = slide_md(&l, &place, &files[0]).unwrap();
    let (front, body) = front_of(&md);
    assert_eq!(front["title"], "Slide 1");
    assert!(body.starts_with("# Slide 1\n"), "{body}");
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
fn the_workspace_host_and_web_links_follow_eds_region() {
    assert_eq!(workspace_host("us"), "sahara.us.edstem.org");
    assert_eq!(
        lesson_url("us", 1, 2),
        "https://edstem.org/us/courses/1/lessons/2"
    );
    assert_eq!(
        slide_url("us", 1, 2, 3),
        "https://edstem.org/us/courses/1/lessons/2/slides/3"
    );
    assert_eq!(
        thread_url("us", 1, 4),
        "https://edstem.org/us/courses/1/discussion/4"
    );

    // An unknown region: no link in the body, `null` in the front matter.
    let l = lesson_of(json!([{"id": 9, "type": "document", "title": "A", "index": 0}]));
    let place = place("lessons/1-l", None);
    let files = files_of(&l, place.stem, HashMap::new());
    let md = lesson_index_md(&l, &place, &files);
    assert!(!md.contains("Open in Ed"), "{md}");
    let (front, _) = front_of(&md);
    assert_eq!(front["url"], Value::Null);
    assert_eq!(front["slides"][0]["url"], Value::Null);
    let md = slide_md(&l, &place, &files[0]).unwrap();
    let (front, _) = front_of(&md);
    assert_eq!(front["url"], Value::Null);
}
