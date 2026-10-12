//! Overlays drawn on prepared trees: they find their targets after the
//! morph, so they follow terms that move, and they fail with the closest key.

use crate::error::StudioError;
use crate::layout::LNode;
use crate::render::{Engine, RenderOptions, SceneCall, Size};
use crate::scene::Kind;
use crate::theme::Theme;
use serde_json::Map;
use vello_cpu::kurbo::{Rect, Shape};

fn prepare(tsx: &str, t: f64) -> Result<LNode, StudioError> {
    let theme = Theme::bundled("default").unwrap();
    let size = Size {
        width: 640,
        height: 360,
    };
    let mut e = Engine::new(theme, size, None, RenderOptions::default()).unwrap();
    e.load_component("F", tsx).unwrap();
    let props = Map::new();
    let call = SceneCall {
        component: "F",
        props: &props,
        t,
        duration: 2.0,
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

const FACTOR: &str = r#"
export const props = {};
export default () => (
  <Stack align="center" justify="center" style={{ width: "100%", height: "100%" }}>
    <Steps at={[0]} dur={1} ease={linear}>
      <Math key="eq" tex="2^4 - 1" style={{ fontSize: 40 }} />
      <Math key="eq" tex="(2^2 - 1)(2^2 + 1)" style={{ fontSize: 40 }} />
    </Steps>
    <Surround key="s" target="eq/1" pad={5} />
    <Brace key="b" target="eq" label="n" />
  </Stack>
);
"#;

/// The Surround's outline sits `pad` outside the 1's box at every frame
/// of the morph, so it moves with it.
#[test]
fn a_surround_follows_its_target_through_a_morph() {
    let mut seen = Vec::new();
    for t in [0.0, 0.5, 1.0] {
        let tree = prepare(FACTOR, t).unwrap();
        let one = find(&tree, "eq/1").unwrap().rect;
        let s = find(&tree, "s").unwrap();
        assert_eq!(s.kind, Kind::Box);
        let path = s.children[0].path.as_ref().unwrap();
        let drawn = path.d.bounding_box();
        let want = one.inflate(5.0, 5.0);
        let close = |a: Rect, b: Rect| {
            (a.x0 - b.x0).abs() < 1e-6
                && (a.y0 - b.y0).abs() < 1e-6
                && (a.x1 - b.x1).abs() < 1e-6
                && (a.y1 - b.y1).abs() < 1e-6
        };
        assert!(close(drawn, want), "t {t}: {drawn:?} not {want:?}");
        seen.push(drawn.x0);
    }
    assert!(seen[0] != seen[2], "the 1 moves, so its surround does");
}

/// The brace spans the Math box and its label hangs below it, centred.
#[test]
fn a_brace_sits_under_its_target_with_its_label_below() {
    let tree = prepare(FACTOR, 1.0).unwrap();
    let eq = find(&tree, "eq").unwrap().rect;
    let b = find(&tree, "b").unwrap();
    let brace = b.children[0].path.as_ref().unwrap().d.bounding_box();
    assert!(brace.y0 > eq.y1, "{brace:?} under {eq:?}");
    assert!((brace.center().x - eq.center().x).abs() < 0.5);
    assert!((brace.width() - eq.width()).abs() < 1.0, "{brace:?} {eq:?}");
    let label = b.children[1].rect;
    assert!(label.y0 > brace.y1);
    assert!((label.center().x - eq.center().x).abs() < 0.5);
}

#[test]
fn indicate_scales_and_tints_its_target_mid_pulse() {
    let tsx = r#"
export const props = {};
export default () => (
  <Stack align="center" justify="center" style={{ width: "100%", height: "100%" }}>
    <Math key="eq" tex="a + b" style={{ fontSize: 40 }} />
    <Indicate target="eq/a" progress={0.5} />
  </Stack>
);
"#;
    let tree = prepare(tsx, 0.0).unwrap();
    let a = find(&tree, "eq/a").unwrap();
    assert!((a.style.scale.unwrap() - 1.2).abs() < 1e-6);
    let b = find(&tree, "eq/b").unwrap();
    assert!(b.style.scale.is_none());
}

#[test]
fn a_missing_target_names_the_closest_key() {
    let tsx = r#"
export const props = {};
export default () => (
  <Box><Math key="eq" tex="a + b" /><Underline target="eq/c" /></Box>
);
"#;
    let Err(e) = prepare(tsx, 0.0) else {
        panic!("a missing target should fail");
    };
    let e = e.to_string();
    assert!(
        e.contains("<Underline> target `eq/c` matches no keyed node"),
        "{e}"
    );
    assert!(
        e.contains("did you mean `eq/a`") || e.contains("did you mean `eq/b`"),
        "{e}"
    );
}
