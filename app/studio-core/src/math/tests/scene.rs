//! `<Math>` in a component: expanded into keyed glyphs, matched across a
//! Morph, and its errors.

use crate::error::StudioError;
use crate::layout::LNode;
use crate::render::{Engine, RenderOptions, SceneCall, Size};
use crate::theme::Theme;
use serde_json::{Map, Value, json};

fn engine(tsx: &str) -> Engine {
    let theme = Theme::bundled("default").unwrap();
    let size = Size {
        width: 640,
        height: 360,
    };
    let mut e = Engine::new(theme, size, None, RenderOptions::default()).unwrap();
    e.load_component("F", tsx).unwrap();
    e
}

fn prepare(e: &mut Engine, t: f64, props: &Map<String, Value>) -> Result<LNode, StudioError> {
    let call = SceneCall {
        component: "F",
        props,
        t,
        duration: 1.0,
        index: 0,
        id: None,
        words: None,
    };
    e.prepare(call).map(|p| p.tree)
}

fn find<'a>(n: &'a LNode, key: &str) -> Option<&'a LNode> {
    if n.key.as_deref() == Some(key) {
        return Some(n);
    }
    n.children.iter().find_map(|c| find(c, key))
}

fn keys(n: &LNode, out: &mut Vec<String>) {
    out.extend(n.key.clone());
    for c in &n.children {
        keys(c, out);
    }
}

const FACTOR: &str = r#"
export const props = {};
export default (_p, { t }) => (
  <Stack align="center" justify="center" style={{ width: "100%", height: "100%" }}>
    <Morph p={t}>
      <Math key="eq" tex="2^4 - 1" style={{ fontSize: 40 }} />
      <Math key="eq" tex="(2^2 - 1)(2^2 + 1)" style={{ fontSize: 40 }} />
    </Morph>
  </Stack>
);
"#;

#[test]
fn shared_terms_glide_and_the_rest_fade() {
    let mut e = engine(FACTOR);
    let none = Map::new();
    let a = prepare(&mut e, 0.0, &none).unwrap();
    let mid = prepare(&mut e, 0.5, &none).unwrap();
    let b = prepare(&mut e, 1.0, &none).unwrap();
    // At p = 0 the tree is A's layout with B's own terms still invisible.
    let mut ka = Vec::new();
    keys(&a, &mut ka);
    assert_eq!(ka.len(), 1 + 4 + 12 - 3, "{ka:?}");
    assert_eq!(find(&a, "eq/(").unwrap().style.opacity(), 0.0);
    for key in ["eq/2", "eq/−", "eq/1"] {
        let (x0, x1) = (find(&a, key).unwrap().rect, find(&b, key).unwrap().rect);
        let m = find(&mid, key).unwrap();
        let want = x0.x0 + (x1.x0 - x0.x0) * 0.5;
        assert!(
            (m.rect.x0 - want).abs() < 1e-3,
            "{key} at {} not {want}",
            m.rect.x0
        );
        assert_eq!(m.style.opacity(), 1.0, "{key}");
        assert_ne!(x0.x0, x1.x0, "{key} should move");
    }
    // Mid-way A's exponent is fading out as B's parentheses fade in, and
    // it rides out beside the 2 it was on.
    let four = find(&mid, "eq/4").unwrap();
    assert!(four.style.opacity() > 0.0 && four.style.opacity() < 1.0);
    let paren = find(&mid, "eq/(").unwrap().style.opacity();
    assert!(paren > 0.0 && paren < 1.0, "{paren}");
    let beside = |t: &LNode| find(t, "eq/4").unwrap().rect.x0 - find(t, "eq/2").unwrap().rect.x0;
    assert!((beside(&mid) - beside(&a)).abs() < 1e-3);
}

#[test]
fn a_math_without_a_key_in_a_morph_is_keyed_by_its_place() {
    let tsx = r#"
export const props = {};
export default (_p, { t }) => (
  <Box>
    <Math tex="x" />
    <Morph p={t}>
      <Row><Math tex="a + b" /><Math tex="x" /></Row>
      <Row><Math tex="b + a" /></Row>
    </Morph>
  </Box>
);
"#;
    let mut e = engine(tsx);
    let a = prepare(&mut e, 0.0, &Map::new()).unwrap();
    let b = prepare(&mut e, 1.0, &Map::new()).unwrap();
    let mut ka = Vec::new();
    keys(&a, &mut ka);
    // The Math outside the Morph keys nothing; inside, the nth Math of
    // each side shares a prefix, so `a` and `b` swap places.
    ka.sort();
    assert_eq!(ka, ["~0.0/+", "~0.0/a", "~0.0/b", "~0.1/x"]);
    let x = |t: &LNode, k: &str| find(t, k).unwrap().rect.x0;
    assert!(x(&a, "~0.0/a") < x(&a, "~0.0/b"));
    assert!(x(&b, "~0.0/a") > x(&b, "~0.0/b"));
}

