use std::path::PathBuf;

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::Nonce;

use super::sealed::{cipher, open_sealed, NONCE_LEN, TAG_LEN, VERSION};
use super::*;
use crate::names;
use crate::paths::vault as path_in;
use std::fs::File;
use std::io::Read;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let mut r = [0u8; 4];
        getrandom::getrandom(&mut r).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "keyd-vault-{name}-{}-{:08x}",
            std::process::id(),
            u32::from_ne_bytes(r)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn key(byte: u8) -> MasterKey {
    MasterKey::from_bytes([byte; 32])
}

#[test]
fn values_round_trip_through_a_fresh_handle() {
    let dir = Scratch::new("round-trip");
    let path = path_in(&dir.0);
    let v = Vault::new(&path, key(1));
    assert!(
        v.load().unwrap().is_empty(),
        "a missing file is an empty vault"
    );
    v.store(names::VOYAGE, "pa-123").unwrap();
    v.store(names::OKTA_PASSWORD, "hunter2 ünïcode \"quoted\"\n")
        .unwrap();

    let again = Vault::new(&path, key(1));
    assert_eq!(again.get(names::VOYAGE).unwrap().as_deref(), Some("pa-123"));
    assert_eq!(
        again.get(names::OKTA_PASSWORD).unwrap().as_deref(),
        Some("hunter2 ünïcode \"quoted\"\n")
    );
    assert!(again.has(names::VOYAGE).unwrap());
    assert!(!again.has(names::GROQ).unwrap());

    assert!(again.remove(names::VOYAGE).unwrap());
    assert!(!again.remove(names::VOYAGE).unwrap());
    assert!(!v.has(names::VOYAGE).unwrap());
    assert!(v.has(names::OKTA_PASSWORD).unwrap());
}

#[test]
fn the_wrong_key_fails_and_never_overwrites() {
    let dir = Scratch::new("wrong-key");
    let path = path_in(&dir.0);
    Vault::new(&path, key(1))
        .store(names::GROQ, "gsk-1")
        .unwrap();
    let before = std::fs::read(&path).unwrap();

    let wrong = Vault::new(&path, key(2));
    assert_eq!(wrong.load().unwrap_err(), VaultError::Undecryptable);
    assert_eq!(
        wrong.has(names::GROQ).unwrap_err(),
        VaultError::Undecryptable
    );
    assert_eq!(
        wrong.store(names::MINERU, "x").unwrap_err(),
        VaultError::Undecryptable
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "a failed open must leave the file alone"
    );
    assert_eq!(
        Vault::new(&path, key(1))
            .get(names::GROQ)
            .unwrap()
            .as_deref(),
        Some("gsk-1")
    );
}

#[test]
fn truncated_or_altered_files_fail_cleanly() {
    let dir = Scratch::new("damaged");
    let path = path_in(&dir.0);
    let v = Vault::new(&path, key(3));
    v.store(names::MINERU, "token").unwrap();
    let good = std::fs::read(&path).unwrap();

    for len in [0, 1, 12, 1 + NONCE_LEN + TAG_LEN - 1] {
        std::fs::write(&path, &good[..len]).unwrap();
        assert!(
            matches!(v.load(), Err(VaultError::Damaged(_))),
            "truncated to {len}"
        );
    }
    // Long enough to parse, short of the real ciphertext.
    std::fs::write(&path, &good[..good.len() - 1]).unwrap();
    assert_eq!(v.load().unwrap_err(), VaultError::Undecryptable);

    let mut flipped = good.clone();
    let last = flipped.len() - 1;
    flipped[last] ^= 0x01;
    std::fs::write(&path, &flipped).unwrap();
    assert_eq!(v.load().unwrap_err(), VaultError::Undecryptable);

    let mut nonce = good.clone();
    nonce[3] ^= 0x80;
    std::fs::write(&path, &nonce).unwrap();
    assert_eq!(v.load().unwrap_err(), VaultError::Undecryptable);

    let mut version = good.clone();
    version[0] = 9;
    std::fs::write(&path, &version).unwrap();
    assert!(matches!(v.load(), Err(VaultError::Damaged(_))));

    // Sealed correctly, but not a map of strings.
    let plain = br#"{"voyage": 5}"#;
    let n = [7u8; NONCE_LEN];
    let sealed = cipher(&key(3))
        .encrypt(
            Nonce::from_slice(&n),
            Payload {
                msg: plain,
                aad: &[VERSION],
            },
        )
        .unwrap();
    let mut odd = vec![VERSION];
    odd.extend_from_slice(&n);
    odd.extend_from_slice(&sealed);
    std::fs::write(&path, &odd).unwrap();
    assert!(matches!(v.load(), Err(VaultError::Damaged(_))));
}

