//! Text gliding 0.25 px per frame must move smoothly: its ink centroid
//! follows a straight line, with no whole-pixel steps (a glyph atlas or
//! pixel snapping would show up here as ~0.4 px residuals). The same holds
//! for a word rising in through `reveal`, for text under a moving Camera,
//! and for text carried by a push transition or a magic move.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

use serde_json::{Map, Value, json};
use studio_core::{Engine, Frame, Project, RenderOptions, Renderer, SceneCall, Size, Theme, Time};

const COMPONENT: &str = r##"export const props = { dx: num(), dy: num() };
export default ({ dx, dy }) => (
  <Box style={{ fill: "#ffffff", padding: 40 }}>
    <Text style={{ fontFamily: "Inter", fontSize: 40, fontWeight: 500, color: "#000000", translateX: dx, translateY: dy }}>
      Gliding text
    </Text>
  </Box>
);
"##;

const REVEAL: &str = r##"export const props = { p: num() };
export default ({ p }) => (
  <Box style={{ fill: "#ffffff", padding: 40 }}>
    <Text style={{ fontFamily: "Inter", fontSize: 40, fontWeight: 500, color: "#000000", reveal: { by: "word", from: "rise", progress: p } }}>
      Rising
    </Text>
  </Box>
);
"##;

const CAMERA: &str = r##"export const props = { y: num() };
export default ({ y }) => (
  <Camera y={y} zoom={1.2}>
    <Box style={{ flexGrow: 1, fill: "#ffffff", padding: 40 }}>
      <Text style={{ fontFamily: "Inter", fontSize: 40, fontWeight: 500, color: "#000000" }}>Panning</Text>
    </Box>
  </Camera>
);
"##;

fn centroids(axis_y: bool) -> Vec<f64> {
    let props = |d: f64| {
        if axis_y {
            json!({ "dx": 0, "dy": d })
        } else {
            json!({ "dx": d, "dy": 0 })
        }
    };
    ink_centroids(COMPONENT, axis_y, |k| props(f64::from(k) * 0.25))
}

/// The ink centroid along one axis over 12 frames, frame k with `props(k)`.
fn ink_centroids(tsx: &str, axis_y: bool, props: impl Fn(u32) -> serde_json::Value) -> Vec<f64> {
    let mut e = Engine::new(
        Theme::bundled("default").unwrap(),
        Size {
            width: 400,
            height: 140,
        },
        None,
        RenderOptions::default(),
    )
    .unwrap();
    e.load_component("Glide", tsx).unwrap();
    (0..12)
        .map(|k| {
            let props: Map<_, _> = props(k).as_object().cloned().unwrap();
            let f = e
                .render(SceneCall {
                    component: "Glide",
                    props: &props,
                    t: 0.0,
                    duration: 1.0,
                    index: 0,
                })
                .unwrap();
            centroid(&f, axis_y)
        })
        .collect()
}

/// The ink centroid of black on white along one axis.
fn centroid(f: &Frame, axis_y: bool) -> f64 {
    let (mut s, mut m) = (0.0, 0.0);
    for y in 0..f.height {
        for x in 0..f.width {
            let i = ((y * f.width + x) * 4) as usize;
            let ink = 255.0 - f64::from(f.rgba[i]);
            s += ink * (f64::from(if axis_y { y } else { x }) + 0.5);
            m += ink;
        }
    }
    s / m
}

