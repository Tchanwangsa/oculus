//! Every way a component can be wrong comes back as a `StudioError` an
//! agent can act on: what, where (component, TSX line) and what is valid.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::fixture;
use serde_json::{Map, Value, json};
use studio_core::{BudgetKind, Engine, RenderOptions, SceneCall, Size, StudioError, Theme};

fn engine() -> Engine {
    Engine::new(
        Theme::bundled("default").unwrap(),
        Size {
            width: 640,
            height: 360,
        },
        Some(fixture("broken").join("assets")),
        RenderOptions::default(),
    )
    .unwrap()
}

fn render(tsx: &str, props: Value) -> StudioError {
    let mut e = engine();
    if let Err(err) = e.load_component("Comp", tsx) {
        return err;
    }
    let props: Map<String, Value> = props.as_object().cloned().unwrap_or_default();
    e.render(SceneCall {
        component: "Comp",
        props: &props,
        t: 0.0,
        duration: 1.0,
        index: 0,
        words: None,
    })
    .unwrap_err()
}

#[test]
fn unknown_style_key_names_the_closest_and_the_line() {
    let e = render(
        "export const props = {};\nexport default () => (\n  <Stack>\n    <Box style={{ backgroundColour: '#fff', pading: 4 }} />\n  </Stack>\n);\n",
        json!({}),
    );
    let StudioError::Style {
        ref node, ref loc, ..
    } = e
    else {
        panic!("{e:?}")
    };
    let msg = e.to_string();
    assert!(
        msg.contains("unknown style key `backgroundColour`")
            || msg.contains("unknown style key `pading`"),
        "{msg}"
    );
    assert_eq!(node, "Box > Box[0]");
    assert_eq!(loc.as_ref().map(|l| l.line), Some(4), "{msg}");
    let e = render(
        "export const props = {};\nexport default () => <Box style={{ pading: 4 }} />;\n",
        json!({}),
    );
    assert!(e.to_string().contains("did you mean `padding`"), "{e}");
}

#[test]
fn wrong_prop_type() {
    let e = render(
        "export const props = { n: num() };\nexport default ({ n }) => <Box style={{ width: n }} />;\n",
        json!({ "n": "12" }),
    );
    assert_eq!(
        e.to_string(),
        "Comp prop `n`: expected a finite number, got \"12\""
    );
}

#[test]
fn missing_prop() {
    let e = render(
        "export const props = { title: str() };\nexport default ({ title }) => <Text>{title}</Text>;\n",
        json!({}),
    );
    assert!(matches!(e, StudioError::Props { .. }), "{e}");
    assert!(
        e.to_string()
            .contains("`title`: expected a string (required), got nothing"),
        "{e}"
    );
}

#[test]
fn missing_image() {
    let e = render(
        "export const props = {};\nexport default () => <Stack>\n  <Image src=\"nope.png\" />\n</Stack>;\n",
        json!({}),
    );
    let StudioError::Image {
        ref src, ref loc, ..
    } = e
    else {
        panic!("{e:?}")
    };
    assert_eq!(src, "nope.png");
    assert_eq!(loc.as_ref().map(|l| l.line), Some(3));
    assert!(e.to_string().contains("not found"), "{e}");
    let e = render(
        "export const props = {};\nexport default () => <Image src=\"../video.json\" />;\n",
        json!({}),
    );
    assert!(
        e.to_string().contains("inside the project's assets/"),
        "{e}"
    );
}

