use super::*;

fn over(model: Option<&str>, voice: Option<&str>, speed: Option<f64>) -> VoiceOverrides {
    VoiceOverrides {
        base_url: None,
        model: model.map(Into::into),
        voice: voice.map(Into::into),
        speed,
    }
}

#[test]
fn the_row_reads_with_defaults_on_missing_or_bad_values() {
    assert_eq!(settings_from(None), VoiceSettings::default());
    assert_eq!(settings_from(Some("not json")), VoiceSettings::default());
    assert_eq!(
        settings_from(Some(
            r#"{"baseUrl":" http://127.0.0.1:8880/v1 ","model":"m","voice":"bf_emma","speed":1.25}"#
        )),
        VoiceSettings {
            base_url: Some("http://127.0.0.1:8880/v1".into()),
            model: Some("m".into()),
            voice: Some("bf_emma".into()),
            speed: 1.25,
        }
    );
    let bad = settings_from(Some(r#"{"baseUrl":"","model":7,"voice":"  ","speed":-1}"#));
    assert_eq!(bad, VoiceSettings::default());
    assert_eq!(
        settings_from(Some(r#"{"speed":"fast"}"#)).speed,
        DEFAULT_SPEED
    );
}

#[test]
fn flags_beat_the_video_which_beats_settings() {
    let settings = VoiceSettings {
        base_url: Some("http://settings:1".into()),
        model: Some("settings-model".into()),
        voice: Some("settings-voice".into()),
        speed: 0.9,
    };
    let none = VoiceOverrides::default();
    let video = over(Some("video-model"), Some("video-voice"), Some(1.1));

    let v = resolve(&none, &none, &settings).unwrap();
    assert_eq!(
        (
            v.base_url.as_str(),
            v.model.as_str(),
            v.voice.as_str(),
            v.speed
        ),
        ("http://settings:1", "settings-model", "settings-voice", 0.9)
    );

    let v = resolve(&none, &video, &settings).unwrap();
    assert_eq!(
        (v.model.as_str(), v.voice.as_str(), v.speed),
        ("video-model", "video-voice", 1.1)
    );

    let mut flags = over(None, Some("flag-voice"), None);
    flags.base_url = Some("http://flag:2/v1/".into());
    let v = resolve(&flags, &video, &settings).unwrap();
    assert_eq!(
        (
            v.base_url.as_str(),
            v.model.as_str(),
            v.voice.as_str(),
            v.speed
        ),
        ("http://flag:2", "video-model", "flag-voice", 1.1),
        "each field falls through on its own"
    );

    let blank = over(Some(" "), None, None);
    assert_eq!(
        resolve(&blank, &none, &settings).unwrap().model,
        "settings-model",
        "a blank override is no override"
    );
}

#[test]
fn no_model_or_voice_anywhere_says_where_to_set_one() {
    let none = VoiceOverrides::default();
    let mut settings = VoiceSettings {
        base_url: Some("http://x:1".into()),
        ..Default::default()
    };
    assert_eq!(resolve(&none, &none, &settings).unwrap_err(), NO_MODEL);
    settings.model = Some("m".into());
    assert_eq!(resolve(&none, &none, &settings).unwrap_err(), NO_VOICE);
    settings.voice = Some("v".into());
    assert!(resolve(&none, &none, &settings).is_ok());
}

#[test]
fn no_server_anywhere_says_where_to_set_one() {
    let e = resolve(
        &VoiceOverrides::default(),
        &VoiceOverrides::default(),
        &VoiceSettings::default(),
    )
    .unwrap_err();
    assert!(
        e.contains("Settings -> Voice") && e.contains("--server"),
        "{e}"
    );
    let flags = VoiceOverrides {
        base_url: Some("http://x:1".into()),
        speed: Some(0.0),
        ..Default::default()
    };
    let e = resolve(
        &flags,
        &VoiceOverrides::default(),
        &VoiceSettings::default(),
    )
    .unwrap_err();
    assert!(e.contains("speed 0"), "{e}");
}

#[tokio::test]
async fn save_writes_the_camel_case_row_and_keeps_other_keys() {
    use sqlx::sqlite::SqlitePoolOptions;
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    for migration in crate::db::migrations::all() {
        sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
    }
    crate::db::store::set_setting(&pool, SETTINGS_KEY, r#"{"keep":1,"baseUrl":"http://old"}"#)
        .await
        .unwrap();
    let mut settings = VoiceSettings {
        base_url: Some("http://127.0.0.1:8880/v1".into()),
        model: Some("kokoro".into()),
        voice: Some("af_heart".into()),
        speed: 1.2,
    };
    save(&pool, &settings).await.unwrap();
    let raw = crate::db::store::setting(&pool, SETTINGS_KEY)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&raw).unwrap(),
        serde_json::json!({ "keep": 1, "baseUrl": "http://127.0.0.1:8880",
            "model": "kokoro", "voice": "af_heart", "speed": 1.2 })
    );
    assert_eq!(
        settings_from(Some(&raw)).base_url.as_deref(),
        Some("http://127.0.0.1:8880")
    );

    settings.base_url = None;
    settings.voice = None;
    save(&pool, &settings).await.unwrap();
    let raw = crate::db::store::setting(&pool, SETTINGS_KEY)
        .await
        .unwrap()
        .unwrap();
    assert!(!raw.contains("baseUrl") && !raw.contains("voice"), "{raw}");
    settings.speed = f64::NAN;
    assert!(save(&pool, &settings).await.is_err());
}

#[test]
fn the_granted_host_is_the_rows_server_and_nothing_else() {
    let host = |row: Option<&str>| server_host(&settings_from(row));
    assert_eq!(host(None), None, "no row, no grant");
    assert_eq!(host(Some(r#"{ "voice": "af_heart" }"#)), None);
    assert_eq!(
        host(Some(r#"{ "baseUrl": "http://127.0.0.1:8880" }"#)),
        Some(("127.0.0.1".into(), 8880))
    );
    assert_eq!(
        host(Some(r#"{ "baseUrl": "http://localhost:8880/v1" }"#)),
        Some(("localhost".into(), 8880))
    );
    assert_eq!(
        host(Some(r#"{ "baseUrl": "https://tts.example.org" }"#)),
        Some(("tts.example.org".into(), 443))
    );
    assert_eq!(host(Some(r#"{ "baseUrl": "not a url" }"#)), None);
}
