use super::ImageMap;
use super::*;

fn md(html: &str) -> String {
    to_markdown(html, &ImageMap::new())
}

#[test]
fn headings_paragraphs_and_emphasis() {
    assert_eq!(
        md("<h2>Week 1</h2><p>Read <strong>chapter 3</strong> and <em>notes</em>.</p>"),
        "## Week 1\n\nRead **chapter 3** and *notes*."
    );
}

#[test]
fn nested_lists_indent() {
    let out = md("<ul><li>a<ul><li>b</li></ul></li><li>c</li></ul>");
    assert_eq!(out, "- a\n  - b\n- c");
}

#[test]
fn ordered_lists_number_from_one() {
    assert_eq!(
        md("<ol><li>first</li><li>second</li></ol>"),
        "1. first\n2. second"
    );
}

#[test]
fn tables_stay_single_line_per_row() {
    let out = md("<table><tr><th>Week</th><th>Topic</th></tr>\
                      <tr><td>1</td><td><p>Intro</p><p>Setup</p></td></tr></table>");
    assert_eq!(
        out,
        "| Week | Topic |\n| --- | --- |\n| 1 | Intro · Setup |"
    );
}

#[test]
fn table_cells_escape_pipes_and_drop_images() {
    let out = md("<table><tr><td>a|b</td><td><img src='x.png'>text</td></tr></table>");
    assert!(out.contains(r"a\|b"), "{out}");
    assert!(!out.contains("x.png"), "{out}");
}

#[test]
fn short_rows_are_padded_to_the_header_width() {
    let out = md("<table><tr><th>A</th><th>B</th></tr><tr><td>1</td></tr></table>");
    assert!(out.ends_with("| 1 |  |"), "{out}");
}

#[test]
fn screenreader_and_script_content_is_dropped() {
    let out = md("<p>keep<span class='screenreader-only'>drop</span></p><script>bad()</script>");
    assert_eq!(out, "keep");
}

#[test]
fn empty_emphasis_leaves_no_artifact() {
    // Canvas emits <strong></strong>; naive conversion yields "****".
    assert_eq!(md("<p>a<strong></strong>b</p>"), "ab");
}

#[test]
fn marker_only_emphasis_is_not_wrapped() {
    assert_eq!(md("<p><strong>†</strong> note</p>"), "† note");
}

#[test]
fn links_and_images_survive() {
    assert_eq!(
        md("<p><a href='/x'>go</a> <img src='p.png' alt='fig'></p>"),
        "[go](/x) ![fig](p.png)"
    );
}

#[test]
fn image_src_is_rewritten_to_the_local_copy() {
    let mut images = ImageMap::new();
    images.insert("/courses/1/files/9/preview".into(), "images/9.png".into());
    let out = to_markdown(
        "<p><img src='/courses/1/files/9/preview' alt='f'></p>",
        &images,
    );
    assert_eq!(out, "![f](images/9.png)");
}

#[test]
fn markdown_specials_in_text_are_escaped() {
    assert_eq!(md("<p>2*3 and _x_ [ok]</p>"), r"2\*3 and \_x\_ \[ok\]");
}

#[test]
fn blockquotes_and_code_blocks() {
    assert_eq!(md("<blockquote><p>hi</p></blockquote>"), "> hi");
    assert_eq!(md("<pre>let x = 1;\n</pre>"), "```\nlet x = 1;\n```");
}

#[test]
fn finds_image_endpoints_from_either_attribute() {
    let refs = image_refs(
        "<img data-api-endpoint='/api/v1/files/5' src='a.png'>\
             <img data-id='7' src='b.png'><img src='c.png'>",
    );
    assert_eq!(
        refs,
        vec![
            ("/api/v1/files/5".to_string(), "a.png".to_string()),
            ("/api/v1/files/7".to_string(), "b.png".to_string()),
        ]
    );
}

#[test]
fn extracts_page_slugs_and_course_file_ids() {
    let (pages, files) = canvas_links(
        "<a href='/courses/42/pages/week-one?x=1'>p</a>\
             <a href='/courses/42/files/123/download'>f</a>\
             <a href='/courses/99/files/456'>other course</a>",
        42,
    );
    assert_eq!(pages, vec!["week-one"]);
    assert_eq!(files, vec!["123"]);
}
