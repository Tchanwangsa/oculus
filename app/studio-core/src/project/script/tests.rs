use super::*;

#[test]
fn sections_strip_emphasis_and_collapse_whitespace() {
    let s = Script::parse(
        "\n## intro\n\nWe follow the *gradient*\n  downhill, `one` __small__ step.\n\n## update-2\nThen   we stop.\n",
    )
    .unwrap();
    assert_eq!(s.sections.len(), 2);
    let intro = s.section("intro").unwrap();
    assert_eq!(
        intro.text,
        "We follow the gradient downhill, one small step."
    );
    assert_eq!(intro.line, 2);
    assert!(intro.raw.contains("*gradient*"));
    assert_eq!(s.section("update-2").unwrap().text, "Then we stop.");
    assert!(s.section("outro").is_none());
}

#[test]
fn deeper_headings_are_text() {
    let s = Script::parse("## a\n### not a heading\n").unwrap();
    assert_eq!(s.sections[0].text, "### not a heading");
}

#[test]
fn errors_name_their_line() {
    let e = Script::parse("Intro text\n## a\nhi\n").unwrap_err();
    assert!(e.starts_with("line 1: text before the first"), "{e}");
    let e = Script::parse("## a\nhi\n\n## a\nagain\n").unwrap_err();
    assert!(
        e.contains("line 4: `## a` repeats the section at line 1"),
        "{e}"
    );
    let e = Script::parse("## Intro Scene\nhi\n").unwrap_err();
    assert!(
        e.contains("line 1: `## Intro Scene` is not a scene id"),
        "{e}"
    );
    let e = Script::parse("## a\n\n## b\nhi\n").unwrap_err();
    assert!(e.contains("line 1: `## a` has no text"), "{e}");
    let e = Script::parse("## a\nhi\n## b\n**\n").unwrap_err();
    assert!(e.contains("line 3: `## b` has no text"), "{e}");
}

#[test]
fn pauses_are_cut_from_the_text_and_placed_by_word() {
    let s = Script::parse(
        "## a\n[pause 0.5] It *lands* on one.[pause]\nKeep\n[PAUSE  2] the rest. [pause]\n",
    )
    .unwrap();
    let a = &s.sections[0];
    assert_eq!(a.text, "It lands on one. Keep the rest.");
    let at: Vec<(usize, Option<f64>)> = a.pauses.iter().map(|p| (p.words, p.seconds)).collect();
    assert_eq!(at, [(0, Some(0.5)), (4, None), (5, Some(2.0)), (7, None)]);
    assert_eq!(
        a.clip_text(1.0),
        "[pause 0.5] It lands on one. [pause 1] Keep [pause 2] the rest. [pause 1]"
    );
    assert_eq!(
        a.parts(0.75),
        [
            ClipPart::Pause(0.5),
            ClipPart::Say("It lands on one.".into()),
            ClipPart::Pause(0.75),
            ClipPart::Say("Keep".into()),
            ClipPart::Pause(2.0),
            ClipPart::Say("the rest.".into()),
            ClipPart::Pause(0.75),
        ]
    );
}

#[test]
fn without_pauses_the_clip_text_is_the_text() {
    let s = Script::parse("## a\nOne *two*\nthree.\n").unwrap();
    let a = &s.sections[0];
    assert!(a.pauses.is_empty());
    assert_eq!(a.clip_text(1.0), a.text);
    assert_eq!(a.parts(1.0), [ClipPart::Say("One two three.".into())]);
}

#[test]
fn a_malformed_pause_is_neither_said_nor_held() {
    let s = Script::parse("## a\nOne [pause x] two [pause 9] three [pause\n").unwrap();
    let a = &s.sections[0];
    assert_eq!(a.text, "One two three");
    assert!(a.pauses.is_empty());
    let e = Script::parse("## a\n[pause]\n").unwrap_err();
    assert!(e.contains("`## a` has no text"), "{e}");
}

#[test]
fn markers_read_their_seconds() {
    let read = |line: &str| -> Vec<Result<Option<f64>, String>> {
        markers(line).into_iter().map(|m| m.seconds).collect()
    };
    assert_eq!(
        read("a [pause] b [pause 1.5] [ pause .5 ]"),
        [Ok(None), Ok(Some(1.5)), Ok(Some(0.5))]
    );
    assert!(read("[note] and [1] are text").is_empty());
    let bad = |line: &str| read(line).remove(0).unwrap_err();
    assert!(bad("[pause x]").contains("is not a pause; write `[pause]`"));
    assert!(bad("[pause 1 2]").contains("is not a pause"));
    assert!(bad("[pause -1]").contains("is not a pause"));
    assert!(bad("[pauses]").contains("is not a pause"));
    assert!(bad("[pause 0]").contains("more than 0 and at most 5 seconds"));
    assert!(bad("[pause 5.5]").contains("more than 0 and at most 5 seconds"));
    assert!(bad("[pause 2").contains("no closing `]`"));
    let m = &markers("ab [pause 2] c")[0];
    assert_eq!((m.start, m.end), (3, 12));
}
