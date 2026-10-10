use std::path::Path;

use crate::test_support::Scratch;

use super::bucket::{bucket_dir, resolve_subject};
use super::frontmatter::{parse, render};
use super::maintain::{reindex, relocate, remove};
use super::query::{find, list};
use super::slug::slug;
use super::write::{write, WriteSpec};
use super::INDEX_NAME;

fn scratch(name: &str) -> Scratch {
    let root = Scratch::new(&format!("memory-{name}"));
    std::fs::create_dir_all(root.join("courses/INFO30006_2026_SM2")).unwrap();
    root
}

fn spec(about: &str, body: &str, kind: &str) -> WriteSpec {
    WriteSpec {
        description: Some(about.into()),
        body: Some(body.into()),
        kind: Some(kind.into()),
        ..Default::default()
    }
}

fn index(root: &Path, code: Option<&str>) -> String {
    std::fs::read_to_string(bucket_dir(root, code).join(INDEX_NAME)).unwrap()
}

/// The file and its index line are one call.
#[test]
fn a_memory_and_its_index_are_written_together() {
    let root = scratch("one-write");
    let out = write(
        &root,
        None,
        spec("Wants the verdict first", "Asks for one.", "user"),
    )
    .unwrap();

    let file = std::fs::read_to_string(&out.path).unwrap();
    assert!(
        file.starts_with("---\n"),
        "front matter is written for the caller"
    );
    assert!(file.contains("type: user"));
    assert!(
        file.contains("created: "),
        "and the dates are not the agent's to remember"
    );
    assert!(file.contains("updated: "));

    let index = index(&root, None);
    assert!(
        index.contains("](wants-the-verdict-first.md)"),
        "the index has its line"
    );
    assert!(
        index.contains("Wants the verdict first"),
        "and the hook is the description"
    );
    assert_eq!(out.indexed, 1);
}

/// Derived, not appended, so a deletion is complete.
#[test]
fn the_index_is_derived_from_the_files_beside_it() {
    let root = scratch("derived");
    write(&root, None, spec("First fact", "One.", "reference")).unwrap();
    write(&root, None, spec("Second fact", "Two.", "reference")).unwrap();
    assert_eq!(index(&root, None).matches("- [").count(), 2);

    remove(&root, "first-fact", None).unwrap();
    let after = index(&root, None);
    assert_eq!(after.matches("- [").count(), 1);
    assert!(
        !after.contains("first-fact"),
        "a deleted memory leaves no line behind"
    );

    // A file dropped in by hand is picked up by the next rewrite.
    std::fs::write(
            bucket_dir(&root, None).join("by-hand.md"),
            "---\nname: by-hand\ndescription: Written without the command\nmetadata:\n  type: user\n---\n\nStill a memory.\n",
        )
        .unwrap();
    assert_eq!(reindex(&root, None).unwrap(), 2);
    assert!(index(&root, None).contains("](by-hand.md)"));
}

