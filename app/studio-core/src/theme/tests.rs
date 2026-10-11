use super::*;

#[test]
fn default_theme_is_valid() {
    let t = Theme::bundled("default").unwrap();
    assert_eq!(t.body_font, "Inter");
    assert!(t.section("color").unwrap().contains_key("accent"));
    assert!(t.section("motion.spring").unwrap().contains_key("gentle"));
    assert!(t.section("nope").is_none());
}

#[test]
fn unknown_bundled_theme_lists_the_bundled_ones() {
    let e = Theme::bundled("dark").unwrap_err().to_string();
    assert!(e.contains("bundled themes: default"), "{e}");
}

#[test]
fn rejects_unknown_sections_fonts_and_missing_bg() {
    let base: Value = serde_json::from_str(BUNDLED[0].1).unwrap();
    let with = |f: &dyn Fn(&mut Value)| {
        let mut v = base.clone();
        f(&mut v);
        Theme::parse(&v.to_string(), Path::new("t.json"))
            .unwrap_err()
            .to_string()
    };
    let e = with(&|v| v["colours"] = serde_json::json!({}));
    assert!(e.contains("colours"), "{e}");
    let e = with(&|v| v["font"]["body"] = "Comic Sans".into());
    assert!(e.contains("bundled families: Inter"), "{e}");
    let e = with(&|v| {
        v["color"].as_object_mut().unwrap().remove("bg");
    });
    assert!(e.contains("color.bg is required"), "{e}");
    let e = with(&|v| v["color"]["accent"] = "indigo".into());
    assert!(e.contains("#rrggbb"), "{e}");
    let e = with(&|v| v["type"]["h1"]["color"] = "#000".into());
    assert!(e.contains("type.h1.color"), "{e}");
}
