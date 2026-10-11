//! Loading a component again under the same name replaces it in the same
//! engine, which is how the Components playground hot-reloads an edit.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

use serde_json::{Map, Value, json};
use studio_core::{Engine, RenderOptions, SceneCall, Size, Theme};

const V1: &str = r##"
export const props = { label: str() };
let calls = 0;
const swatch = "#ff0000";
const Inner = (_p, { theme }) => <Box style={{ flexGrow: 1, fill: swatch }} />;
export default ({ label }) => {
  calls += 1;
  console.log("v1", label, calls);
  return <Stack><Inner /></Stack>;
};
"##;

// Same top-level names (`calls`, `swatch`, `Inner`), a new prop and colour.
const V2: &str = r##"
export const props = { label: str(), tone: oneOf(["blue", "green"]).default("blue") };
let calls = 100;
const swatch = { blue: "#0000ff", green: "#00ff00" };
const Inner = ({ tone }) => <Box style={{ flexGrow: 1, fill: swatch[tone] }} />;
export default ({ label, tone }) => {
  calls += 1;
  console.log("v2", label, calls);
  return <Stack><Inner tone={tone} /></Stack>;
};
"##;

fn engine() -> Engine {
    let size = Size {
        width: 64,
        height: 36,
    };
    Engine::new(
        Theme::bundled("default").unwrap(),
        size,
        None,
        RenderOptions::default(),
    )
    .unwrap()
}

fn props(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap()
}

fn centre(e: &mut Engine, props: &Map<String, Value>) -> ([u8; 4], Vec<String>) {
    let frame = e
        .render(SceneCall {
            component: "Swatch",
            props,
            t: 0.0,
            duration: 1.0,
            index: 0,
            words: None,
        })
        .unwrap();
    let i = ((frame.height / 2 * frame.width + frame.width / 2) * 4) as usize;
    let px = frame.rgba[i..i + 4].try_into().unwrap();
    (px, frame.logs)
}

#[test]
fn reloading_a_component_under_the_same_name_uses_the_new_source() {
    let mut e = engine();
    let p = props(json!({ "label": "a" }));

    e.load_component("Swatch", V1).unwrap();
    let (px, logs) = centre(&mut e, &p);
    assert_eq!(px, [255, 0, 0, 255]);
    assert_eq!(logs, ["v1 a 1"]);

    let info = e.load_component("Swatch", V2).unwrap();
    assert_eq!(info.props.len(), 2, "the new props schema replaces the old");
    let (px, logs) = centre(&mut e, &p);
    assert_eq!(px, [0, 0, 255, 255], "renders with the new source");
    assert_eq!(logs, ["v2 a 101"], "module state starts fresh");

    let (px, _) = centre(&mut e, &props(json!({ "label": "a", "tone": "green" })));
    assert_eq!(px, [0, 255, 0, 255]);

    // And back again: the first version's module is not cached by name.
    e.load_component("Swatch", V1).unwrap();
    let (px, logs) = centre(&mut e, &p);
    assert_eq!(px, [255, 0, 0, 255]);
    assert_eq!(logs, ["v1 a 1"]);
}

#[test]
fn a_failed_reload_keeps_the_previous_version() {
    let mut e = engine();
    let p = props(json!({ "label": "a" }));
    e.load_component("Swatch", V1).unwrap();

    let broken = V2.replace("export default", "export default (");
    assert!(e.load_component("Swatch", &broken).is_err());
    let no_props = V2.replace("export const props", "const props");
    assert!(e.load_component("Swatch", &no_props).is_err());

    assert_eq!(e.component("Swatch").unwrap().props.len(), 1);
    let (px, _) = centre(&mut e, &p);
    assert_eq!(px, [255, 0, 0, 255]);
}
