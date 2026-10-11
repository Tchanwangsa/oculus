//! Text gliding 0.25 px per frame must move smoothly: its ink centroid
//! follows a straight line, with no whole-pixel steps (a glyph atlas or
//! pixel snapping would show up here as ~0.4 px residuals).
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

use serde_json::{Map, json};
use studio_core::{Engine, RenderOptions, SceneCall, Size, Theme};

const COMPONENT: &str = r##"export const props = { dx: num(), dy: num() };
export default ({ dx, dy }) => (
  <Box style={{ fill: "#ffffff", padding: 40 }}>
    <Text style={{ fontFamily: "Inter", fontSize: 40, fontWeight: 500, color: "#000000", translateX: dx, translateY: dy }}>
      Gliding text
    </Text>
  </Box>
);
"##;

fn centroids(axis_y: bool) -> Vec<f64> {
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
    e.load_component("Glide", COMPONENT).unwrap();
    (0..12)
        .map(|k| {
            let d = f64::from(k) * 0.25;
            let props: Map<_, _> = if axis_y {
                json!({ "dx": 0, "dy": d })
            } else {
                json!({ "dx": d, "dy": 0 })
            }
            .as_object()
            .cloned()
            .unwrap();
            let f = e
                .render(SceneCall {
                    component: "Glide",
                    props: &props,
                    t: 0.0,
                    duration: 1.0,
                    index: 0,
                })
                .unwrap();
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
        })
        .collect()
}

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
