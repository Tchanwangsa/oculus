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

/// Sequenced, staggered and entering children, with module state and
/// random numbers in the mix.
const TIMELINE: &str = r##"
let calls = 0;
export const props = {};
const Bar = ({ i }, { t, duration }) => {
  calls++;
  return <Box style={{ height: 6, width: `${clamp(t / duration, 0, 1) * 100}%`, fill: "#5e6ad2", opacity: 0.5 + Math.random() / 2 + calls * 0 }} />;
};
export default () => (
  <Stack gap={2}>
    <Sequence from={0.2} dur={0.5}><Bar i={0} /></Sequence>
    <Stagger each={0.15} at={0.1}>
      {[1, 2, 3].map((i) => <Enter key={i} preset="rise" dur={0.3}><Bar i={i} /></Enter>)}
    </Stagger>
    <Sequence from={0.4}>{({ t }) => <Box style={{ height: 6, width: t * 40, fill: "#000000" }} />}</Sequence>
  </Stack>
);
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
    e.load_component("Timeline", TIMELINE).unwrap();
    e
}

fn frame(e: &mut Engine, t: f64) -> Vec<u8> {
    render(e, "Counter", t)
}

fn render(e: &mut Engine, component: &str, t: f64) -> Vec<u8> {
    e.render(SceneCall {
        component,
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

#[test]
fn sequenced_frames_are_seekable() {
    let mut warm = engine();
    let sweep: Vec<Vec<u8>> = (0..=30)
        .map(|i| render(&mut warm, "Timeline", f64::from(i) / 30.0))
        .collect();
    // Any frame alone, cold or after the sweep in reverse, matches the sweep.
    for i in [0, 7, 13, 21, 30] {
        let t = f64::from(i) / 30.0;
        assert!(
            render(&mut engine(), "Timeline", t) == sweep[i as usize],
            "cold t={t}"
        );
        assert!(
            render(&mut warm, "Timeline", t) == sweep[i as usize],
            "warm t={t}"
        );
    }
    assert!(sweep[0] != sweep[21], "the timeline moves");
}
