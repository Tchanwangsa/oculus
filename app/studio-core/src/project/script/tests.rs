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
