use super::super::{image_refs, to_markdown, ImageMap};
use super::md;

const EQ: &str = r#"<img class="equation_image" src="https://canvas.lms.unimelb.edu.au/equation_images/X%2520%255Csim%2520%255Cmathrm%257BN%257D?scale=1" title="X \sim \mathrm{N}" data-equation-content="X \sim \mathrm{N}" alt="LaTeX: X \sim \mathrm{N}" data-ignore-a11y-check="">"#;

#[test]
fn an_equation_image_in_prose_is_inline_maths() {
    assert_eq!(
        md(&format!("<p>Let {EQ} where</p>")),
        r"Let $X \sim \mathrm{N}$ where"
    );
}

#[test]
fn an_equation_image_alone_in_its_block_is_display_maths() {
    assert_eq!(
        md(&format!("<p>&nbsp;<span>{EQ}</span> </p><div>{EQ}</div>")),
        "$$\nX \\sim \\mathrm{N}\n$$\n\n$$\nX \\sim \\mathrm{N}\n$$"
    );
}

#[test]
fn equation_latex_falls_back_through_title_src_and_alt() {
    assert_eq!(
        md(r#"<p>a <img class="equation_image" title="\alpha" alt="x"></p>"#),
        r"a $\alpha$"
    );
    assert_eq!(
        md(
            r#"<p>a <img class="equation_image" src="/equation_images/%255Cboldsymbol%2520p?scale=1"></p>"#
        ),
        r"a $\boldsymbol p$"
    );
    assert_eq!(
        md(r#"<p>a <img src="/equation_images/" alt="LaTeX: \beta"></p>"#),
        r"a $\beta$"
    );
    assert_eq!(md(r#"<p>a <img class="equation_image" src=""></p>"#), "a");
}

#[test]
fn equation_latex_is_one_line_and_unescaped_inline() {
    assert_eq!(
        md("<p><img class='equation_image' data-equation-content=' a_1 *\nb ' src='x'> x</p>"),
        "$a_1 * b$ x"
    );
}

#[test]
fn equation_images_survive_table_cells_as_inline_maths() {
    let out = md(&format!(
        "<table><tr><td><p>{EQ}</p></td><td>x {EQ} <img src='p.png'></td></tr></table>"
    ));
    assert_eq!(
        out,
        "| $X \\sim \\mathrm{N}$ | x $X \\sim \\mathrm{N}$ |\n| --- | --- |"
    );
}

#[test]
fn equation_images_are_not_downloaded() {
    let refs = image_refs(
        r#"<img class="equation_image" data-id="3" src="/equation_images/x">
           <img data-api-endpoint="/api/v1/files/4" src="/equation_images/y">
           <img data-id="5" src="a.png">"#,
    );
    assert_eq!(
        refs,
        vec![("/api/v1/files/5".to_string(), "a.png".to_string())]
    );
    // Even a src the image map rewrote stays maths.
    let mut images = ImageMap::new();
    images.insert("/equation_images/x".into(), "images/x.png".into());
    assert_eq!(
        to_markdown(
            r#"<p>a <img class="equation_image" title="x" src="/equation_images/x"></p>"#,
            &images
        ),
        "a $x$"
    );
}

#[test]
fn mathjax_inline_delimiters_become_dollars() {
    assert_eq!(md(r"<p>Let \( x^2 \) be</p>"), "Let $x^2$ be");
    assert_eq!(md(r"<div>Let \(x_1\) be</div>"), "Let $x_1$ be");
    assert_eq!(md(r"<ul><li>Let \(x_1\) be</li></ul>"), "- Let $x_1$ be");
}

#[test]
fn mathjax_display_delimiters_become_a_block() {
    assert_eq!(
        md(r"<p>so \[a = b\] holds</p>"),
        "so \n\n$$\na = b\n$$\n\n holds"
    );
    assert_eq!(md(r"<p>\[a\]</p>"), "$$\na\n$$");
    assert_eq!(
        md(r"<table><tr><td>\[a\]</td></tr></table>"),
        "| $a$ |\n| --- |"
    );
}

#[test]
fn unmatched_delimiters_stay_escaped_prose() {
    assert_eq!(md(r"<p>a \( b</p>"), r"a \\( b");
    assert_eq!(md(r"<p>\(  \) x</p>"), r"\\(  \\) x");
}

#[test]
fn maths_in_code_stays_literal() {
    assert_eq!(md(r"<p><code>\(x\) $5</code></p>"), r"`\(x\) $5`");
    assert_eq!(md("<pre>\\[a\\] $5\n</pre>"), "```\n\\[a\\] $5\n```");
}

#[test]
fn currency_is_never_maths() {
    assert_eq!(md("<p>$10 to $15</p>"), r"\$10 to \$15");
    assert_eq!(md("<div>$10 to $15</div>"), r"\$10 to \$15");
    assert_eq!(md("<ul><li>$10 to $15</li></ul>"), r"- \$10 to \$15");
    assert_eq!(
        md("<table><tr><td>$10 to $15</td></tr></table>"),
        "| \\$10 to \\$15 |\n| --- |"
    );
    // A summary is inside an HTML block, where a backslash would show.
    assert!(md("<details><summary>$5 fee</summary><p>x</p></details>")
        .contains("<summary>$5 fee</summary>"));
}