/// An update keeps what the caller leaves out.
#[test]
fn a_name_already_filed_is_updated_and_keeps_what_was_not_passed() {
    let root = scratch("upsert");
    let first = write(&root, None, spec("A fact", "The body.", "reference")).unwrap();
    assert!(first.created);

    // Passing the name says "this one"; a derived name from a new line is a new memory.
    let second = write(
        &root,
        None,
        WriteSpec {
            name: Some("a-fact".into()),
            description: Some("A better line".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!second.created, "the same name is one memory, not two");
    assert_eq!(second.indexed, 1);

    let file = std::fs::read_to_string(&second.path).unwrap();
    assert!(file.contains("A better line"), "the new line landed");
    assert!(
        file.contains("The body."),
        "the body it did not pass survived"
    );
    assert!(file.contains("type: reference"), "and so did the type");
}

/// Instruction types must carry a reason and an application.
#[test]
fn an_instruction_memory_without_its_why_is_refused() {
    let root = scratch("why");
    let err = write(&root, None, spec("Prefers bun", "Uses bun.", "feedback")).unwrap_err();
    assert!(err.contains("--why"), "{err}");

    let ok = write(
        &root,
        None,
        WriteSpec {
            why: Some("npm writes a second lockfile.".into()),
            how: Some("Reach for bun and bunx.".into()),
            ..spec("Prefers bun", "Uses bun.", "feedback")
        },
    )
    .unwrap();
    let file = std::fs::read_to_string(&ok.path).unwrap();
    assert!(file.contains("**Why:** npm writes"));
    assert!(file.contains("**How to apply:** Reach for bun"));
}

/// A subject code resolves on disk, not in the database.
#[test]
fn a_subject_fact_is_filed_under_the_subject() {
    let root = scratch("buckets");
    let code = resolve_subject(&root, "INFO30006").unwrap();
    assert_eq!(
        code, "INFO30006_2026_SM2",
        "a bare code matches the folder it prefixes"
    );

    write(
        &root,
        Some(&code),
        spec("The MST is on Friday", "Week 7.", "project"),
    )
    .unwrap_err();
    write(
        &root,
        Some(&code),
        WriteSpec {
            why: Some("It gates the week's plan.".into()),
            how: Some("Check it before planning past Friday.".into()),
            ..spec("The MST is on Friday", "Week 7.", "project")
        },
    )
    .unwrap();

    assert!(index(&root, Some(&code)).contains("The MST is on Friday"));
    assert_eq!(
        list(&root, None).unwrap().len(),
        0,
        "and not in the cross-subject bucket"
    );
    assert!(
        resolve_subject(&root, "COMP90007").is_err(),
        "an unknown code is not guessed at"
    );
}

/// Moving a misfiled memory rewrites both indexes.
#[test]
fn moving_a_memory_rewrites_both_indexes() {
    let root = scratch("move");
    let code = resolve_subject(&root, "INFO30006").unwrap();
    write(
        &root,
        None,
        spec("An INFO30006 fact", "Filed wrong.", "reference"),
    )
    .unwrap();

    let entry = find(&root, "an-info30006-fact", None).unwrap();
    let moved = relocate(&root, &entry, Some(&code)).unwrap();
    assert_eq!(moved.subject.as_deref(), Some(code.as_str()));

    assert!(
        !index(&root, None).contains("an-info30006-fact"),
        "gone from the old index"
    );
    assert!(
        index(&root, Some(&code)).contains("an-info30006-fact"),
        "and in the new one"
    );
    assert!(
        find(&root, "an-info30006-fact", None).is_ok(),
        "still findable across buckets"
    );
}

/// Hand-written indexes and prose are kept, and their titles adopted into files.
#[test]
fn a_hand_written_index_keeps_its_prose_and_its_titles() {
    let root = scratch("legacy");
    let dir = bucket_dir(&root, None);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
            dir.join("tchan-study-workflow.md"),
            "---\nname: tchan-study-workflow\ndescription: ROI-first triage; wants a verdict.\nmetadata:\n  type: user\n---\n\nTriages by return on investment.\n",
        )
        .unwrap();
    std::fs::write(
            dir.join(INDEX_NAME),
            "# Memories — all subjects\n\nStanding preferences live in `../TASTE.md`.\n\n- [Tanat's study workflow](tchan-study-workflow.md) — ROI-first triage.\n",
        )
        .unwrap();

    reindex(&root, None).unwrap();
    let after = index(&root, None);
    assert!(
        after.contains("Standing preferences live in"),
        "their prose is kept"
    );
    assert!(
        after.contains("[Tanat's study workflow]"),
        "and so is the title they wrote"
    );

    let file = std::fs::read_to_string(dir.join("tchan-study-workflow.md")).unwrap();
    assert!(
        file.contains("title: Tanat's study workflow"),
        "adopted into the file: {file}"
    );

    // And its dates, stamped once from the filesystem.
    assert!(
        file.contains("created: 20"),
        "dated from the filesystem: {file}"
    );
    assert!(
        !file.contains("updated:"),
        "but not an `updated` — an mtime moves when this rewrite touches the file: {file}"
    );
    let entry = find(&root, "tchan-study-workflow", None).unwrap();
    assert_eq!(entry.dates().0.as_deref(), entry.front.meta("created"));
    assert_eq!(
        entry.dates().1,
        None,
        "nobody has revised it through the command"
    );
}

/// A name is derived, and stays readable in a listing.
#[test]
fn a_write_that_names_nothing_takes_a_name_from_its_line() {
    let root = scratch("naming");
    let out = write(
        &root,
        None,
        spec(
            "Ed answers are the marking authority for INFO30006; the brief is not",
            "Staff said so in Ed #66.",
            "reference",
        ),
    )
    .unwrap();
    assert_eq!(out.name, "ed-answers-are-the-marking-authority");

    assert_eq!(
        slug("INFO30006 — Week 3 (slides)"),
        "info30006-week-3-slides"
    );
    assert!(
        !slug(&"a-very-long-name-".repeat(9)).ends_with('-'),
        "a cut never lands mid-word or on a dash"
    );
    assert!(slug(&"word ".repeat(40)).len() <= 64);
}

/// A guess slugged from the title lands, and a miss lists what is there.
#[test]
fn a_name_read_off_the_title_still_finds_the_file() {
    let root = scratch("find");
    write(
        &root,
        None,
        WriteSpec {
            name: Some("info30006-topic4-group-report-progress".into()),
            title: Some("INFO30006 Topic 4 group-report progress".into()),
            why: Some("It is the week's work.".into()),
            how: Some("Read it before planning the report.".into()),
            ..spec(
                "Group 15's report on the Great Firewall",
                "The draft.",
                "project",
            )
        },
    )
    .unwrap();
    write(
        &root,
        None,
        spec("Ed is the marking authority", "Staff said so.", "reference"),
    )
    .unwrap();

    let name = |q: &str| find(&root, q, None).map(|e| e.front.name);
    assert_eq!(
        name("info30006-topic4-group-report-progress").unwrap(),
        "info30006-topic4-group-report-progress"
    );
    assert_eq!(
        name("info30006-topic-4-group-report-progress").unwrap(),
        "info30006-topic4-group-report-progress",
        "the hyphen a title slugs to is not where the file puts it"
    );
    assert!(
        name("INFO30006 Topic 4 group-report progress").is_ok(),
        "the title itself works"
    );
    assert!(
        name("info30006-topic4").is_ok(),
        "and a prefix, while it is unambiguous"
    );
    assert!(
        name("group-report").is_ok(),
        "and a fragment, from the middle of one"
    );
    // The description is not searched: a sentence matches too much for `rm`.
    assert!(
        name("great-firewall").is_err(),
        "a fragment of the description is not a name"
    );

    // A guess that fits two memories must not pick one.
    write(
        &root,
        None,
        spec("Ed is where the deadlines land", "Also Ed.", "reference"),
    )
    .unwrap();
    let both = find(&root, "ed-is", None).unwrap_err();
    assert!(
        both.contains("ed-is-the-marking-authority"),
        "both names are named: {both}"
    );
    assert!(
        both.contains("ed-is-where-the-deadlines"),
        "both names are named: {both}"
    );

    let miss = find(&root, "something-else-entirely", None).unwrap_err();
    assert!(miss.starts_with("no memory called"), "{miss}");
    assert!(
        miss.contains("the store has"),
        "a miss says what is there: {miss}"
    );
}

/// Front matter round-trips, including an unknown key.
#[test]
fn front_matter_round_trips_including_what_this_build_does_not_know() {
    let text = "---\nname: a-fact\ntitle: A fact\ndescription: \"One line: with a colon\"\nmetadata:\n  type: user\n  provenance: thread 26\n---\n\nThe body.\n";
    let (front, body) = parse(text).unwrap();
    assert_eq!(front.name, "a-fact");
    assert_eq!(front.title.as_deref(), Some("A fact"));
    assert_eq!(front.description, "One line: with a colon");
    assert_eq!(front.meta("provenance"), Some("thread 26"));
    assert_eq!(body.trim(), "The body.");

    let again = render(&front, &body);
    let (front2, body2) = parse(&again).unwrap();
    assert_eq!(
        front2.description, front.description,
        "the colon survived the quotes"
    );
    assert_eq!(front2.meta("provenance"), Some("thread 26"));
    assert_eq!(body2.trim(), body.trim());
}
