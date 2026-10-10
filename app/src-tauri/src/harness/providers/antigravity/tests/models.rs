use super::super::models::{is_slug, model_slug, parse_models};

#[test]
fn model_slugs_are_read_off_the_first_column() {
    let out = "\
Models available to your account

  gemini-3.8-flash-high      Fastest, highest effort
  gemini-3.8-flash-medium    Balanced
  gemini-3.8-flash-low
  claude-opus-5              via Antigravity
";
    let ids: Vec<String> = parse_models(out).into_iter().map(|m| m.id).collect();
    assert_eq!(ids, ["gemini-3.8-flash", "claude-opus-5"]);
}

/// The listing as 1.2.9 prints it.
#[test]
fn a_real_listing_keeps_the_names_it_prints() {
    let out = "Fetching available models...\n\
gemini-3.8-flash-high\tGemini 3.8 Flash (High)\n\
gemini-3.8-flash-low\tGemini 3.8 Flash (Low)\n\
claude-sonnet-4-6\tClaude Sonnet 4.6 (Thinking)\n";
    let got: Vec<(String, String, Vec<String>)> = parse_models(out)
        .into_iter()
        .map(|m| (m.id, m.display_name, m.reasoning_efforts))
        .collect();
    assert_eq!(
        got,
        vec![
            (
                "gemini-3.8-flash".to_string(),
                "Gemini 3.8 Flash".to_string(),
                vec!["high".to_string(), "low".to_string()]
            ),
            (
                "claude-sonnet-4-6".to_string(),
                "Claude Sonnet 4.6 (Thinking)".to_string(),
                vec![]
            ),
        ]
    );
}

#[test]
fn level_suffixes_become_reasoning_levels() {
    let out = "\
gemini-3.8-flash-high
gemini-3.8-flash-medium
gemini-3.8-flash-low
gemini-3.1-pro-high
gemini-3.1-pro-low
claude-sonnet-4-6
claude-opus-4-6-thinking
gpt-oss-120b-medium
";
    let got: Vec<(String, String, Vec<String>, Option<String>)> = parse_models(out)
        .into_iter()
        .map(|m| {
            (
                m.id,
                m.display_name,
                m.reasoning_efforts,
                m.default_reasoning_effort,
            )
        })
        .collect();
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(
        got,
        vec![
            (
                "gemini-3.8-flash".into(),
                "Gemini 3.8 Flash".into(),
                s(&["high", "medium", "low"]),
                Some("medium".into())
            ),
            (
                "gemini-3.1-pro".into(),
                "Gemini 3.1 Pro".into(),
                s(&["high", "low"]),
                Some("high".into())
            ),
            (
                "claude-sonnet-4-6".into(),
                "Claude Sonnet 4.6".into(),
                vec![],
                None
            ),
            (
                "claude-opus-4-6-thinking".into(),
                "Claude Opus 4.6 Thinking".into(),
                vec![],
                None
            ),
            (
                "gpt-oss-120b".into(),
                "GPT-OSS 120B".into(),
                s(&["medium"]),
                Some("medium".into())
            ),
        ]
    );
}

#[test]
fn the_level_is_folded_back_into_the_slug() {
    assert_eq!(
        model_slug("gemini-3.8-flash", Some("high")),
        "gemini-3.8-flash-high"
    );
    assert_eq!(model_slug("claude-sonnet-4-6", None), "claude-sonnet-4-6");
    assert_eq!(
        model_slug("gemini-3.8-flash-low", Some("high")),
        "gemini-3.8-flash-low"
    );
    assert_eq!(
        model_slug("gemini-3.8-flash-low", None),
        "gemini-3.8-flash-low"
    );
}

#[test]
fn furniture_is_not_a_model() {
    assert!(parse_models("Models\n\n──────────\nNone found.\n").is_empty());
    assert!(!is_slug("models"));
    assert!(!is_slug("──────────"));
    assert!(!is_slug("a-"));
    assert!(is_slug("gemini-3.8-flash"));
}

#[test]
fn a_repeated_slug_is_one_model() {
    let ids: Vec<String> = parse_models("gemini-3.8-flash\ngemini-3.8-flash\n")
        .into_iter()
        .map(|m| m.id)
        .collect();
    assert_eq!(ids, ["gemini-3.8-flash"]);
}
