use std::path::Path;

use serde_json::Value;

use super::super::{CaptionStyle, Highlight, Placement};
use crate::scene::Color;
use crate::theme::Theme;

fn default_json() -> Value {
    serde_json::from_str(include_str!("../../../themes/default.json")).unwrap()
}

fn parse(v: &Value) -> Result<Theme, String> {
    Theme::parse(&v.to_string(), Path::new("t.json")).map_err(|e| e.to_string())
}

#[test]
fn the_default_theme_resolves_its_caption_tokens() {
    let c = Theme::bundled("default").unwrap().captions;
    assert_eq!(c.font_family, "Inter");
    assert_eq!(c.highlight_style, Highlight::Pill);
    assert_eq!(c.placement, Placement::Bottom);
    assert_eq!(c.highlight, Color::parse("#5e6ad2").unwrap());
    let plate = c.plate.unwrap();
    assert!((plate.a - 0.8).abs() < 1e-6);
    assert_eq!((c.max_chars, c.max_lines), (42, 2));
}

#[test]
fn a_theme_without_captions_gets_them_from_its_own_tokens() {
    let mut v = default_json();
    v.as_object_mut().unwrap().remove("captions");
    v["color"]["accent"] = "#ff0000".into();
    v["font"]["body"] = "Manrope".into();
    let t = parse(&v).unwrap();
    assert_eq!(t.captions.highlight, Color::parse("#ff0000").unwrap());
    assert_eq!(t.captions.font_family, "Manrope");
    // The filled block is part of the tokens components see.
    assert_eq!(t.tokens["captions"]["highlight"], "accent");
}

#[test]
fn bad_caption_tokens_are_named() {
    let with = |key: &str, value: Value| {
        let mut v = default_json();
        v["captions"][key] = value;
        parse(&v).unwrap_err()
    };
    let e = with("highlight", "brand".into());
    assert!(
        e.contains("captions.highlight") && e.contains("`brand`"),
        "{e}"
    );
    let e = with("position", "middle".into());
    assert!(e.contains("captions.position = `middle`"), "{e}");
    let e = with("maxLines", 5.into());
    assert!(e.contains("captions.maxLines = 5"), "{e}");
    let e = with("fontFamily", "Comic Sans".into());
    assert!(e.contains("captions.fontFamily"), "{e}");
    let e = with("size", 3.into());
    assert!(e.contains("size"), "{e}");
    let mut v = default_json();
    v["captions"]["plate"] = "none".into();
    assert_eq!(parse(&v).unwrap().captions.plate, None);
}

#[test]
fn portrait_lines_hold_fewer_characters() {
    let c: CaptionStyle = Theme::bundled("default").unwrap().captions;
    assert_eq!(c.line_chars(1920, 1080), 42);
    assert_eq!(c.line_chars(1080, 1920), 24);
    assert_eq!(c.line_chars(1080, 1080), 24);
    assert_eq!(CaptionStyle::unit(1080, 1920), 1.0);
    assert_eq!(CaptionStyle::unit(640, 360), 1.0 / 3.0);
}
