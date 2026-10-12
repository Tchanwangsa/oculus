//! Steps picks the pair of states by time and feeds a Morph; Write and the
//! overlays hand Rust their specs. Trees are read as the prelude returns them.

use super::{CALL, host};
use crate::host::CallCtx;
use serde_json::{Map, Value};

fn at(tsx: &str, t: f64) -> Value {
    let mut h = host();
    h.load("S", tsx).unwrap();
    h.call("S", &Map::new(), CallCtx { t, ..CALL }).unwrap().0
}

const FOUR: &str = r#"export const props = {};
export default () => (
  <Steps at={[1, 2, 3]} dur={0.5} ease={linear}>
    <Box key="s0" /><Box key="s1" />{({ t }) => <Box key="s2" />}<Box key="s3" />
  </Steps>
);
"#;

/// The pair's keys and the Morph's p.
fn pair(v: &Value) -> (String, String, f64) {
    let k = |i: usize| v["children"][i]["key"].as_str().unwrap().to_owned();
    (k(0), k(1), v["p"].as_f64().unwrap())
}

/// Which state a Morph shows when `p` is 0 or 1 (`None` in between).
fn shown(v: &Value) -> Option<String> {
    let (a, b, p) = pair(v);
    if p <= 0.0 {
        Some(a)
    } else if p >= 1.0 {
        Some(b)
    } else {
        None
    }
}

#[test]
fn steps_picks_the_pair_by_time() {
    let cases = [
        (0.0, ("s0", "s1", 0.0)),
        (1.0, ("s0", "s1", 0.0)),
        (1.25, ("s0", "s1", 0.5)),
        (1.75, ("s0", "s1", 1.0)),
        (2.0, ("s1", "s2", 0.0)),
        (3.4, ("s2", "s3", 0.8)),
        (9.0, ("s2", "s3", 1.0)),
    ];
    for (t, (a, b, p)) in cases {
        let (ga, gb, gp) = pair(&at(FOUR, t));
        assert_eq!((ga.as_str(), gb.as_str()), (a, b), "t = {t}");
        assert!((gp - p).abs() < 1e-9, "t = {t}: p {gp} not {p}");
    }
}

#[test]
fn steps_is_continuous_at_every_cue() {
    for cue in [1.0, 2.0, 3.0] {
        let before = shown(&at(FOUR, cue - 1e-6));
        let after = shown(&at(FOUR, cue));
        assert!(
            before.is_some() && before == after,
            "at {cue}: {before:?} then {after:?}"
        );
    }
}

/// A transition longer than the gap to the next cue is cut to the gap, so
/// it still finishes before the next one starts.
#[test]
fn a_transition_is_cut_short_by_the_next_cue() {
    let tsx = FOUR.replace("dur={0.5}", "dur={4}");
    let (a, b, p) = pair(&at(&tsx, 1.5));
    assert_eq!((a.as_str(), b.as_str()), ("s0", "s1"));
    assert!((p - 0.5).abs() < 1e-9, "{p}");
    let (_, b, p) = pair(&at(&tsx, 2.0 - 1e-9));
    assert_eq!(b, "s1");
    assert!(p > 1.0 - 1e-6, "{p}");
}

#[test]
fn steps_passes_lag_fade_and_a_sampled_ease_to_the_morph() {
    let tsx = r#"export const props = {};
export default () => (
  <Steps at={[0]} lag={0.2} fade="sequential" ease={(x) => x * x}><Box /><Box /></Steps>
);
"#;
    let v = at(tsx, 0.25);
    assert_eq!(v["k"], "morph");
    assert_eq!(v["lag"], 0.2);
    assert_eq!(v["fade"], "sequential");
    let ease = v["ease"].as_array().unwrap();
    assert_eq!(ease.len(), 65);
    assert!((ease[32].as_f64().unwrap() - 0.25).abs() < 1e-12);
}

#[test]
fn write_hands_each_group_its_linear_progress() {
    let tsx = r#"export const props = {};
export default () => (
  <Write at={{ a: 0, b: 1 }} dur={{ a: 2, b: 0.5 }}><Box><Math tex="\k{a}{x} + \k{b}{y}" /></Box></Write>
);
"#;
    let v = at(tsx, 0.5);
    let w = &v["children"][0]["write"];
    assert_eq!(w["groups"], serde_json::json!([["a", 0.25], ["b", -1]]));
    assert_eq!(w["stroke"], 0.025);
}

#[test]
fn write_without_a_math_says_so() {
    let mut h = host();
    h.load(
        "S",
        "export const props = {};\nexport default () => <Write at={0}><Text>hi</Text></Write>;\n",
    )
    .unwrap();
    let e = h.call("S", &Map::new(), CALL).unwrap_err().to_string();
    assert!(e.contains("found no <Math>"), "{e}");
}

#[test]
fn overlays_are_absolute_host_nodes_with_their_progress() {
    let tsx = r#"export const props = {};
export default () => (
  <Box>
    <Brace key="b" target="eq" side="above" label="n" at={0} dur={1} ease={linear} />
    <Arrow from="eq/a" to={[10, 20]} bend={30} />
  </Box>
);
"#;
    let v = at(tsx, 0.5);
    let brace = &v["children"][0];
    assert_eq!(brace["k"], "overlay");
    assert_eq!(brace["kind"], "brace");
    assert_eq!(brace["side"], "above");
    assert_eq!(brace["progress"], 0.5);
    assert_eq!(brace["style"]["position"], "absolute");
    assert_eq!(brace["children"][0]["text"], "n");
    let arrow = &v["children"][1];
    assert_eq!(arrow["kind"], "arrow");
    assert_eq!(arrow["from"], "eq/a");
    assert_eq!(arrow["to"], serde_json::json!([10, 20]));
    assert_eq!(arrow["progress"], 1);
}