#[test]
fn undecodable_image() {
    let dir = std::env::temp_dir().join(format!("studio-bad-image-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("bad.png"), b"not a png").unwrap();
    let mut e = Engine::new(
        Theme::bundled("default").unwrap(),
        Size {
            width: 64,
            height: 64,
        },
        Some(dir.clone()),
        RenderOptions::default(),
    )
    .unwrap();
    e.load_component(
        "Comp",
        "export const props = {};\nexport default () => <Image src=\"bad.png\" />;\n",
    )
    .unwrap();
    let err = e
        .render(SceneCall {
            component: "Comp",
            props: &Map::new(),
            t: 0.0,
            duration: 1.0,
            index: 0,
            words: None,
        })
        .unwrap_err();
    std::fs::remove_dir_all(&dir).ok();
    assert!(err.to_string().contains("could not be decoded"), "{err}");
}

#[test]
fn unknown_font() {
    let e = render(
        "export const props = {};\nexport default () => <Text style={{ fontFamily: 'Helvetica' }}>hi</Text>;\n",
        json!({}),
    );
    assert!(matches!(e, StudioError::Font { .. }), "{e}");
    assert!(
        e.to_string().contains("bundled families: Inter, Manrope"),
        "{e}"
    );
}

#[test]
fn endless_loop_is_a_budget_error() {
    let e = render(
        "export const props = {};\nexport default () => { while (true) {} };\n",
        json!({}),
    );
    assert!(
        matches!(
            e,
            StudioError::Budget {
                kind: BudgetKind::Steps,
                ..
            }
        ),
        "{e}"
    );
    assert!(
        e.to_string().starts_with("Comp ran out of its step budget"),
        "{e}"
    );
}

#[test]
fn thrown_error_points_at_its_tsx_line() {
    let e = render(
        "export const props = {};\n\nconst check = (n: number) => {\n  if (n > 0) throw new Error('too big: ' + n);\n};\n\nexport default () => {\n  check(3);\n  return <Box />;\n};\n",
        json!({}),
    );
    let loc = e.loc().cloned().unwrap();
    assert_eq!(loc.line, 4, "{e}");
    assert!(loc.snippet.contains("throw new Error"), "{e}");
    assert!(e.to_string().contains("too big: 3"), "{e}");
}

#[test]
fn date_now_is_unavailable() {
    let e = render(
        "export const props = {};\nexport default () => <Text>{Date.now()}</Text>;\n",
        json!({}),
    );
    assert!(e.to_string().contains("Date"), "{e}");
    assert_eq!(e.loc().map(|l| l.line), Some(2), "{e}");
}

#[test]
fn imports_are_rejected() {
    let e = render(
        "import { thing } from 'somewhere';\nexport const props = {};\nexport default () => <Box />;\n",
        json!({}),
    );
    assert!(matches!(e, StudioError::Transpile { .. }), "{e}");
    assert!(e.to_string().contains("imports are not supported"), "{e}");
}

#[test]
fn returning_a_non_element_is_an_error() {
    let e = render(
        "export const props = {};\nexport default () => 'hello';\n",
        json!({}),
    );
    assert!(e.to_string().contains("must return one element"), "{e}");
}

#[test]
fn sequence_with_a_negative_dur_names_its_line() {
    let e = render(
        "export const props = {};\nexport default () => <Stack>\n  <Sequence from={1} dur={-2}><Box /></Sequence>\n</Stack>;\n",
        json!({}),
    );
    assert_eq!(e.loc().map(|l| l.line), Some(3), "{e}");
    assert!(
        e.to_string().starts_with(
            "Comp.tsx:3:3: RangeError: <Sequence> dur must be a finite number of seconds, 0 or more, got -2"
        ),
        "{e}"
    );
}

#[test]
fn unknown_enter_preset_lists_the_presets() {
    let e = render(
        "export const props = {};\nexport default () => <Stack>\n  <Enter preset=\"fade-in\"><Box /></Enter>\n</Stack>;\n",
        json!({}),
    );
    assert_eq!(e.loc().map(|l| l.line), Some(3), "{e}");
    assert!(e.to_string().contains("presets: fade, rise,"), "{e}");
}

#[test]
fn unknown_reveal_unit_is_a_style_error_with_a_suggestion() {
    let e = render(
        "export const props = {};\nexport default () => <Stack>\n  <Text style={{ reveal: { by: 'wrod', progress: 0.5 } }}>Hello there</Text>\n</Stack>;\n",
        json!({}),
    );
    let StudioError::Style {
        ref node, ref loc, ..
    } = e
    else {
        panic!("{e:?}")
    };
    assert_eq!(node, "Box > Text[0]");
    assert_eq!(loc.as_ref().map(|l| l.line), Some(3));
    let msg = e.to_string();
    assert!(
        msg.contains("unknown reveal.by `wrod` (did you mean `word`?)"),
        "{msg}"
    );
    let e = render(
        "export const props = {};\nexport default () => <Text style={{ reveal: { progress: 1, from: 'rice' } }}>x</Text>;\n",
        json!({}),
    );
    assert!(e.to_string().contains("did you mean `rise`"), "{e}");
}

#[test]
fn bad_path_data_names_the_element_and_line() {
    let e = render(
        "export const props = {};\nexport default () => <Stack>\n  <Path d=\"M 0 0 Q 10\" stroke={{ width: 2, color: '#000' }} />\n</Stack>;\n",
        json!({}),
    );
    let msg = e.to_string();
    assert!(matches!(e, StudioError::Component { .. }), "{e:?}");
    assert_eq!(e.loc().map(|l| l.line), Some(3), "{msg}");
    assert!(
        msg.contains("Box > Path[0]: d is not SVG path data"),
        "{msg}"
    );
    let e = render(
        "export const props = {};\nexport default () => <Path d=\"M 0 0 L 1 1\" stroke={{ width: 2 }} />;\n",
        json!({}),
    );
    assert!(e.to_string().contains("missing field `color`"), "{e}");
}
