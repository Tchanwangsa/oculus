//! Lazy resolution: components run top-down with their subtree's ctx, so
//! Sequence, Stagger, Enter and Exit shift or shape time for their children.
//! `CALL` is t = 0.5 of a 2 s scene.

use super::{call, host};
use serde_json::Value;
use vello_cpu::kurbo::{BezPath, ParamCurve};

fn tree(name: &str, tsx: &str) -> Value {
    let mut h = host();
    h.load(name, tsx).unwrap();
    call(&mut h, name).unwrap().0
}

fn texts(v: &Value) -> Vec<String> {
    v["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["text"].as_str().unwrap_or("").to_owned())
        .collect()
}

const PROBES: &str = r##"export const props = {};
const Probe = (_p, { t, duration }) => <Text>{t} {duration}</Text>;
export default () => (
  <Stack>
    <Sequence from={0.25}><Probe /></Sequence>
    <Sequence from={0.125} dur={1}><Sequence from={0.125}><Probe /></Sequence></Sequence>
    <Sequence from={1}><Probe /></Sequence>
    <Sequence dur={0.25}><Probe /></Sequence>
    <Sequence dur={0.25} hold><Probe /></Sequence>
    <Stagger each={0.25}><Probe /><><Probe /><Probe /></></Stagger>
    <Sequence from={0.25}>{({ t }) => <Text>{t}</Text>}</Sequence>
    <Sequence from={0.25}><><Box key="a" /><Box key="b" /></></Sequence>
    <Probe key="p" />
  </Stack>
);
"##;

#[test]
fn sequences_and_staggers_shift_time_for_their_subtree() {
    let t = tree("Probes", PROBES);
    assert_eq!(
        texts(&t),
        [
            "0.25 1.75",
            "0.25 0.875",
            "0 1",
            "0.25 0.25",
            "0.25 0.25",
            "0.5 2",
            "0.25 1.75",
            "0 1.5",
            "0.25",
            "",
            "",
            "0.5 2",
        ]
    );
    let kids = t["children"].as_array().unwrap();
    let opacity = |i: usize| kids[i]["style"]["opacity"].as_f64();
    // Outside its window a subtree keeps its slot, hidden.
    assert_eq!(opacity(2), Some(0.0), "before the window");
    assert_eq!(opacity(3), Some(0.0), "after the window");
    assert_eq!(opacity(4), None, "held");
    assert_eq!(opacity(0), None);
    // A Fragment inside a Sequence flattens into the parent's children.
    assert_eq!(
        (kids[9]["key"].as_str(), kids[10]["key"].as_str()),
        (Some("a"), Some("b"))
    );
    // A function component's key lands on the host node it resolves to.
    assert_eq!(kids[11]["key"], "p");
}

#[test]
fn text_still_rejects_a_component_child() {
    let mut h = host();
    h.load(
        "Nested",
        "export const props = {};\nconst Probe = () => <Text>x</Text>;\nexport default () =>\n  <Text>a <Probe /></Text>;\n",
    )
    .unwrap();
    let e = call(&mut h, "Nested").unwrap_err().to_string();
    assert!(
        e.contains("<Text> children must be strings or numbers, got a <Probe> element"),
        "{e}"
    );
    assert!(e.starts_with("Nested.tsx:4:"), "{e}");
}

#[test]
fn errors_from_built_ins_point_at_their_element() {
    let mut h = host();
    h.load(
        "Neg",
        "export const props = {};\nconst Inner = () => (\n  <Sequence dur={-1}><Box /></Sequence>\n);\nexport default () => <Stack><Inner /></Stack>;\n",
    )
    .unwrap();
    let e = call(&mut h, "Neg").unwrap_err();
    assert_eq!(e.loc().map(|l| l.line), Some(3), "{e}");
    assert!(
        e.to_string()
            .contains("<Sequence> dur must be a finite number of seconds, 0 or more, got -1"),
        "{e}"
    );
    // A throw in the component's own code keeps its own line.
    h.load(
        "Own",
        "export const props = {};\nconst Probe = () => {\n  throw new Error('probe broke');\n};\nexport default () => <Stack><Sequence><Probe /></Sequence></Stack>;\n",
    )
    .unwrap();
    let e = call(&mut h, "Own").unwrap_err();
    assert_eq!(e.loc().map(|l| l.line), Some(3), "{e}");
}

#[test]
fn enter_and_exit_compose_onto_roots() {
    let t = tree(
        "Presets",
        r##"export const props = {};
export default () => (
  <Stack>
    <Enter preset="rise" dur={1} easing={linear}><Box style={{ opacity: 0.5, translateY: 10 }} /></Enter>
    <Enter preset="mask" dur={1} easing={linear}><Box /></Enter>
    <Enter preset="scale" at={1}><Box /></Enter>
    <Exit preset="slide-left" dur={1}><Box /></Exit>
    <Exit preset="fade" at={0} dur={1} easing={linear}><Box /><Box /></Exit>
  </Stack>
);
"##,
    );
    let kids = t["children"].as_array().unwrap();
    let style = |i: usize, k: &str| kids[i]["style"][k].as_f64();
    assert_eq!(style(0, "opacity"), Some(0.25));
    let ty = style(0, "translateY").unwrap();
    assert!((ty - (10.0 + 0.5 * 0.03 * 1080.0)).abs() < 1e-9, "{ty}");
    assert_eq!(
        kids[1]["style"]["clipInset"],
        serde_json::json!([0, 0.5, 0, 0])
    );
    assert_eq!(style(2, "opacity"), Some(0.0), "before `at`");
    assert_eq!(style(2, "scale"), Some(0.9));
    // An Exit defaults to ending with the scene: at 0.5 of 2 s it is in place.
    assert_eq!(style(3, "opacity"), Some(1.0));
    assert_eq!(style(3, "translateX"), None);
    assert_eq!(
        (style(4, "opacity"), style(5, "opacity")),
        (Some(0.5), Some(0.5))
    );
}

#[test]
fn unknown_presets_are_named() {
    let mut h = host();
    h.load(
        "Bad",
        "export const props = {};\nexport default () => <Stack>\n  <Enter preset=\"zoom\"><Box /></Enter>\n</Stack>;\n",
    )
    .unwrap();
    let e = call(&mut h, "Bad").unwrap_err();
    assert_eq!(e.loc().map(|l| l.line), Some(3), "{e}");
    assert!(
        e.to_string()
            .contains("<Enter> has no preset \"zoom\"; presets: fade, rise, slide-left, slide-right, scale, blur, mask"),
        "{e}"
    );
}

#[test]
fn camera_centres_its_point() {
    let t = tree(
        "Cam",
        "export const props = {};\nexport default () => <Camera x={480} y={270} zoom={2}><Box /></Camera>;\n",
    );
    let s = &t["style"];
    assert_eq!(s["transformOrigin"], serde_json::json!([0.25, 0.25]));
    assert_eq!(
        (s["translateX"].as_f64(), s["translateY"].as_f64()),
        (Some(480.0), Some(270.0))
    );
    assert_eq!(
        (s["scale"].as_f64(), s["width"].as_str()),
        (Some(2.0), Some("100%"))
    );
}

#[test]
fn arrows_are_a_trimmed_shaft_and_a_head() {
    let t = tree(
        "Arr",
        "export const props = {};\nexport default () => <Box><Arrow key=\"a\" points={[[0, 0], [100, 0]]} progress={0.5} stroke={{ width: 4, color: '#ff0000' }} /></Box>;\n",
    );
    let kids = t["children"].as_array().unwrap();
    assert_eq!(kids.len(), 2);
    assert_eq!(kids[0]["k"], "path");
    assert_eq!(kids[0]["trim"], serde_json::json!([0, 0.5]));
    assert_eq!(kids[0]["key"], "a:shaft");
    assert_eq!(kids[0]["viewBox"], serde_json::json!([0, 0, 1920, 1080]));
    // The shaft stops 0.6 head-lengths short of the tip (head 18 px).
    let d = kids[0]["d"].as_str().unwrap();
    let end: f64 = d.split(' ').nth(4).unwrap().parse().unwrap();
    assert!(
        d.starts_with("M 0 0 L ") && (end - 89.2).abs() < 1e-9,
        "{d}"
    );
    assert_eq!(kids[1]["key"], "a:head");
    assert_eq!(kids[1]["fill"], "#ff0000");
}

/// Where the painter's trim of a resolved shaft ends.
fn drawn_end(shaft: &Value) -> (f64, f64) {
    let d = BezPath::from_svg(shaft["d"].as_str().unwrap()).unwrap();
    let to = shaft["trim"][1].as_f64().unwrap();
    let pieces = crate::paint::trim(&d, 0.0, to);
    let end = pieces.last().unwrap().path.segments().last().unwrap().end();
    (end.x, end.y)
}

/// At every progress the shaft's drawn end is where the head says it is:
/// `back` behind the tip along the head's axis, which is `back / size` of
/// the way from the tip to the base centre whatever the head's scale.
#[test]
fn the_arrow_head_rides_the_drawn_end_of_the_shaft() {
    // (points, curve, stroke width): straight; the loop (down, a long run
    // left, a short run up); a curve.
    let cases: [(&str, f64, f64); 3] = [
        ("[[100, 100], [900, 400]]", 0.0, 4.0),
        (
            "[[1380, 560], [1380, 680], [540, 680], [540, 560]]",
            0.0,
            8.0,
        ),
        (
            "[[300, 800], [800, 300], [1300, 700], [1700, 400]]",
            1.0,
            6.0,
        ),
    ];
    let steps = 200;
    let mut worst = 0.0f64;
    for (points, curve, width) in cases {
        let pts: Vec<[f64; 2]> = serde_json::from_str(points).unwrap();
        let [a, b] = [pts[pts.len() - 2], pts[pts.len() - 1]];
        let size = width * 4.5;
        let back = (size * 0.6).min((b[0] - a[0]).hypot(b[1] - a[1]) * 0.9);
        let arrows: String = (0..=steps)
            .map(|i| {
                let p = f64::from(i) / f64::from(steps);
                format!("<Arrow points={{{points}}} curve={{{curve}}} progress={{{p}}} stroke={{{{ width: {width} }}}} />\n")
            })
            .collect();
        let t = tree(
            "Ride",
            &format!("export const props = {{}};\nexport default () => <Box>\n{arrows}</Box>;\n"),
        );
        let kids = t["children"].as_array().unwrap();
        assert_eq!(kids.len(), 2 * (steps as usize + 1));
        for (i, pair) in kids.chunks(2).enumerate() {
            let hd = path_numbers(pair[1]["d"].as_str().unwrap());
            let tip = (hd[0], hd[1]);
            let base = ((hd[2] + hd[4]) / 2.0, (hd[3] + hd[5]) / 2.0);
            let f = back / size;
            let attach = (tip.0 + (base.0 - tip.0) * f, tip.1 + (base.1 - tip.1) * f);
            let gap = if i == 0 {
                // Nothing is drawn yet, and the head is a point at the start.
                (tip.0 - pts[0][0]).hypot(tip.1 - pts[0][1])
            } else {
                let end = drawn_end(&pair[0]);
                (end.0 - attach.0).hypot(end.1 - attach.1)
            };
            worst = worst.max(gap);
            assert!(gap < 0.5, "{points} at {i}/{steps}: gap {gap} px");
        }
        // At 1 the tip is the last point.
        let hd = path_numbers(kids[kids.len() - 1]["d"].as_str().unwrap());
        let miss = (hd[0] - b[0]).hypot(hd[1] - b[1]);
        assert!(miss < 0.5, "{points}: tip {miss} px off the last point");
    }
    eprintln!("worst head gap {worst:e} px");
}

#[test]
fn the_ctx_a_subtree_sees_is_frozen() {
    let t = tree(
        "Frozen",
        "export const props = {};\nconst Probe = (_p, ctx) => { try { ctx.t = 9; } catch (_) {} return <Text>{ctx.t} {String(Object.isFrozen(ctx))}</Text>; };\nexport default () => <Stack><Sequence from={0.25}><Probe /></Sequence></Stack>;\n",
    );
    assert_eq!(texts(&t), ["0.25 true"]);
}

#[test]
fn a_loop_inside_a_nested_component_hits_the_step_budget() {
    let mut h = host();
    h.load(
        "Spin",
        "export const props = {};\nconst Spin = () => { while (true) {} };\nexport default () => <Stack><Sequence from={0.25}><Spin /></Sequence></Stack>;\n",
    )
    .unwrap();
    let e = call(&mut h, "Spin").unwrap_err();
    assert!(matches!(e, crate::error::StudioError::Budget { .. }), "{e}");
}

/// The numbers of an SVG path string, commands dropped.
fn path_numbers(d: &str) -> Vec<f64> {
    d.split(' ').filter_map(|w| w.parse().ok()).collect()
}

/// Whether two 2D directions point the same way.
fn same_direction(a: (f64, f64), b: (f64, f64)) -> bool {
    let cross = a.0 * b.1 - a.1 * b.0;
    let dot = a.0 * b.0 + a.1 * b.1;
    dot > 0.0 && cross.abs() < 1e-9 * (a.0.hypot(a.1) * b.0.hypot(b.1))
}

#[test]
fn curved_arrows_have_natural_ends_and_the_head_follows_the_shaft() {
    let t = tree(
        "Curve",
        "export const props = {};\nexport default () => <Box>\n  <Arrow points={[[0, 0], [100, 0], [200, 100]]} curve={1} head={0} />\n  <Arrow points={[[0, 0], [100, 0], [200, 100]]} curve={1} />\n  <Arrow points={[[0, 0], [90, 30]]} curve={1} head={0} />\n</Box>;\n",
    );
    let kids = t["children"].as_array().unwrap();
    // Interior tangent m1 = (p2 - p0) / 2 = (100, 50); natural ends give
    // m0 = 1.5 (p1 - p0) - 0.5 m1 = (100, -25) and m2 = 1.5 (p2 - p1) - 0.5 m1
    // = (100, 125).
    let n = path_numbers(kids[0]["d"].as_str().unwrap());
    assert!(
        same_direction((n[2] - n[0], n[3] - n[1]), (100.0, -25.0)),
        "{n:?}"
    );
    let (c2, end) = (
        (n[n.len() - 4], n[n.len() - 3]),
        (n[n.len() - 2], n[n.len() - 1]),
    );
    assert!(
        same_direction((end.0 - c2.0, end.1 - c2.1), (100.0, 125.0)),
        "{n:?}"
    );
    // With a head the shaft is pulled back; the head's axis (base centre to
    // tip) is the drawn shaft's end tangent.
    let s = path_numbers(kids[1]["d"].as_str().unwrap());
    let shaft = (
        s[s.len() - 2] - s[s.len() - 4],
        s[s.len() - 1] - s[s.len() - 3],
    );
    let hd = path_numbers(kids[2]["d"].as_str().unwrap());
    let base = ((hd[2] + hd[4]) / 2.0, (hd[3] + hd[5]) / 2.0);
    assert!(
        same_direction((hd[0] - base.0, hd[1] - base.1), shaft),
        "{hd:?} vs {shaft:?}"
    );
    // Two points with a curve are a straight line.
    let l = path_numbers(kids[3]["d"].as_str().unwrap());
    for k in [2, 4] {
        assert!(same_direction((l[k], l[k + 1]), (90.0, 30.0)), "{l:?}");
    }
}

#[test]
fn morph_is_a_host_node_with_two_resolved_children() {
    let mut h = host();
    h.load(
        "M",
        "export const props = {};\nconst Two = () => <><Box /><Box /></>;\nexport default () => <Morph p={1.4}><Text>a</Text><Row /></Morph>;\n",
    )
    .unwrap();
    let (tree, _) = call(&mut h, "M").unwrap();
    assert_eq!(tree["k"], "morph");
    assert_eq!(tree["p"], 1.0);
    assert_eq!(tree["children"].as_array().unwrap().len(), 2);
    h.load(
        "Bad",
        "export const props = {};\nconst Two = () => <><Box /><Box /></>;\nexport default () => (\n  <Morph p={0.5}><Box /><Two /></Morph>\n);\n",
    )
    .unwrap();
    let e = call(&mut h, "Bad").unwrap_err().to_string();
    assert!(
        e.starts_with("Bad.tsx:4:3: TypeError: <Morph> takes exactly two elements, the layout at p = 0 and the layout at p = 1; its children resolved to 3"),
        "{e}"
    );
}

#[test]
fn a_hidden_sequence_hides_both_layouts_of_a_morph() {
    let mut h = host();
    h.load(
        "M",
        "export const props = {};\nexport default () => <Sequence from={1}><Morph p={0.5}><Box /><Box style={{ opacity: 0.5 }} /></Morph></Sequence>;\n",
    )
    .unwrap();
    let (tree, _) = call(&mut h, "M").unwrap();
    assert_eq!(tree["k"], "morph");
    assert_eq!(tree["children"][0]["style"]["opacity"], 0.0);
    assert_eq!(tree["children"][1]["style"]["opacity"], 0.0);
}
