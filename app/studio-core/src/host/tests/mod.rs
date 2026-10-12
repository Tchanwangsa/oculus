use super::*;
use crate::error::BudgetKind;
use serde_json::json;

fn host() -> Host {
    Host::new(&json!({ "color": { "accent": "#5e6ad2" } })).unwrap()
}

const CALL: CallCtx = CallCtx {
    t: 0.5,
    duration: 2.0,
    width: 1920.0,
    height: 1080.0,
    seed: 7,
    words: None,
};

fn call(h: &mut Host, name: &str) -> Result<(Value, Vec<String>)> {
    h.call(name, &Map::new(), CALL)
}

#[test]
fn runs_a_component_with_props_and_ctx() {
    let mut h = host();
    let info = h
        .load(
            "Hello",
            r#"
export const props = { title: str().default("Hi"), n: num().optional() };
export const examples = { short: { title: "A" } };
const Label = ({ text }: { text: string }, { theme }) =>
  <Text style={{ color: theme.color.accent }}>{text}</Text>;
export default ({ title }: { title: string }, { t, size }) => (
  <Stack gap={8}>
    <Label text={title} />
    <Text>{t} {size.width}</Text>
    <>{[1, 2].map((i) => <Box key={i} />)}</>
  </Stack>
);
"#,
        )
        .unwrap();
    assert_eq!(info.props.len(), 2);
    assert!(info.examples.contains_key("short"));
    let mut props = Map::new();
    props.insert("title".into(), json!("Yo"));
    let (tree, _) = h.call("Hello", &props, CALL).unwrap();
    assert_eq!(tree["k"], "box");
    assert_eq!(tree["style"]["flexDirection"], "column");
    assert_eq!(tree["children"][0]["text"], "Yo");
    assert_eq!(tree["children"][0]["style"]["color"], "#5e6ad2");
    assert_eq!(tree["children"][1]["text"], "0.5 1920");
    assert_eq!(tree["children"][3]["key"], "2");
    assert_eq!(
        tree["children"][0]["loc"][0], 5,
        "__source gives the TSX line"
    );
}

#[test]
fn thrown_errors_point_at_the_tsx_line() {
    let mut h = host();
    h.load(
        "Boom",
        "export const props = {};\n\ntype P = { a: number };\n\nexport default (p: P) => {\n  const x: any = undefined;\n  return <Box>{x.y}</Box>;\n};\n",
    )
    .unwrap();
    let e = call(&mut h, "Boom").unwrap_err();
    let loc = e.loc().cloned().unwrap();
    assert_eq!(loc.line, 7, "{e}");
    assert!(loc.snippet.contains("x.y"), "{e}");
    assert!(e.to_string().contains("TypeError"), "{e}");
}

#[test]
fn text_children_must_be_strings_or_numbers() {
    let mut h = host();
    h.load(
        "Nested",
        "export const props = {};\nexport default () =>\n  <Text>a <Text>b</Text></Text>;\n",
    )
    .unwrap();
    let e = call(&mut h, "Nested").unwrap_err().to_string();
    assert!(
        e.contains("<Text> children must be strings or numbers"),
        "{e}"
    );
    assert!(e.starts_with("Nested.tsx:3:"), "{e}");
    assert!(
        e.ends_with("\n    3 | <Text>a <Text>b</Text></Text>;"),
        "{e}"
    );
}

#[test]
fn infinite_loops_hit_the_step_budget() {
    let mut h = host();
    h.load(
        "Spin",
        "export const props = {};\nexport default () => { while (true) {} };\n",
    )
    .unwrap();
    let e = call(&mut h, "Spin").unwrap_err();
    assert!(
        matches!(e, StudioError::Budget { kind: BudgetKind::Steps, ref component } if component == "Spin"),
        "{e}"
    );
    // The budget is per entry: the next call starts fresh.
    h.load(
        "Ok",
        "export const props = {};\nexport default () => <Box />;\n",
    )
    .unwrap();
    call(&mut h, "Ok").unwrap();
}

#[test]
fn top_level_loops_hit_the_budget_while_loading() {
    let mut h = host();
    let e = h
        .load(
            "Spin",
            "for (;;) {}\nexport const props = {};\nexport default () => <Box />;\n",
        )
        .unwrap_err();
    assert!(matches!(e, StudioError::Budget { .. }), "{e}");
}

