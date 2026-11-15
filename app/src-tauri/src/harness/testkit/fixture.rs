use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::outbound::Expected;
use crate::harness::Provider;

#[derive(Deserialize)]
pub(super) struct Fixture {
    pub provider: Provider,
    pub cli_version: String,
    pub protocol: String,
    pub model: Option<String>,
    pub settings: Value,
    pub capture_date: String,
    pub source: String,
    pub redaction: String,
    pub scenario: String,
    pub frames: usize,
    pub sha256: String,
    pub expected_outbound: Vec<Expected>,
    #[serde(default)]
    input: Option<String>,
    #[serde(default)]
    pub native_session: String,
    /// How the recorded chat asked. Recordings made before requests ran
    /// under `never`, which is "Don't ask".
    #[serde(default)]
    pub permission_mode: Option<crate::harness::requests::PermissionMode>,
}

pub(super) fn directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/harness/turn-model")
}

impl Fixture {
    pub fn load(name: &str) -> (Self, Vec<Value>) {
        let path = directory().join(format!("{name}.manifest.json"));
        let fixture: Self = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for value in [
            &fixture.cli_version,
            &fixture.protocol,
            &fixture.capture_date,
            &fixture.source,
            &fixture.redaction,
            &fixture.scenario,
        ] {
            assert!(!value.is_empty(), "fixture manifest is incomplete: {name}");
        }
        assert!(fixture.model.as_ref().is_none_or(|m| !m.is_empty()));
        assert!(fixture.settings.is_object(), "fixture settings: {name}");
        let input = fixture
            .input
            .clone()
            .unwrap_or_else(|| format!("{name}.ndjson"));
        let raw = std::fs::read_to_string(directory().join(input)).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(raw.as_bytes())),
            fixture.sha256,
            "fixture hash: {name}"
        );
        let frames: Vec<Value> = raw
            .lines()
            .filter(|s| !s.trim().is_empty())
            .map(|line| serde_json::from_str(line).expect("recorded JSON frame"))
            .collect();
        assert_eq!(frames.len(), fixture.frames);
        (fixture, frames)
    }
}
