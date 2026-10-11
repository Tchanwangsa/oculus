//! Lint of a component's source and declared props: token use (literal
//! colours and style numbers that equal a theme token) and required props
//! missing from an example.
#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::json;
use std::path::{Path, PathBuf};
use studio_core::Project;

fn project(name: &str, tsx: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("studio-lint-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("components")).unwrap();
    let video = json!({
        "size": { "width": 64, "height": 36 }, "theme": "default",
        "scenes": [{ "component": "Card", "props": { "title": "Hi" }, "hold": 1 }]
    });
    std::fs::write(dir.join("video.json"), video.to_string()).unwrap();
    std::fs::write(dir.join("components/Card.tsx"), tsx).unwrap();
    dir
}

fn lint_lines(dir: &Path) -> Vec<String> {
    let report = Project::load(dir).unwrap().lint().unwrap();
    std::fs::remove_dir_all(dir).ok();
    report.issues.iter().map(ToString::to_string).collect()
}

const TOKENS: &str = r##"export const props = { title: str() };
export const examples = { short: { title: "Hi" } };
export default ({ title }, { theme }) => (
  <Box style={{ fill: "#5e6ad3", padding: 24 }}>
    <Text style={{ ...theme.type.h2, color: theme.color.text }}>{title}</Text>
  </Box>
);
"##;

#[test]
fn a_colour_literal_is_refused_with_its_token_and_position() {
    let lines = lint_lines(&project("colour", TOKENS));
    let text = lines.join("\n");
    assert!(text.contains("Card.tsx:4:24: colour"), "{text}");
    assert!(
        text.contains("colour `#5e6ad3` is a literal; use theme.color.accent"),
        "{text}"
    );
    assert!(
        text.contains("style value 24 equals theme.space.m; use theme.space.m"),
        "{text}"
    );
}

const MISSING: &str = r#"export const props = { title: str(), note: str(), size: num().default(2) };
export const examples = { short: { title: "Hi" } };
export default ({ title }, { theme }) => <Text style={{ color: theme.color.text }}>{title}</Text>;
"#;

#[test]
fn an_example_that_omits_a_required_prop_is_refused() {
    let lines = lint_lines(&project("missing", MISSING));
    let text = lines.join("\n");
    assert!(
        text.contains("Card example `short`: omits required prop `note`, which has no default"),
        "{text}"
    );
    // `size` has a default, so an example may omit it.
    assert!(!text.contains("`size`"), "{text}");
}

const NO_EXAMPLES: &str = r#"export const props = { title: str() };
export default ({ title }, { theme }) => <Text style={{ color: theme.color.text }}>{title}</Text>;
"#;

#[test]
fn required_props_with_no_examples_are_refused_once() {
    let lines = lint_lines(&project("noexamples", NO_EXAMPLES));
    let card: Vec<&String> = lines
        .iter()
        .filter(|l| l.contains("has required props"))
        .collect();
    assert_eq!(card.len(), 1, "{lines:#?}");
    assert!(card[0].contains("(title)"), "{}", card[0]);
}