#[test]
fn runaway_allocation_hits_the_memory_budget() {
    let mut h = host();
    h.load(
        "Hog",
        "export const props = {};\nexport default () => { let s = 'x'; const a = []; for (let i = 0; i < 1e9; i++) a.push(s + i); return <Box />; };\n",
    )
    .unwrap();
    let e = call(&mut h, "Hog").unwrap_err();
    assert!(matches!(e, StudioError::Budget { .. }), "{e}");
}

#[test]
fn no_clock() {
    let mut h = host();
    h.load(
        "Clock",
        "export const props = {};\nexport default () => <Text>{Date.now()}</Text>;\n",
    )
    .unwrap();
    let e = call(&mut h, "Clock").unwrap_err().to_string();
    assert!(e.contains("Date"), "{e}");
    h.load(
        "Perf",
        "export const props = {};\nexport default () => <Text>{typeof performance}</Text>;\n",
    )
    .unwrap();
    let (tree, _) = call(&mut h, "Perf").unwrap();
    assert_eq!(tree["text"], "undefined");
    h.load(
        "Gc",
        "export const props = {};\nexport default () => <Text>{typeof WeakRef} {typeof FinalizationRegistry}</Text>;\n",
    )
    .unwrap();
    let (tree, _) = call(&mut h, "Gc").unwrap();
    assert_eq!(tree["text"], "undefined undefined");
}

#[test]
fn random_is_seeded_per_call() {
    let mut h = host();
    h.load(
        "Dice",
        "export const props = {};\nexport default () => <Text>{Math.random()}</Text>;\n",
    )
    .unwrap();
    let a = call(&mut h, "Dice").unwrap().0;
    let b = call(&mut h, "Dice").unwrap().0;
    assert_eq!(a, b);
    let c = h
        .call("Dice", &Map::new(), CallCtx { seed: 8, ..CALL })
        .unwrap()
        .0;
    assert_ne!(a, c);
}

#[test]
fn imports_are_rejected_with_their_line() {
    let mut h = host();
    let e = h
        .load(
            "Imp",
            "export const props = {};\nimport { x } from './x';\nexport default () => <Box />;\n",
        )
        .unwrap_err();
    assert!(matches!(e, StudioError::Transpile { .. }), "{e}");
    assert_eq!(e.loc().map(|l| l.line), Some(2), "{e}");
    assert!(e.to_string().contains("imports are not supported"), "{e}");
}

#[test]
fn syntax_errors_point_at_the_tsx_line() {
    let mut h = host();
    let e = h
        .load(
            "Bad",
            "export const props = {};\n\nexport default () => <Box>;\n",
        )
        .unwrap_err();
    assert!(matches!(e, StudioError::Transpile { .. }), "{e}");
    assert_eq!(e.loc().map(|l| l.line), Some(3), "{e}");
}

#[test]
fn props_export_is_required() {
    let mut h = host();
    let e = h
        .load("NoProps", "export default () => <Box />;\n")
        .unwrap_err()
        .to_string();
    assert!(e.contains("export const props"), "{e}");
}

#[test]
fn element_props_are_checked() {
    let mut h = host();
    h.load(
        "Gap",
        "export const props = {};\nexport default () => <Box gap={8} />;\n",
    )
    .unwrap();
    let e = call(&mut h, "Gap").unwrap_err().to_string();
    assert!(e.contains("<Box> has no prop \"gap\""), "{e}");
}

#[test]
fn console_goes_to_logs() {
    let mut h = host();
    h.load(
        "Chatty",
        "export const props = {};\nexport default ({}, { t }) => { console.log('t is', t, { a: 1 }); console.warn('w'); return <Box />; };\n",
    )
    .unwrap();
    let (_, logs) = call(&mut h, "Chatty").unwrap();
    assert_eq!(
        logs,
        vec!["t is 0.5 {\"a\":1}".to_owned(), "warn: w".to_owned()]
    );
}

