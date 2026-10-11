use std::io::Cursor;
use std::sync::Mutex;

use sha2::{Digest, Sha256};

use super::claim::Claim;
use super::fetch::write_verified;
use super::*;
use crate::test_support::{FakeServer, Reply, Scratch};

fn sha(bytes: &[u8]) -> &'static str {
    Box::leak(format!("{:x}", Sha256::digest(bytes)).into_boxed_str())
}

fn file_for(name: &'static str, body: &[u8], url: String) -> PackFile {
    PackFile {
        file: name,
        url: Box::leak(url.into_boxed_str()),
        bytes: body.len() as u64,
        sha256: sha(body),
    }
}

fn pack_of(id: &'static str, files: Vec<PackFile>) -> Pack {
    Pack {
        id,
        files: Box::leak(files.into_boxed_slice()),
    }
}

#[test]
fn a_verified_stream_is_renamed_into_place() {
    let dir = Scratch::new("onnx-pack-ok");
    let body = vec![7u8; 200_000];
    let file = file_for("model.onnx", &body, String::new());
    let dest = dir.join("model.onnx");
    let last = Mutex::new(0);
    write_verified(
        &mut Cursor::new(&body),
        &dest,
        &file,
        &|n| *last.lock().unwrap() = n,
        &|| false,
    )
    .unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), body);
    assert!(!dir.join("model.onnx.part").exists());
    assert_eq!(*last.lock().unwrap(), 200_000);
}

#[test]
fn a_checksum_mismatch_deletes_the_part_and_names_both_digests() {
    let dir = Scratch::new("onnx-pack-sha");
    let body = vec![1u8; 1000];
    let mut file = file_for("model.onnx", &body, String::new());
    file.sha256 = sha(b"something else");
    let dest = dir.join("model.onnx");
    let error =
        write_verified(&mut Cursor::new(&body), &dest, &file, &|_| {}, &|| false).unwrap_err();
    assert!(error.contains("failed its checksum"), "{error}");
    assert!(error.contains(file.sha256), "{error}");
    assert!(
        error.contains(&format!("{:x}", Sha256::digest(&body))),
        "{error}"
    );
    assert!(!dest.exists() && !dir.join("model.onnx.part").exists());
}

#[test]
fn a_short_or_long_stream_fails_before_the_checksum() {
    let dir = Scratch::new("onnx-pack-size");
    let body = vec![1u8; 1000];
    let file = file_for("model.onnx", &body, String::new());
    let dest = dir.join("model.onnx");
    let short = write_verified(
        &mut Cursor::new(&body[..600]),
        &dest,
        &file,
        &|_| {},
        &|| false,
    )
    .unwrap_err();
    assert!(short.contains("stopped at 600 of 1000"), "{short}");
    let long = [body.clone(), vec![0; 10]].concat();
    let long =
        write_verified(&mut Cursor::new(&long), &dest, &file, &|_| {}, &|| false).unwrap_err();
    assert!(long.contains("ran past"), "{long}");
    assert!(!dest.exists() && !dir.join("model.onnx.part").exists());
}

#[test]
fn a_cancelled_stream_leaves_nothing() {
    let dir = Scratch::new("onnx-pack-cancel");
    let body = vec![1u8; 1000];
    let file = file_for("model.onnx", &body, String::new());
    let dest = dir.join("model.onnx");
    let error =
        write_verified(&mut Cursor::new(&body), &dest, &file, &|_| {}, &|| true).unwrap_err();
    assert_eq!(error, CANCELLED);
    assert!(!dest.exists() && !dir.join("model.onnx.part").exists());
}

#[test]
fn a_pack_downloads_its_missing_files_and_reports_progress_over_the_pack() {
    let a = vec![1u8; 3000];
    let b = vec![2u8; 5000];
    let (sa, sb) = (a.clone(), b.clone());
    let server = FakeServer::start(move |hit| match hit.url.as_str() {
        "/a.bin" => Reply::bytes(sa.clone()),
        "/b.bin" => Reply::bytes(sb.clone()),
        _ => Reply::status(404, serde_json::json!({})),
    });
    let dir = Scratch::new("onnx-pack-download");
    let pack = pack_of(
        "test-pack-download",
        vec![
            file_for("a.bin", &a, format!("{}/a.bin", server.origin())),
            file_for("b.bin", &b, format!("{}/b.bin", server.origin())),
        ],
    );
    assert!(!is_present(&dir, &pack));
    let seen = Mutex::new(Vec::new());
    download(
        &dir,
        &pack,
        |received, total| seen.lock().unwrap().push((received, total)),
        || false,
    )
    .unwrap();
    assert!(is_present(&dir, &pack));
    let seen = seen.into_inner().unwrap();
    assert_eq!(seen.first(), Some(&(0, 8000)));
    assert_eq!(seen.last(), Some(&(8000, 8000)));
    assert!(!downloading(pack.id));
    // A second download fetches nothing: both files are at their pinned size.
    download(&dir, &pack, |_, _| {}, || false).unwrap();
    assert_eq!(server.hits().len(), 2);
    assert_eq!(delete(&dir, &pack).unwrap(), 8000);
    assert!(!is_present(&dir, &pack));
}

#[test]
fn a_server_length_other_than_the_pinned_one_is_refused() {
    let server = FakeServer::start(|_| Reply::bytes(vec![0u8; 10]));
    let dir = Scratch::new("onnx-pack-length");
    let pack = pack_of(
        "test-pack-length",
        vec![file_for(
            "a.bin",
            &[0u8; 12],
            format!("{}/a.bin", server.origin()),
        )],
    );
    let error = download(&dir, &pack, |_, _| {}, || false).unwrap_err();
    assert!(
        error.contains("not the pinned 12") || error.contains("stopped at 10 of 12"),
        "{error}"
    );
    assert!(!dir.join("a.bin").exists() && !dir.join("a.bin.part").exists());
}

#[test]
fn a_second_download_of_one_pack_is_refused_and_cancel_reaches_the_first() {
    let first = Claim::take("test-pack-claim").unwrap();
    assert!(Claim::take("test-pack-claim").is_err());
    assert!(downloading("test-pack-claim"));
    assert!(cancel("test-pack-claim"));
    assert!(first.flag().load(std::sync::atomic::Ordering::Relaxed));
    drop(first);
    assert!(!cancel("test-pack-claim"));
}

#[test]
fn deleting_removes_files_and_partials() {
    let dir = Scratch::new("onnx-pack-delete");
    let pack = pack_of(
        "test-pack-delete",
        vec![file_for("a.bin", &[0u8; 4], String::new())],
    );
    std::fs::write(dir.join("a.bin"), [0u8; 4]).unwrap();
    std::fs::write(dir.join("a.bin.part"), [0u8; 3]).unwrap();
    assert_eq!(delete(&dir, &pack).unwrap(), 7);
    assert_eq!(delete(&dir, &pack).unwrap(), 0);
}

#[test]
fn threads_are_half_the_cores_at_most_four() {
    let threads = crate::runtime::onnx::intra_threads();
    assert!((1..=4).contains(&threads));
}
