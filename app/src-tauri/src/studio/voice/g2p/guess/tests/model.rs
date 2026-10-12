use sha2::{Digest, Sha256};

use super::super::{CONFIG_FILE, PACK};
use super::CONFIG;

#[test]
fn every_file_is_pinned_to_a_commit() {
    for file in PACK.files {
        let commit = file
            .url
            .split('/')
            .find(|part| part.len() == 40 && part.bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(commit.is_some(), "{} is not pinned", file.url);
        assert_eq!(file.sha256.len(), 64);
    }
}

#[test]
fn the_test_config_is_the_pinned_one() {
    let pinned = PACK.files.iter().find(|f| f.file == CONFIG_FILE).unwrap();
    assert_eq!(CONFIG.len() as u64, pinned.bytes);
    let sha: String = Sha256::digest(CONFIG.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(sha, pinned.sha256);
}
