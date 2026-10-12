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

#[test]
fn narration_tokens_are_required_and_non_negative() {
    let t = Theme::bundled("default").unwrap();
    let n = t.narration;
    assert_eq!((n.lead, n.tail, n.pause), (0.25, 1.0, 1.0));
    let base: Value = serde_json::from_str(BUNDLED[0].1).unwrap();
    let parse = |v: &Value| Theme::parse(&v.to_string(), Path::new("t.json"));
    let mut v = base.clone();
    v["narration"]["lead"] = (-0.1).into();
    let e = parse(&v).unwrap_err().to_string();
    assert!(e.contains("narration.lead = -0.1"), "{e}");
    let mut v = base.clone();
    v["narration"]["pause"] = 6.into();
    let e = parse(&v).unwrap_err().to_string();
    assert!(
        e.contains("narration.pause = 6: must be more than 0 and at most 5"),
        "{e}"
    );
    let mut v = base.clone();
    v["narration"].as_object_mut().unwrap().remove("pause");
    let older = parse(&v).unwrap();
    assert_eq!(
        older.narration.pause, DEFAULT_PAUSE,
        "a theme without pause loads"
    );
    let mut v = base;
    v.as_object_mut().unwrap().remove("narration");
    let e = parse(&v).unwrap_err().to_string();
    assert!(e.contains("missing field `narration`"), "{e}");
}

#[test]
fn the_mono_tokens_are_in_the_bundled_themes_and_default_for_older_ones() {
    let t = Theme::bundled("default").unwrap();
    assert_eq!(t.tokens["font"]["mono"], "JetBrains Mono");
    assert_eq!(t.tokens["type"]["code"]["fontFamily"], "JetBrains Mono");
    let base: Value = serde_json::from_str(BUNDLED[0].1).unwrap();
    let parse = |v: &Value| Theme::parse(&v.to_string(), Path::new("t.json"));
    // A theme written before the monospace face existed still loads, and
    // components still read the tokens from it.
    let mut v = base.clone();
    v["font"].as_object_mut().unwrap().remove("mono");
    v["type"].as_object_mut().unwrap().remove("code");
    let older = parse(&v).unwrap();
    assert_eq!(older.tokens["font"]["mono"], "JetBrains Mono");
    assert_eq!(older.tokens["type"]["code"], t.tokens["type"]["code"]);
    // A theme's own values win over the defaults.
    let mut v = base.clone();
    v["type"]["code"]["fontSize"] = 40.into();
    assert_eq!(parse(&v).unwrap().tokens["type"]["code"]["fontSize"], 40);
    let mut v = base;
    v["font"]["mono"] = "Courier".into();
    let e = parse(&v).unwrap_err().to_string();
    assert!(
        e.contains("font.mono: unknown font family `Courier`"),
        "{e}"
    );
}

#[test]
fn theme_names_are_lowercase_digits_and_dashes() {
    for good in ["default", "warm", "chalk-2", "a"] {
        assert!(valid_theme_name(good), "{good}");
    }
    for bad in ["", "Chalk", "a_b", "a/b", "../x", "x.json", &"a".repeat(65)] {
        assert!(!valid_theme_name(bad), "{bad}");
    }
    for name in Theme::bundled_names() {
        assert!(valid_theme_name(name), "bundled {name}");
        assert!(Theme::bundled_text(name).is_some());
    }
    for (name, _) in crate::STARTER_THEMES {
        assert!(valid_theme_name(name), "starter {name}");
    }
    assert!(Theme::bundled_text("warm").is_none());
}
