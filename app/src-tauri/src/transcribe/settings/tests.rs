use super::*;

fn settings(row: &str) -> Settings {
    settings_from(Some(row))
}

#[test]
fn the_order_defaults_to_groq_whisper_apple_and_keeps_each_engine_once() {
    let default = vec!["groq", "whisper", "apple"];
    assert_eq!(settings_from(None).order, default);
    assert_eq!(settings("not json").order, default);
    assert_eq!(settings(r#"{"order":"apple"}"#).order, default);
    assert_eq!(settings(r#"{"order":[]}"#).order, default);
    assert_eq!(
        settings(r#"{"order":["apple","groq","whisper"]}"#).order,
        vec!["apple", "groq", "whisper"]
    );
    // Unknown names and non-strings are dropped, the missing appended in the default order.
    assert_eq!(
        settings(r#"{"order":["vosk",3,"apple",null]}"#).order,
        vec!["apple", "groq", "whisper"]
    );
    assert_eq!(
        settings(r#"{"order":["whisper","whisper","apple","whisper"]}"#).order,
        vec!["whisper", "apple", "groq"]
    );
}

#[test]
fn every_engine_is_on_unless_switched_off_and_mistyped_values_read_as_on() {
    let all = settings_from(None);
    assert!(all.groq && all.whisper && all.apple);
    assert_eq!(all.whisper_model, None);
    let row = settings(
        r#"{"groq":{"enabled":false},"whisper":{"enabled":"no","model":" small "},"apple":[1],"x":1}"#,
    );
    assert!(!row.groq && row.whisper && row.apple);
    assert_eq!(row.whisper_model.as_deref(), Some("small"));
    assert_eq!(
        settings(r#"{"whisper":{"model":"  "},"apple":{"enabled":false}}"#).whisper_model,
        None
    );
    assert!(!settings(r#"{"apple":{"enabled":false}}"#).apple);
}

#[test]
fn the_language_falls_back_to_the_older_per_engine_keys() {
    let language = |row: &str| settings(row).language;
    assert_eq!(settings_from(None).language, Language(None));
    assert_eq!(
        language(r#"{"language":"th_TH","apple":{"locale":"en_AU"}}"#),
        Language(Some("th_TH".into()))
    );
    assert_eq!(
        language(r#"{"apple":{"locale":"en_AU"},"whisper":{"language":"fr"}}"#),
        Language(Some("en_AU".into()))
    );
    assert_eq!(
        language(r#"{"apple":{"locale":" "},"whisper":{"language":"fr"}}"#),
        Language(Some("fr".into()))
    );
    assert_eq!(
        language(r#"{"whisper":{"language":"AUTO"}}"#),
        Language(Some("auto".into()))
    );
    assert_eq!(
        language(r#"{"language":7,"whisper":{"language":"de"}}"#),
        Language(Some("de".into()))
    );
}

#[test]
fn each_engine_reads_the_language_its_own_way() {
    let of = |l: Option<&str>| Language(l.map(str::to_string));
    // The default: English, in the Mac's region for on-device speech.
    assert_eq!(
        (of(None).whisper(), of(None).groq(), of(None).apple()),
        ("en".into(), Some("en".into()), None)
    );
    let australian = of(Some("en_AU"));
    assert_eq!(
        (australian.whisper(), australian.groq(), australian.apple()),
        ("en".into(), Some("en".into()), Some("en_AU".into()))
    );
    let thai = of(Some("th_TH"));
    assert_eq!(
        (thai.whisper(), thai.groq(), thai.apple()),
        ("th".into(), Some("th".into()), Some("th_TH".into()))
    );
    assert_eq!(of(Some("zh-Hant-TW")).whisper(), "zh");
    // Apple has no auto-detect: it keeps its default.
    let auto = of(Some("auto"));
    assert_eq!(
        (auto.whisper(), auto.groq(), auto.apple()),
        ("auto".into(), None, None)
    );
    // The old Whisper default, a bare `en`, is the Mac's own English.
    assert_eq!(of(Some("en")).apple(), None);
    assert_eq!(of(Some("fr")).apple(), Some("fr".into()));
}

#[test]
fn groq_switched_off_is_unconfigured_without_reading_the_key() {
    let off = settings(r#"{"groq":{"enabled":false}}"#);
    let refused = groq_engine(&off, || panic!("the key was read"))
        .err()
        .unwrap();
    assert_eq!(refused, "Groq is turned off in Settings → Transcription");

    let on = settings(r#"{"language":"auto"}"#);
    assert!(groq_engine(&on, || Ok(None))
        .err()
        .unwrap()
        .contains("Settings → Transcription"));
    let refused = groq_engine(&on, || Err("denied".into())).err().unwrap();
    assert!(
        refused.contains("keychain") && refused.contains("denied"),
        "{refused}"
    );
    assert_eq!(
        groq_engine(&on, || Ok(Some("gsk_test".into())))
            .ok()
            .unwrap()
            .name(),
        "groq"
    );
}

#[test]
fn an_unknown_forced_engine_is_refused_before_anything_is_read() {
    let refused = engines(None, Path::new("/no/ffmpeg"), Some("vosk"))
        .err()
        .unwrap();
    assert_eq!(
        refused,
        "no engine called vosk — it is groq, whisper, apple"
    );
}