#[test]
fn a_write_replaces_the_file_by_rename() {
    let dir = Scratch::new("atomic");
    let path = path_in(&dir.0);
    let v = Vault::new(&path, key(4));
    v.store(names::VOYAGE, "old").unwrap();

    // A reader that opened the old file keeps reading the whole old file.
    let mut held = File::open(&path).unwrap();
    v.store(names::VOYAGE, "new").unwrap();
    let mut old = Vec::new();
    held.read_to_end(&mut old).unwrap();
    assert_eq!(
        open_sealed(&key(4), &old).unwrap().get(names::VOYAGE),
        Some("old")
    );
    assert_eq!(v.get(names::VOYAGE).unwrap().as_deref(), Some("new"));

    let mut leftovers: Vec<String> = std::fs::read_dir(&dir.0)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    leftovers.sort();
    assert_eq!(
        leftovers,
        ["vault.bin", "vault.bin.lock"],
        "no temp file left behind"
    );
    // Its mode is the platform's to test (`platform/unix.rs`).
}

#[test]
fn every_write_draws_a_fresh_nonce() {
    let dir = Scratch::new("nonce");
    let path = path_in(&dir.0);
    let v = Vault::new(&path, key(5));
    v.store(names::GROQ, "a").unwrap();
    let first = std::fs::read(&path).unwrap();
    v.store(names::GROQ, "b").unwrap();
    v.store(names::GROQ, "a").unwrap();
    let third = std::fs::read(&path).unwrap();
    assert_ne!(first[1..1 + NONCE_LEN], third[1..1 + NONCE_LEN]);
    assert_ne!(first, third);
}

#[test]
fn an_unchanged_update_does_not_rewrite() {
    let dir = Scratch::new("unchanged");
    let path = path_in(&dir.0);
    let v = Vault::new(&path, key(6));
    v.store(names::GROQ, "a").unwrap();
    let before = std::fs::read(&path).unwrap();
    v.store(names::GROQ, "a").unwrap();
    assert!(!v.remove(names::MINERU).unwrap());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn concurrent_writers_lose_nothing() {
    let dir = Scratch::new("concurrent");
    let path = path_in(&dir.0);
    let threads: Vec<_> = (0..8)
        .map(|i| {
            let path = path.clone();
            std::thread::spawn(move || {
                Vault::new(path, key(7))
                    .store(&format!("t{i}"), "v")
                    .unwrap();
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let entries = Vault::new(&path, key(7)).load().unwrap();
    assert_eq!(entries.names().count(), 8);
}

#[test]
fn debug_output_never_carries_a_value() {
    let mut e = Entries::default();
    e.insert(names::VOYAGE, "pa-secret-value");
    let k = MasterKey::from_hex(&"ab".repeat(32)).unwrap();
    let shown = format!("{e:?} {k:?} {:?}", Vault::new("/x/vault.bin", k.clone()));
    assert!(shown.contains("voyage"));
    assert!(!shown.contains("pa-secret-value"));
    assert!(!shown.contains("abab"));
}

#[test]
fn the_master_key_hex_round_trips_and_rejects_junk() {
    let k = MasterKey::generate().unwrap();
    let hex = k.to_hex();
    assert_eq!(hex.len(), 64);
    assert_eq!(MasterKey::from_hex(&hex).unwrap().0, k.0);
    assert_eq!(MasterKey::from_hex(&format!("{hex}\n")).unwrap().0, k.0);
    assert!(MasterKey::from_hex(&hex[..62]).is_none());
    assert!(MasterKey::from_hex(&"zz".repeat(32)).is_none());
    assert!(MasterKey::from_hex(&"é".repeat(32)).is_none());
}
