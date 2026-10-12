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
    assert_eq!(keys("2^4 - 1"), ["2", "4", "−", "1"]);
    assert_eq!(
        keys("(2^2-1)(2^2+1)"),
        [
            "(", "2", "2#2", "−", "1", ")", "(#2", "2#3", "2#4", "+", "1#2", ")#2"
        ]
    );
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