/// A two-scene 400x140 video with `transition` into scene 1; the vertical
/// ink centroid of scene 1 at each `t`. The canvas is white like the
/// scenes, so the antialiased seam where two pushed scenes meet has no ink.
fn transition_centroids(name: &str, a: &str, b: &str, transition: Value, ts: &[f64]) -> Vec<f64> {
    let dir = std::env::temp_dir().join(format!("studio-subpixel-{name}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("components")).unwrap();
    let theme = include_str!("../themes/default.json")
        .replace("\"bg\": \"#f4f4f6\"", "\"bg\": \"#ffffff\"");
    std::fs::write(dir.join("white.json"), theme).unwrap();
    let video = json!({
        "size": { "width": 400, "height": 140 },
        "theme": "white.json",
        "scenes": [
            { "component": "A", "hold": 2.0 },
            { "component": "B", "hold": 2.0, "transition": transition }
        ]
    });
    std::fs::write(dir.join("video.json"), video.to_string()).unwrap();
    std::fs::write(dir.join("components/A.tsx"), a).unwrap();
    std::fs::write(dir.join("components/B.tsx"), b).unwrap();
    let project = Project::load(&dir).unwrap();
    let mut r = Renderer::new(&project, RenderOptions::default()).unwrap();
    ts.iter()
        .map(|&t| centroid(&r.frame(Time::Scene { index: 1, t }).unwrap(), true))
        .collect()
}

const LINE: &str = r##"export const props = {};
export default () => (
  <Box style={{ fill: "#ffffff", padding: 40 }}>
    <Text key="line" style={{ fontFamily: "Inter", fontSize: 40, fontWeight: 500, color: "#000000" }}>Moving text</Text>
  </Box>
);
"##;

/// (max |residual| from the least-squares line, slope)
fn fit(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let mx = (n - 1.0) / 2.0;
    let my = v.iter().sum::<f64>() / n;
    let (mut num, mut den) = (0.0, 0.0);
    for (k, y) in v.iter().enumerate() {
        num += (k as f64 - mx) * (y - my);
        den += (k as f64 - mx).powi(2);
    }
    let slope = num / den;
    let worst = v
        .iter()
        .enumerate()
        .map(|(k, y)| (y - (my + slope * (k as f64 - mx))).abs())
        .fold(0.0, f64::max);
    (worst, slope)
}

#[test]
fn vertical_glide_has_no_pixel_steps() {
    let (worst, slope) = fit(&centroids(true));
    assert!((slope - 0.25).abs() < 0.01, "slope {slope}");
    println!("worst deviation {worst:.4} px, slope {slope:.4}");
    assert!(worst < 0.05, "worst deviation {worst} px");
}

#[test]
fn horizontal_glide_has_no_pixel_steps() {
    let (worst, slope) = fit(&centroids(false));
    assert!((slope - 0.25).abs() < 0.01, "slope {slope}");
    println!("worst deviation {worst:.4} px, slope {slope:.4}");
    assert!(worst < 0.05, "worst deviation {worst} px");
}

#[test]
fn a_word_rising_in_has_no_pixel_steps() {
    // Rise is (1 - eased p) * 0.35 em: pick p so the lift falls 0.25 px a
    // frame (6 px down to 3.25 px), while the fade goes from 57% to 77%.
    let lift = |k: u32| 6.0 - f64::from(k) * 0.25;
    let ys = ink_centroids(
        REVEAL,
        true,
        |k| json!({ "p": 1.0 - (lift(k) / (0.35 * 40.0)).cbrt() }),
    );
    let off: Vec<f64> = ys
        .iter()
        .enumerate()
        .map(|(k, y)| y - lift(k as u32))
        .collect();
    let (worst, slope) = fit(&off);
    println!("rise: worst deviation {worst:.4} px, slope {slope:.4}");
    assert!(slope.abs() < 0.01, "slope {slope}");
    assert!(worst < 0.05, "worst deviation {worst} px");
}

#[test]
fn text_under_a_panning_camera_has_no_pixel_steps() {
    // The camera's point moves up 0.25 px a frame; at zoom 1.2 the text
    // moves down 0.3 px a frame.
    let ys = ink_centroids(CAMERA, true, |k| json!({ "y": 70.0 - f64::from(k) * 0.25 }));
    let (worst, slope) = fit(&ys);
    println!("camera: worst deviation {worst:.4} px, slope {slope:.4}");
    assert!((slope - 0.3).abs() < 0.01, "slope {slope}");
    assert!(worst < 0.05, "worst deviation {worst} px");
}

#[test]
fn text_pushed_in_has_no_pixel_steps() {
    // B comes up from below over 1.4 s, linear: 140 px / 1.4 s, so 0.25 px
    // every 2.5 ms. A is blank white.
    let blank =
        "export const props = {};\nexport default () => <Box style={{ fill: \"#ffffff\" }} />;\n";
    let ts: Vec<f64> = (0..12).map(|k| 1.2 + f64::from(k) * 0.0025).collect();
    let push = json!({ "kind": "push", "direction": "up", "dur": 1.4, "easing": "linear" });
    let (worst, slope) = fit(&transition_centroids("push", blank, LINE, push, &ts));
    println!("push: worst deviation {worst:.4} px, slope {slope:.4}");
    assert!((slope + 0.25).abs() < 0.01, "slope {slope}");
    assert!(worst < 0.05, "worst deviation {worst} px");
}

#[test]
fn text_in_a_magic_move_has_no_pixel_steps() {
    // The same keyed line 40 px lower in B, over 1 s, linear: 0.25 px every
    // 6.25 ms.
    let lower = LINE.replace("padding: 40", "padding: [80, 40, 0, 40]");
    let ts: Vec<f64> = (0..12).map(|k| 0.3 + f64::from(k) * 0.00625).collect();
    let magic = json!({ "kind": "magic", "dur": 1.0, "easing": "linear" });
    let (worst, slope) = fit(&transition_centroids("magic", LINE, &lower, magic, &ts));
    println!("magic: worst deviation {worst:.4} px, slope {slope:.4}");
    assert!((slope - 0.25).abs() < 0.01, "slope {slope}");
    assert!(worst < 0.05, "worst deviation {worst} px");
}
