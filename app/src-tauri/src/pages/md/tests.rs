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

#[test]
fn ampersands_and_angle_brackets_in_text_are_entities() {
    assert_eq!(
        md("<p>Fish &amp; chips in your &lt;head&gt;</p>"),
        "Fish &amp; chips in your &lt;head>"
    );
    assert_eq!(md("<div>a &lt;b&gt; c</div>"), "a &lt;b> c");
    assert_eq!(md("<ul><li>x &lt; y</li></ul>"), "- x &lt; y");
    assert!(md("<table><tr><td>a &amp; &lt;b&gt;</td></tr></table>").contains("a &amp; &lt;b>"));
}

#[test]
fn code_stays_literal() {
    assert_eq!(md("<p><code>&lt;div&gt; &amp;</code></p>"), "`<div> &`");
    assert_eq!(md("<pre>&lt;div&gt;\n</pre>"), "```\n<div>\n```");
}

const ECHO_SRC: &str = "/courses/253699/external_tools/retrieve?display=borderless&amp;url=https%3A%2F%2Fecho360.net.au%2Flti%2F9381284d-5253-4e5b-b173-fb47a63576cb%3FlinkId%3Dabc-123%26autoplay%3Dfalse";

#[test]
fn relative_echo_iframe_becomes_an_absolute_block_tag() {
    let out = md(&format!(
        "<p>Watch <iframe src=\"{ECHO_SRC}\"></iframe> now</p>"
    ));
    assert_eq!(
        out,
        "Watch \n\n<echo-video link-id=\"abc-123\" src=\"https://canvas.lms.unimelb.edu.au/courses/253699/external_tools/retrieve?display=borderless&amp;url=https%3A%2F%2Fecho360.net.au%2Flti%2F9381284d-5253-4e5b-b173-fb47a63576cb%3FlinkId%3Dabc-123%26autoplay%3Dfalse\">\n</echo-video>\n\n now"
    );
}

#[test]
fn echo_iframe_in_a_list_item_stays_inside_it() {
    let out = md(&format!(
        "<ul><li>a<iframe src=\"{ECHO_SRC}\"></iframe></li><li>b</li></ul>"
    ));
    assert!(
        out.starts_with("- a\n  \n  <echo-video link-id=\"abc-123\""),
        "{out}"
    );
    assert!(out.ends_with("\n  </echo-video>\n- b"), "{out}");
}

#[test]
fn other_iframes_link_out_by_host() {
    assert_eq!(
        md("<iframe src=\"https://player.vimeo.com/video/1?h=2\"></iframe>"),
        "[Embedded: player.vimeo.com](https://player.vimeo.com/video/1?h=2)"
    );
    assert_eq!(md("<p>x<iframe src=\"about:blank\"></iframe></p>"), "x");
    assert_eq!(md("<p>x<iframe></iframe></p>"), "x");
}

#[test]
fn details_keep_markdown_inside() {
    assert_eq!(
        md("<details><summary>More &amp; <b>less</b></summary><p><b>hi</b></p></details>"),
        "<details>\n<summary>More &amp; less</summary>\n\n**hi**\n\n</details>"
    );
    assert_eq!(
        md("<details><p>no summary</p></details>"),
        "<details>\n\nno summary\n\n</details>"
    );
}

#[test]
fn thtr_day_one_page() {
    let out = md(include_str!("fixtures/thtr-day-one.html"));

    let ids: Vec<&str> = out
        .lines()
        .filter_map(|l| l.strip_prefix("<echo-video link-id=\""))
        .map(|l| &l[..l.find('"').unwrap()])
        .collect();
    assert_eq!(
        ids,
        [
            "943d56e6-1f0b-4c4e-9a51-7d2c8f3b6a10",
            "3c6e28af-5b7d-4e19-8c2a-0f6e1d9b4c27",
            "b8a1f2d4-6c3e-4a7b-9e0d-2f5c8a1b3e64",
            "e5d7c9a1-2b4f-4d6e-8a0c-3f1b5d7e9a82",
            "71f3b5d9-8e2a-4c6b-a0d4-5e7f9b1c3d95",
            "c2e4a6b8-0d1f-4e3a-b5c7-9d1e3f5a7b06",
            "5a7c9e1b-3d5f-4a8c-b0e2-4f6a8c0e2d19",
        ]
    );
    // Every tag opens a line of its own after a blank one, and closes on the next.
    for (i, l) in out.lines().enumerate() {
        if l.starts_with("<echo-video") {
            assert_eq!(out.lines().nth(i - 1), Some(""), "{out}");
            assert_eq!(out.lines().nth(i + 1), Some("</echo-video>"), "{out}");
        }
    }

    assert!(out.contains("<details>\n<summary>Before you watch: set up your editor</summary>\n\nInstall **VS Code**"), "{out}");
    assert!(out.contains("- Make `index.html`\n\n<details>\n<summary>Stuck on Windows?</summary>\n\nUse *File Explorer*"), "{out}");
    assert_eq!(out.matches("</details>").count(), 2);

    assert!(out.contains("## &lt;section1> The Wot"), "{out}");
    assert!(out.contains("your &lt;head> section"), "{out}");
    assert!(out.contains("Fish &amp; chips"), "{out}");

    let fence = out
        .find("```\n<div class=\"card\">\n  <p>Hello</p>\n</div>\n```")
        .expect(&out);
    assert_eq!(
        out.find("<div"),
        Some(fence + 4),
        "only the fence holds a live <div: {out}"
    );
    assert!(
        out.contains("&lt;div onclick=\"submit()\">Click&lt;/div>"),
        "{out}"
    );

    assert!(
        out.contains("\n[Embedded: padlet.com](https://padlet.com/embed/thtr30042dayone)\n"),
        "{out}"
    );
}
