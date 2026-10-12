use super::voice::{validated, VoiceRow};
use crate::studio::voice::VoiceSettings;

fn row(base_url: Option<&str>, model: Option<&str>, voice: Option<&str>, speed: f64) -> VoiceRow {
    VoiceRow {
        base_url: base_url.map(Into::into),
        model: model.map(Into::into),
        voice: voice.map(Into::into),
        speed,
    }
}

#[test]
fn a_row_is_trimmed_and_its_url_normalised() {
    let saved = validated(row(
        Some(" http://127.0.0.1:8880/v1/ "),
        Some(" kokoro "),
        Some("af_heart"),
        1.25,
    ))
    .unwrap();
    assert_eq!(
        saved,
        VoiceSettings {
            base_url: Some("http://127.0.0.1:8880".into()),
            model: Some("kokoro".into()),
            voice: Some("af_heart".into()),
            speed: 1.25,
        }
    );
}

#[test]
fn blank_fields_are_unset() {
    let saved = validated(row(Some("  "), Some(""), None, 1.0)).unwrap();
    assert_eq!(saved, VoiceSettings::default());
}

#[test]
fn a_bad_url_or_speed_is_refused_with_a_reason() {
    let e = validated(row(Some("127.0.0.1:8880"), None, None, 1.0)).unwrap_err();
    assert!(e.contains("http://"), "{e}");
    for speed in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let e = validated(row(None, None, None, speed)).unwrap_err();
        assert!(e.contains("positive"), "{e}");
    }
}

#[test]
fn the_row_serialises_camel_case_with_nulls() {
    let json = serde_json::to_value(VoiceRow::from(VoiceSettings::default())).unwrap();
    assert_eq!(
        json,
        serde_json::json!({ "baseUrl": null, "model": null, "voice": null, "speed": 1.0 })
    );
    let back: VoiceRow =
        serde_json::from_value(serde_json::json!({ "baseUrl": "http://x:1", "model": null,
            "voice": "v", "speed": 0.9 }))
        .unwrap();
    assert_eq!(back, row(Some("http://x:1"), None, Some("v"), 0.9));
}
