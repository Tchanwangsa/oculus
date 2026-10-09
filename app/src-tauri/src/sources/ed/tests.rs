use super::courses::code_token;
use super::document::document_md;
use super::render::fmt_ts;

#[test]
fn subject_tokens_ignore_staff_typed_suffixes() {
    assert_eq!(code_token("comp10002 2024s2"), "COMP10002");
    assert_eq!(code_token("SWEN20003_2025_S2"), "SWEN20003");
    assert_eq!(code_token("INFO30006"), "INFO30006");
    assert_eq!(code_token("  "), "");
}

#[test]
fn timestamps_lose_the_offset_but_keep_the_minute() {
    assert_eq!(
        fmt_ts("2026-08-07T15:42:01.522942+10:00"),
        "2026-08-07 15:42"
    );
    assert_eq!(fmt_ts("junk"), "junk");
}

#[test]
fn document_xml_becomes_markdown() {
    let md = document_md(
        r#"<document version="2.0"><paragraph><bold>CTF</bold> on <link href="https://x.test">this page</link></paragraph><paragraph>See below.</paragraph></document>"#,
    );
    assert_eq!(md, "**CTF** on [this page](https://x.test)\n\nSee below.");
}

#[test]
fn void_tags_do_not_swallow_content() {
    let md = document_md(
        r#"<document><paragraph>one<break/>two <bold>three</bold></paragraph></document>"#,
    );
    assert!(md.contains("one"), "{md}");
    assert!(md.contains("two"), "{md}");
    assert!(md.contains("**three**"), "{md}");
}

#[test]
fn math_blocks_become_display_latex() {
    let md = document_md(
        r#"<document><paragraph>So starting with</paragraph><math>\left(\begin{matrix}1&amp;0\\0&amp;1\end{matrix}\right)</math><paragraph>using dagger.</paragraph><math/></document>"#,
    );
    assert_eq!(
            md,
            "So starting with\n\n$$\n\\left(\\begin{matrix}1&0\\\\0&1\\end{matrix}\\right)\n$$\n\nusing dagger."
        );
}

#[test]
fn images_lists_and_callouts_render() {
    let md = document_md(
        r#"<document><figure><image src="https://img.test/a.png" width="10"/></figure><list style="number"><list-item><paragraph>first</paragraph></list-item><list-item><paragraph>second</paragraph></list-item></list><callout type="info"><bold>note</bold></callout></document>"#,
    );
    assert!(md.contains("![](https://img.test/a.png)"), "{md}");
    assert!(md.contains("1. first"), "{md}");
    assert!(md.contains("2. second"), "{md}");
    assert!(md.contains("> **note**"), "{md}");
}