#[test]
fn motion_helpers() {
    let mut h = host();
    h.load(
        "Motion",
        r##"export const props = {};
export default () => <Text>{[
  interpolate(0.5, [0, 1], [10, 20]),
  interpolate(2, [0, 1], [10, 20]),
  interpolate(0.5, [0, 1], [0, 1], { easing: easeOutCubic }),
  Math.round(spring(10, { stiffness: 170, damping: 26 }) * 1000) / 1000,
  spring(0),
  Math.round(cubicBezier(0.25, 0.1, 0.25, 1)(0.5) * 1000) / 1000,
  mix("#000", "#ffffff", 0.5),
  mix("#00000000", "#ff000080", 1),
].join(" ")}</Text>;
"##,
    )
    .unwrap();
    let (tree, _) = call(&mut h, "Motion").unwrap();
    assert_eq!(tree["text"], "15 20 0.875 1 0 0.802 #808080 #ff000080");
}

#[test]
fn seeds_are_stable() {
    assert_eq!(seed("TitleCard", None, 0), seed("TitleCard", None, 0));
    assert_ne!(seed("TitleCard", None, 0), seed("TitleCard", None, 1));
    assert_eq!(
        seed("TitleCard", Some("intro"), 0),
        seed("TitleCard", Some("intro"), 5)
    );
    assert_ne!(
        seed("TitleCard", Some("intro"), 0),
        seed("TitleCard", Some("recap"), 0)
    );
}

#[test]
fn an_id_seed_never_hashes_an_index_seed() {
    // Index 0x2d2d's first bytes spell the id "--"; the tag keeps them
    // apart, and the end-of-name byte keeps "Ab"+"c" from "A"+"bc".
    assert_ne!(seed("Card", Some("--"), 0), seed("Card", None, 0x2d2d));
    assert_ne!(seed("Ab", Some("c"), 0), seed("A", Some("bc"), 0));
}

#[test]
fn a_local_named_h_does_not_break_jsx() {
    let mut h = host();
    h.load(
        "Bars",
        "export const props = {};\nexport default () => <Row>{[1, 2].map((h) => <Box style={{ height: h }} />)}</Row>;\n",
    )
    .unwrap();
    let (tree, _) = call(&mut h, "Bars").unwrap();
    assert_eq!(tree["children"][1]["style"]["height"], 2);
}

/// Reads the state a component could have mutated, then mutates all of it.
const MUTATOR: &str = r##"
export const props = {};
let calls = 0;
globalThis.loadedBy = "Mutator";
export default (_p, { theme }) => {
  calls += 1;
  const seen = [
    calls,
    typeof globalThis.leak,
    Math.max(1, 2),
    [1].map((x) => x + 1)[0],
    theme.color.accent,
    Math.random(),
  ].join(" ");
  globalThis.leak = 1;
  Math.max = () => 0;
  Array.prototype.map = () => [99];
  try { theme.color.accent = "#000000"; } catch (_) {}
  __studioSetTheme('{"color":{"accent":"#000000"}}');
  return <Text>{seen}</Text>;
};
"##;

#[test]
fn a_call_cannot_see_what_an_earlier_call_changed() {
    let mut h = host();
    h.load("Mutator", MUTATOR).unwrap();
    let first = call(&mut h, "Mutator").unwrap().0;
    let text = first["text"].as_str().unwrap().to_owned();
    assert!(text.starts_with("1 undefined 2 2 #5e6ad2 "), "{text}");
    for _ in 0..3 {
        assert_eq!(call(&mut h, "Mutator").unwrap().0, first);
    }

    // Nor can another component, including what Mutator's module set while
    // loading.
    h.load(
        "Reader",
        "export const props = {};\nexport default (_p, { theme }) => <Text>{[typeof loadedBy, typeof leak, Math.max(1, 2), [1].map((x) => x + 1)[0], theme.color.accent].join(' ')}</Text>;\n",
    )
    .unwrap();
    let (tree, _) = call(&mut h, "Reader").unwrap();
    assert_eq!(tree["text"], "undefined undefined 2 2 #5e6ad2");
}

#[test]
fn the_heap_does_not_grow_across_calls() {
    let mut h = host();
    h.load("Mutator", MUTATOR).unwrap();
    call(&mut h, "Mutator").unwrap();
    let start = h.heap_bytes();
    for _ in 0..2000 {
        call(&mut h, "Mutator").unwrap();
    }
    let end = h.heap_bytes();
    println!("QuickJS heap after 1 call: {start} B, after 2001: {end} B");
    assert_eq!(start, end);
}

mod globals;
mod motion;
mod resolve;
mod steps;
