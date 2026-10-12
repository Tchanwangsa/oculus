use super::*;

/// The server voice `resolve` settled on.
fn server(voice: Voice) -> ServerVoice {
    match voice {
        Voice::Server(v) => v,
        other => panic!("not a server voice: {other:?}"),
    }
}

fn over(model: Option<&str>, voice: Option<&str>, speed: Option<f64>) -> VoiceOverrides {
    VoiceOverrides {
        engine: None,
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
            engine: Some(Engine::Server),
            base_url: Some("http://127.0.0.1:8880/v1".into()),
            model: Some("m".into()),
            voice: Some("bf_emma".into()),
            speed: 1.25,
        }
    );
    let bad = settings_from(Some(
        r#"{"engine":"piper","baseUrl":"","model":7,"voice":"  ","speed":-1}"#,
    ));
    assert_eq!(bad, VoiceSettings::default());
    assert_eq!(
        settings_from(Some(r#"{"speed":"fast"}"#)).speed,
        DEFAULT_SPEED
    );
}

#[test]
fn flags_beat_the_video_which_beats_settings() {
    let settings = VoiceSettings {
        engine: Some(Engine::Server),
        base_url: Some("http://settings:1".into()),
        model: Some("settings-model".into()),
        voice: Some("settings-voice".into()),
        speed: 0.9,
    };
    let none = VoiceOverrides::default();
    let video = over(Some("video-model"), Some("video-voice"), Some(1.1));

    let v = server(resolve(&none, &none, &settings).unwrap());
    assert_eq!(
        (
            v.base_url.as_str(),
            v.model.as_str(),
            v.voice.as_str(),
            v.speed
        ),
        ("http://settings:1", "settings-model", "settings-voice", 0.9)
    );

    let v = server(resolve(&none, &video, &settings).unwrap());
    assert_eq!(
        (v.model.as_str(), v.voice.as_str(), v.speed),
        ("video-model", "video-voice", 1.1)
    );

    let mut flags = over(None, Some("flag-voice"), None);
    flags.base_url = Some("http://flag:2/v1/".into());
    let v = server(resolve(&flags, &video, &settings).unwrap());
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
        server(resolve(&blank, &none, &settings).unwrap()).model,
        "settings-model",
        "a blank override is no override"
    );
}

#[test]
fn no_model_or_voice_anywhere_says_where_to_set_one() {
    let none = VoiceOverrides::default();
    let mut settings = VoiceSettings {
        engine: Some(Engine::Server),
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
        &VoiceSettings {
            engine: Some(Engine::Server),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(
        e.contains("Settings -> Studio -> Narration") && e.contains("--server"),
        "{e}"
    );
    let flags = VoiceOverrides {
        engine: Some(Engine::Server),
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
        engine: Some(Engine::Server),
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
        serde_json::json!({ "keep": 1, "engine": "server", "baseUrl": "http://127.0.0.1:8880",
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
    settings.engine = Some(Engine::Kokoro);
    save(&pool, &settings).await.unwrap();
    let raw = crate::db::store::setting(&pool, SETTINGS_KEY)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(settings_from(Some(&raw)).engine, Some(Engine::Kokoro));
    settings.engine = None;
    save(&pool, &settings).await.unwrap();
    let raw = crate::db::store::setting(&pool, SETTINGS_KEY)
        .await
        .unwrap()
        .unwrap();
    assert!(!raw.contains("engine"), "{raw}");
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

#[test]
fn a_row_without_an_engine_is_a_server_when_it_has_a_url() {
    let engine = |row: &str| settings_from(Some(row)).engine;
    assert_eq!(
        engine(r#"{"baseUrl":"http://127.0.0.1:8880","voice":"af_heart"}"#),
        Some(Engine::Server),
        "a URL is the server engine's"
    );
    assert_eq!(engine(r#"{"voice":"af_heart","speed":1.1}"#), None);
    assert_eq!(
        engine(r#"{"engine":"kokoro","baseUrl":"http://x:1"}"#),
        Some(Engine::Kokoro)
    );
    assert_eq!(engine(r#"{"engine":"server"}"#), Some(Engine::Server));
    assert_eq!(
        engine(r#"{"engine":"piper","baseUrl":"http://x:1"}"#),
        None,
        "an unknown engine is unset, not guessed"
    );
    assert_eq!(engine(r#"{"engine":3,"baseUrl":"http://x:1"}"#), None);
}

#[test]
fn no_engine_anywhere_says_where_to_choose_one() {
    let none = VoiceOverrides::default();
    let settings = VoiceSettings {
        voice: Some("af_heart".into()),
        ..Default::default()
    };
    assert_eq!(resolve(&none, &none, &settings).unwrap_err(), NO_ENGINE);
    assert!(NO_ENGINE.contains("Settings -> Studio -> Narration"));
}

#[test]
fn kokoro_needs_only_a_voice_and_takes_the_engine_from_the_first_that_sets_it() {
    let none = VoiceOverrides::default();
    let settings = VoiceSettings {
        engine: Some(Engine::Server),
        base_url: Some("http://settings:1".into()),
        model: Some("settings-model".into()),
        voice: Some("af_heart".into()),
        speed: 0.9,
    };
    let video = VoiceOverrides {
        engine: Some(Engine::Kokoro),
        ..Default::default()
    };
    assert_eq!(
        resolve(&none, &video, &settings).unwrap(),
        Voice::Kokoro(KokoroVoice {
            voice: "af_heart".into(),
            speed: 0.9
        }),
        "the server fields are not read"
    );
    let flags = VoiceOverrides {
        engine: Some(Engine::Server),
        ..Default::default()
    };
    assert_eq!(
        resolve(&flags, &video, &settings).unwrap().engine(),
        Engine::Server
    );

    let kokoro = VoiceSettings {
        engine: Some(Engine::Kokoro),
        ..Default::default()
    };
    assert_eq!(resolve(&none, &none, &kokoro).unwrap_err(), NO_VOICE);
    let voice = resolve(&over(None, Some("am_michael"), Some(1.2)), &none, &kokoro).unwrap();
    assert_eq!(
        (voice.engine(), voice.voice(), voice.speed()),
        (Engine::Kokoro, "am_michael", 1.2)
    );
    for bad in ["../af_heart", "AF_HEART", "af heart"] {
        let e = resolve(&over(None, Some(bad), None), &none, &kokoro).unwrap_err();
        assert!(e.contains("not a voice id"), "{bad}: {e}");
    }
}

#[test]
fn a_video_blocks_engine_reaches_the_overrides() {
    let block = studio_core::Voice {
        engine: Some(Engine::Kokoro),
        model: None,
        voice: Some("am_michael".into()),
        speed: None,
    };
    let over = VoiceOverrides::from(&block);
    assert_eq!(over.engine, Some(Engine::Kokoro));
    assert_eq!(over.voice.as_deref(), Some("am_michael"));
}
