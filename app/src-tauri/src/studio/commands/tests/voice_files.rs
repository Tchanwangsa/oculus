use super::super::voice_files::{voice, voices_status};
use crate::runtime::onnx::Status;
use crate::studio::tts::kokoro::VOICE_BYTES;
use crate::test_support::Scratch;

fn fake_voice(dir: &std::path::Path, id: &str) {
    std::fs::File::create(dir.join(format!("{id}.bin")))
        .unwrap()
        .set_len(VOICE_BYTES)
        .unwrap();
}

#[test]
fn every_catalogued_voice_is_listed_with_its_status() {
    let dir = Scratch::new("voice-files-status");
    let none = voices_status(&dir);
    assert_eq!(none.len(), 20);
    assert!(none.iter().all(|v| v.status == Status::Missing));

    fake_voice(&dir, "af_heart");
    fake_voice(&dir, "am_puck");
    std::fs::write(dir.join("af_bella.bin"), b"cut short").unwrap();
    fake_voice(&dir, "bf_emma");
    let some = voices_status(&dir);
    let present: Vec<_> = some
        .iter()
        .filter(|v| v.status == Status::Present)
        .map(|v| v.id)
        .collect();
    assert_eq!(present, ["af_heart", "am_puck"]);
    assert_eq!(
        some.len(),
        20,
        "a voice outside the catalogue is not listed"
    );
}

#[test]
fn a_row_serialises_for_the_table() {
    let dir = Scratch::new("voice-files-json");
    fake_voice(&dir, "af_heart");
    let json = serde_json::to_value(voices_status(&dir)).unwrap();
    assert_eq!(
        json[3],
        serde_json::json!({
            "id": "af_heart",
            "name": "Heart",
            "accent": "American",
            "gender": "female",
            "grade": "A",
            "bytes": 522_240,
            "status": "present",
        })
    );
    assert_eq!(json[19]["id"], "am_santa");
    assert_eq!(json[19]["gender"], "male");
    assert_eq!(json[19]["status"], "missing");
}

#[test]
fn an_id_outside_the_catalogue_is_refused_by_name() {
    assert_eq!(voice("af_heart").unwrap().name, "Heart");
    for bad in ["bf_emma", "../af_heart", ""] {
        let e = voice(bad).err().unwrap();
        assert!(e.contains(&format!("`{bad}`")), "{e}");
    }
}
