//! The token-use rules on small sources, against the bundled theme.
#![allow(clippy::unwrap_used, clippy::panic)]

use super::check;
use crate::error::StudioError;
use crate::theme::Theme;

fn messages(src: &str) -> Vec<String> {
    let theme = Theme::bundled("default").unwrap();
    check("Card", src, &theme, &[])
        .iter()
        .map(|e| match e {
            StudioError::Component { message, .. } => message.clone(),
            other => panic!("unexpected {other}"),
        })
        .collect()
}

#[test]
fn a_colour_literal_names_the_nearest_token() {
    let m = messages(r##"export default () => <Box style={{ fill: "#5e6ad3" }} />;"##);
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].contains("use theme.color.accent "), "{}", m[0]);
    assert!(m[0].contains("nearest: accent"), "{}", m[0]);
}

#[test]
fn functional_and_named_colours_are_literals() {
    for src in [
        r##"const a = { fill: "rgb(10, 20, 30)" };"##,
        r##"const a = { fill: "oklch(0.7 0.1 200)" };"##,
        r##"const a = { color: "white" };"##,
        r##"const a = { color: "#fff" };"##,
        r##"const a = { note: "hsl(200 50% 50%)" };"##,
        r##"export default () => <Text>{"#5e6ad2"}</Text>;"##,
    ] {
        assert_eq!(messages(src).len(), 1, "{src}");
    }
}

