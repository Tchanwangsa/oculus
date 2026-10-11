//! A frame is a function of its inputs alone: module-level state a component
//! mutates does not carry from one call to the next.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

use serde_json::Map;
use studio_core::{Engine, RenderOptions, SceneCall, Size, Theme};

const COUNTER: &str = r##"
let frames = 0;
export const props = {};
export const examples = {};
export default (_p, { t }) => {
  frames++;
  return <Box style={{ flexGrow: 1, fill: "#ff0000", opacity: Math.min(1, frames / 30) * t }} />;
};
"##;

fn engine() -> Engine {
    let size = Size {
        width: 64,
        height: 36,
    };
    let mut e = Engine::new(
        Theme::bundled("default").unwrap(),
        size,
        None,
        RenderOptions::default(),
    )
    .unwrap();
    e.load_component("Counter", COUNTER).unwrap();
    e
}

fn frame(e: &mut Engine, t: f64) -> Vec<u8> {
    e.render(SceneCall {
        component: "Counter",
        props: &Map::new(),
        t,
        duration: 1.0,
        index: 0,
    })
    .unwrap()
    .rgba
}

#[test]
fn module_state_does_not_depend_on_render_order() {
    let mut cold = engine();
    let first = frame(&mut cold, 1.0);

    let mut warm = engine();
    for i in 0..=30 {
        frame(&mut warm, f64::from(i) / 30.0);
    }
    let after = frame(&mut warm, 1.0);
    assert!(first == after, "t=1 differs after rendering t=0..1 first");

    // Scrubbing backwards gives the same frame as a cold render too.
    let back = frame(&mut warm, 0.5);
    assert!(back == frame(&mut engine(), 0.5));
}