#[test]
fn baseline_alignment_puts_text_and_maths_on_one_line() {
    let tsx = r#"
export const props = {};
export default () => (
  <Row align="baseline" style={{ padding: 20 }}>
    <Text key="t" style={{ fontSize: 30 }}>so</Text>
    <Math key="m" tex="\frac{a}{b} = x" style={{ fontSize: 30 }} />
  </Row>
);
"#;
    let mut e = engine(tsx);
    let tree = prepare(&mut e, 0.0, &Map::new()).unwrap();
    let text = find(&tree, "t").unwrap();
    let (_, layout) = text.text.as_ref().unwrap();
    let text_base = text.content.y0 + f64::from(layout.lines().next().unwrap().metrics().baseline);
    // The x sits on the formula's baseline: its rect's bottom is its depth
    // (0 for x) below it.
    let x = find(&tree, "m/x").unwrap();
    assert!(
        (x.rect.y1 - text_base).abs() < 0.05,
        "{} vs {text_base}",
        x.rect.y1
    );
}

#[test]
fn errors_name_the_element_and_katex_s_position() {
    let tsx = r#"
export const props = {};
export default () => <Box><Math tex="\frac{a}{" /></Box>;
"#;
    let mut e = engine(tsx);
    let err = prepare(&mut e, 0.0, &Map::new()).err().unwrap().to_string();
    assert!(
        err.contains("Box > Math[0]") && err.contains("at end of input"),
        "{err}"
    );

    let tsx = r#"
export const props = {};
export default () => <Math tex="x" style={{ padding: 8 }} />;
"#;
    let err = prepare(&mut engine(tsx), 0.0, &Map::new())
        .err()
        .unwrap()
        .to_string();
    assert!(err.contains("style.padding"), "{err}");

    let tsx = r#"
export const props = {};
export default () => <Math tex="\color{red}{x}" />;
"#;
    let err = prepare(&mut engine(tsx), 0.0, &Map::new())
        .err()
        .unwrap()
        .to_string();
    assert!(
        err.contains(r"\color{red}") && err.contains("accent"),
        "{err}"
    );
}

#[test]
fn theme_colours_and_type_tokens_work() {
    let tsx = r#"
export const props = {};
export default (_p, { theme }) => (
  <Math key="m" tex="\textcolor{accent}{x} + y" style={{ ...theme.type.body, color: theme.color.muted }} />
);
"#;
    let mut e = engine(tsx);
    let tree = prepare(&mut e, 0.0, &Map::new()).unwrap();
    let fill = |key: &str| {
        find(&tree, key)
            .unwrap()
            .path
            .as_ref()
            .unwrap()
            .fill
            .clone()
    };
    let solid = |hex: &str| {
        Some(crate::scene::Fill::Solid(
            crate::scene::Color::parse(hex).unwrap(),
        ))
    };
    assert_eq!(fill("m/x"), solid("#5e6ad2"));
    assert_eq!(fill("m/y"), solid("#6b6e7a"));
    // 32 px body text, set 1.21 times larger, as KaTeX does.
    let m = find(&tree, "m").unwrap();
    let f = crate::math::typeset(r"\textcolor{accent}{x} + y", true).unwrap();
    assert!((m.rect.width() - f.width * 32.0 * 1.21).abs() < 1e-3);
}

#[test]
fn math_is_still_javascript_s_math() {
    let tsx = r#"
export const props = {};
export default () => <Math tex={String(Math.max(2, 3))} key="m" />;
"#;
    let tree = prepare(&mut engine(tsx), 0.0, &Map::new()).unwrap();
    assert!(find(&tree, "m/3").is_some());
}

#[test]
fn a_latex_prop_is_checked_by_typesetting_it() {
    let tsx = r#"
export const props = { tex: latex() };
export default ({ tex }) => <Math tex={tex} />;
"#;
    let mut e = engine(tsx);
    let ok = json!({ "tex": r"\color{accent}{\sqrt{2}}" });
    assert!(prepare(&mut e, 0.0, ok.as_object().unwrap()).is_ok());
    let bad = json!({ "tex": r"\sqrt{2" });
    let err = prepare(&mut e, 0.0, bad.as_object().unwrap())
        .err()
        .unwrap();
    assert!(matches!(err, StudioError::Props { .. }), "{err}");
    assert!(err.to_string().contains("end of input"), "{err}");
}