#[test]
fn a_token_default_may_hold_its_name_and_its_chain() {
    assert!(
        messages(r##"export const props = { accent: token("color").default("#5e6ad2") };"##)
            .is_empty()
    );
    // A str() default is a literal, not a token name.
    assert_eq!(
        messages(r##"export const props = { note: str().default("#5e6ad2") };"##).len(),
        1
    );
}

#[test]
fn comments_are_not_literals() {
    assert!(
        messages("// fill: \"#5e6ad3\"\n/* \"white\" */ export default () => null;").is_empty()
    );
}

#[test]
fn jsx_text_hex_is_a_literal() {
    assert_eq!(
        messages("export default () => <Text>#5e6ad2</Text>;").len(),
        1
    );
}

#[test]
fn a_font_family_string_is_refused_with_the_token() {
    let heading = messages(r##"const s = { fontFamily: "Manrope" };"##);
    assert_eq!(
        heading,
        ["font family `Manrope` is a literal; use theme.font.heading"]
    );
    let body = messages(r##"const s = { fontFamily: "Inter" };"##);
    assert_eq!(
        body,
        ["font family `Inter` is a literal; use theme.font.body"]
    );
    let other = messages(r##"const s = { fontFamily: "Arial" };"##);
    assert_eq!(other.len(), 1, "{other:?}");
    assert!(messages("const s = { fontFamily: theme.font.body };").is_empty());
}

#[test]
fn a_style_number_equal_to_a_space_or_radius_entry_names_it() {
    // Equal values are flagged wherever they sit in a style, width included;
    // a negative one is an offset, and not a token value.
    let m = messages(
        "export default ({}, { theme }) => <Box style={{ padding: 24, width: 25, gap: -24 }} />;",
    );
    assert_eq!(
        m,
        ["style value 24 equals theme.space.m; use theme.space.m"]
    );
    let m = messages("export default () => <Box style={{ width: 24 }} />;");
    assert_eq!(
        m,
        ["style value 24 equals theme.space.m; use theme.space.m"]
    );
    // 16 is space.s and radius.m: the space name wins.
    let m = messages("export default () => <Box style={{ radius: 16, padding: [0, 16] }} />;");
    assert_eq!(m.len(), 2, "{m:?}");
    assert!(m.iter().all(|x| x.contains("theme.space.s")), "{m:?}");
}

#[test]
fn a_radius_number_names_its_radius_token() {
    let m = messages("export default () => <Box style={{ radius: 44 }} />;");
    assert_eq!(
        m,
        ["style value 44 equals theme.radius.xl; use theme.radius.xl"]
    );
}

#[test]
fn a_nested_style_object_holds_offsets_and_is_not_checked() {
    assert!(messages("export default () => <Box style={{ shadow: { y: 40, blur: 120 }, border: { width: 16 } }} />;").is_empty());
}

#[test]
fn a_numeric_prop_outside_style_is_not_checked() {
    assert!(messages("export default () => <Stack gap={24} />;").is_empty());
}

#[test]
fn the_position_is_the_literal_itself() {
    let theme = Theme::bundled("default").unwrap();
    let src = "export default () => (\n  <Box style={{ fill: \"#5e6ad3\" }} />\n);";
    let errs = check("Card", src, &theme, &[]);
    let StudioError::Component { loc: Some(loc), .. } = &errs[0] else {
        panic!("{errs:?}")
    };
    assert_eq!((loc.line, loc.col), (2, 24), "{loc:?}");
    assert!(loc.snippet.contains("#5e6ad3"));
}

fn with_props(src: &str, colour_props: &[&str]) -> Vec<String> {
    let theme = Theme::bundled("default").unwrap();
    let props: Vec<String> = colour_props.iter().map(|p| (*p).to_owned()).collect();
    check("Card", src, &theme, &props)
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn a_colour_name_is_a_literal_only_where_a_colour_goes() {
    for src in [
        r##"export default () => <Box style={{ color: "purple" }} />;"##,
        r##"export default () => <Box style={{ padding: 5, fill: on ? x : "gold" }} />;"##,
        r##"export default () => <Box style={{ border: { width: 2, color: "red" } }} />;"##,
        r##"export default () => <Box style={{ shadow: { y: 2, color: "black" } }} />;"##,
        r##"const f = { linear: { angle: 90, stops: [[0, "red"], [1, "blue"]] } };"##,
        r##"export default () => <Path d="M0 0" fill="orange" />;"##,
        r##"export default () => <Arrow points={p} stroke={{ width: 4, color: "teal" }} />;"##,
        r##"const s = { backgroundColor: "white", borderColor: "gray" };"##,
    ] {
        let m = messages(src);
        assert!(
            !m.is_empty() && m.iter().all(|x| x.contains("is a literal")),
            "{src}: {m:?}"
        );
    }
    for src in [
        r##"export const examples = { a: { leftValue: "red", rightValue: "purple" } };"##,
        r##"export const props = { label: str().default("gold") };"##,
        r##"export default () => <Card title="orange" />;"##,
        r##"export default () => <Text style={{ color: theme.color.text }}>{"purple"}</Text>;"##,
        r##"const words = ["red", "green", "blue"];"##,
        r##"export default () => <Box style={{ fill: c }} label="silver" />;"##,
        r##"export const props = { accent: token("color").default("purple") };"##,
    ] {
        assert!(messages(src).is_empty(), "{src}: {:?}", messages(src));
    }
}

#[test]
fn a_token_colour_prop_takes_a_theme_name_but_not_a_css_name() {
    let src = r##"export const examples = { a: { tone: "purple" }, b: { tone: "accent" } };"##;
    let m = with_props(src, &["tone"]);
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].contains("colour `purple` is a literal"), "{}", m[0]);
    assert!(with_props(src, &[]).is_empty());
}

#[test]
fn a_unitless_style_key_is_never_checked_for_spacing() {
    for src in [
        "export default () => <Box style={{ opacity: clamp(pop * 4, 0, 1) }} />;",
        "export default () => <Box style={{ opacity: 1, scale: 4, rotate: 16 }} />;",
        "export default () => <Box style={{ flexGrow: 8, flexShrink: 4, zIndex: 24 }} />;",
        "export default () => <Text style={{ fontWeight: 4, lineHeight: 8 }} />;",
        "export default () => <Box style={{ transformOrigin: [4, 8], clipInset: [0, 4, 0, 4] }} />;",
        "export default () => <Text style={{ reveal: { progress: 4 } }} />;",
    ] {
        assert!(messages(src).is_empty(), "{src}: {:?}", messages(src));
    }
}

#[test]
fn a_factor_or_a_call_argument_is_not_a_style_value() {
    for src in [
        "export default () => <Box style={{ width: 24 * s, height: s * 16 }} />;",
        "export default () => <Box style={{ padding: Math.max(16, p), gap: x > 8 ? a : b }} />;",
        "export default () => <Box style={{ translateY: (1 - pop) * 40 }} />;",
    ] {
        assert!(messages(src).is_empty(), "{src}: {:?}", messages(src));
    }
}

#[test]
fn a_length_value_is_checked_through_ternaries_fallbacks_and_arrays() {
    let m = messages(
        "export default () => <Box style={{ padding: big ? 24 : 16, gap: g ?? 8, margin: [(40), 0] }} />;",
    );
    assert_eq!(m.len(), 4, "{m:?}");
    // A term of a sum or difference is a length too; a negative is not.
    let m = messages(
        "export default () => <Box style={{ padding: [8, space.s + 4], width: w - 16, left: -24, top: 64 + x * 2 }} />;",
    );
    assert_eq!(m.len(), 4, "{m:?}");
    // Each style key's own value is checked; a ternary's `:` is not a key.
    let m = messages(
        "export default () => <Box style={{ opacity: on ? x : 4, width: 24, color: \"#fff\" }} />;",
    );
    assert_eq!(m.len(), 2, "{m:?}");
    assert!(m[0].contains("theme.space.m"), "{m:?}");
    assert!(m[1].contains("colour `#fff`"), "{m:?}");
}
