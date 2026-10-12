use crate::math::{Shape, typeset};

fn keys(tex: &str) -> Vec<String> {
    typeset(tex, true)
        .unwrap()
        .items
        .into_iter()
        .map(|i| i.key)
        .collect()
}

#[test]
fn glyphs_are_keyed_by_character_and_occurrence() {
    assert_eq!(keys("2^4 - 1"), ["2", "4^", "−", "1"]);
    assert_eq!(
        keys("(2^2-1)(2^2+1)"),
        [
            "(", "2", "2^", "−", "1", ")", "(#2", "2#2", "2^#2", "+", "1#2", ")#2"
        ]
    );
}

#[test]
fn a_script_is_keyed_apart_from_the_baseline() {
    // The exponent of x^2 is not the 2 of (x - 2); the nth baseline x
    // still meets the nth.
    assert_eq!(
        keys("x^2 - 5x + 6 = 0"),
        ["x", "2^", "−", "5", "x#2", "+", "6", "=", "0"]
    );
    assert_eq!(
        keys("(x - 2)(x - 3) = 0"),
        [
            "(", "x", "−", "2", ")", "(#2", "x#2", "−#2", "3", ")#2", "=", "0"
        ]
    );
    // Up and down, nested; a sub- and superscript pair reads top first.
    assert_eq!(keys("e^{x^2}"), ["e", "x^", "2^^"]);
    assert_eq!(
        keys("x_i^2 + x_{i_0}"),
        ["x", "2^", "i_", "+", "x#2", "i_#2", "0__"]
    );
    // Limits above and below an operator are scripts too; the operator is not.
    assert_eq!(
        keys(r"\sum_{i=1}^{n} i"),
        ["n^", "∑", "i_", "=_", "1_", "i"]
    );
    // A root's index is raised; a fraction's terms stay on their level.
    assert_eq!(
        keys(r"\sqrt[3]{8} = \frac{a}{2}"),
        ["3^", "sqrt", "8", "=", "a", "frac", "2"]
    );
}

#[test]
fn a_named_group_overrides_the_script_key() {
    assert_eq!(keys(r"x^{\k{e}{2}} + 2"), ["x", "e", "+", "2"]);
}

#[test]
fn a_vlist_reads_top_to_bottom() {
    // KaTeX lists a fraction denominator first; reading order is
    // numerator, bar, denominator, and a root sign before its radicand.
    assert_eq!(keys(r"\frac{a}{b}"), ["a", "frac", "b"]);
    assert_eq!(keys(r"\sqrt{x}"), ["sqrt", "x"]);
}

#[test]
fn a_named_group_keys_its_terms_in_order() {
    assert_eq!(
        keys(r"\k{lhs}{2^4 - 1} = \k{rhs}{15}"),
        ["lhs", "lhs.1", "lhs.2", "lhs.3", "=", "rhs", "rhs.1"]
    );
    // The innermost group wins.
    assert_eq!(keys(r"\k{a}{x + \k{b}{y}}"), ["a", "a.1", "b"]);
}

#[test]
fn a_group_named_like_a_symbol_is_refused() {
    let e = typeset(r"\k{x}{y} + x", true).unwrap_err();
    assert!(e.contains("`x`"), "{e}");
}

#[test]
fn rules_and_svg_are_pieces_too() {
    let f = typeset(r"\frac{\sqrt{x}}{\overline{y}}", true).unwrap();
    let kinds: Vec<_> = f
        .items
        .iter()
        .map(|i| match i.shape {
            Shape::Glyph(_) => "glyph",
            Shape::Rule => "rule",
            Shape::Path { .. } => "path",
        })
        .collect();
    assert_eq!(kinds, ["path", "glyph", "rule", "rule", "glyph"]);
}

#[test]
fn a_parse_error_names_its_position() {
    let e = typeset(r"\frac{a}{", true).unwrap_err();
    assert!(e.contains("at end of input"), "{e}");
    let e = typeset(r"x + \nope", true).unwrap_err();
    assert!(e.contains("position 5"), "{e}");
}
