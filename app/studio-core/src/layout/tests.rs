use super::*;
use crate::host::{CallCtx, Host};
use crate::scene::{NumericVariant, parse_tree};
use serde_json::{Map, json};

fn laid_out(tsx: &str) -> LNode {
    let mut host = Host::new(&json!({})).unwrap();
    host.load("Keys", tsx).unwrap();
    let call = CallCtx {
        t: 0.5,
        duration: 2.0,
        width: 200.0,
        height: 100.0,
        seed: 0,
    };
    let (tree, _) = host.call("Keys", &Map::new(), call).unwrap();
    let mut typeset = Typeset::new();
    let mut images = ImageCache::new(None);
    let mut layouter = Layouter {
        typeset: &mut typeset,
        images: &mut images,
        default_family: "Inter",
    };
    let root = parse_tree(&tree).unwrap_or_else(|e| panic!("{}", e.message));
    layouter
        .layout(root, 200.0, 100.0)
        .unwrap_or_else(|_| panic!("layout failed"))
}

#[test]
fn a_function_components_key_reaches_the_laid_out_node() {
    let root = laid_out(
        "export const props = {};\nconst Card = () => <Box style={{ width: 10, height: 10 }} />;\nexport default () => <Row><Card key=\"card\" /><Sequence from={1}><Card key=\"later\" /></Sequence><Box key={3} /></Row>;\n",
    );
    let keys: Vec<Option<&str>> = root.children.iter().map(|c| c.key.as_deref()).collect();
    assert_eq!(keys, [Some("card"), Some("later"), Some("3")]);
    // The hidden Sequence child keeps its slot.
    assert_eq!(root.children[1].rect.x0, 10.0);
}

#[test]
fn paths_size_from_their_view_box() {
    let root = laid_out(
        "export const props = {};\nexport default () => <Row align=\"flex-start\"><Path d=\"M 0 0 L 40 20\" style={{ width: 80 }} /><Path d=\"M 0 0 L 1 1\" viewBox={[0, 0, 4, 1]} style={{ height: 10 }} /></Row>;\n",
    );
    let size = |i: usize| {
        (
            root.children[i].rect.width(),
            root.children[i].rect.height(),
        )
    };
    assert_eq!(size(0), (80.0, 40.0));
    assert_eq!(size(1), (40.0, 10.0));
}

#[test]
fn tabular_digits_share_one_advance() {
    let mut typeset = Typeset::new();
    let width = |text: &str, tabular: bool, typeset: &mut Typeset| {
        let style = Style {
            font_size: Some(40.0),
            font_variant_numeric: tabular.then_some(NumericVariant::TabularNums),
            ..Style::default()
        };
        let mut layout = typeset.take(&TextKey::new(text, "Inter", &style));
        layout.break_all_lines(None);
        layout.full_width()
    };
    let (ones, eights) = (
        width("1111", false, &mut typeset),
        width("8888", false, &mut typeset),
    );
    assert!(
        (ones - eights).abs() > 1.0,
        "proportional: {ones} vs {eights}"
    );
    let (ones, eights) = (
        width("1111", true, &mut typeset),
        width("8888", true, &mut typeset),
    );
    assert!((ones - eights).abs() < 1e-3, "tabular: {ones} vs {eights}");
}
