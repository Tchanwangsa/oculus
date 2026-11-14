//! The cases the measuring spike recorded (`browser-downloads-stage0.md`
//! §1, §3.1), as `Finished` events reach `settle`.

use std::path::Path;

use super::correlate::{settle, Ended, Owed, Pending};
use super::url_key;

const PAGE: u32 = 3;

fn key(url: &str) -> u64 {
    url_key(&url.parse().unwrap())
}

/// Unfinished downloads in `PAGE`: (id, url, staged path).
fn pending<'a>(rows: &'a [(u32, &'a str, &'a str)]) -> Vec<Pending<'a>> {
    rows.iter()
        .map(|&(id, url, staged)| Pending {
            id,
            page: PAGE,
            url_key: key(url),
            staged: Path::new(staged),
        })
        .collect()
}

fn ended<'a>(url: &str, path: Option<&'a str>, success: bool) -> Ended<'a> {
    Ended {
        page: PAGE,
        url_key: key(url),
        path: path.map(Path::new),
        success,
    }
}

/// `settle` as macOS reads a pathless failure (not ours) and as Windows does.
const MAC: bool = false;
const WINDOWS: bool = true;

#[test]
fn one_url_downloading_twice_settles_each_by_its_own_path_in_finish_order() {
    // `same-url-twice`: the second finished while the first was 35% written.
    let zip = "http://127.0.0.1:47811/same.zip";
    let rows = [(1, zip, "/s/1-1/same.zip"), (2, zip, "/s/1-2/same.zip")];
    let mut owed = vec![];
    let first = ended(zip, Some("/s/1-2/same.zip"), true);
    assert_eq!(settle(&pending(&rows), &mut owed, first, MAC), [(2, true)]);
    let second = ended(zip, Some("/s/1-1/same.zip"), true);
    assert_eq!(
        settle(&pending(&rows[..1]), &mut owed, second, MAC),
        [(1, true)]
    );
    assert!(owed.is_empty());
}

#[test]
fn a_blob_saved_twice_and_a_megabyte_data_url_need_nothing_special() {
    let blob = "blob:http://127.0.0.1:47811/0b1c3d4e-0000-4000-8000-000000000000";
    let data = format!("data:application/zip;base64,{}", "A".repeat(4_000_000));
    let rows = [
        (1, blob, "/s/1-1/export.csv"),
        (2, blob, "/s/1-2/export.csv"),
        (3, data.as_str(), "/s/1-3/Unknown.zip"),
    ];
    let mut owed = vec![];
    let done = ended(&data, Some("/s/1-3/Unknown.zip"), true);
    assert_eq!(settle(&pending(&rows), &mut owed, done, MAC), [(3, true)]);
    let failed = ended(blob, Some("/s/1-1/export.csv"), false);
    assert_eq!(
        settle(&pending(&rows[..2]), &mut owed, failed, MAC),
        [(1, false)]
    );
}

#[test]
fn webview2_hands_the_path_back_in_its_verbatim_form() {
    let pdf = "https://example.com/a.pdf";
    let rows = [(1, pdf, r"C:\s\1-1\a.pdf"), (2, pdf, r"C:\s\1-2\a.pdf")];
    let mut owed = vec![];
    let done = ended(pdf, Some(r"\\?\C:\s\1-2\a.pdf"), true);
    assert_eq!(
        settle(&pending(&rows), &mut owed, done, WINDOWS),
        [(2, true)]
    );
}

#[test]
fn a_path_that_names_no_download_settles_nothing_whatever_its_url() {
    let pdf = "https://example.com/a.pdf";
    let rows = [(1, pdf, "/s/1-1/a.pdf")];
    let mut owed = vec![];
    let elsewhere = ended(pdf, Some("/Users/student/Downloads/a.pdf"), true);
    assert!(settle(&pending(&rows), &mut owed, elsewhere, MAC).is_empty());
    assert!(owed.is_empty());
}

#[test]
fn the_orphan_of_a_cross_origin_download_redirect_is_dropped_on_macos() {
    // `dl-redirect`: `Finished { success: false }` with the href and no
    // `Requested`, then the signed URL downloads for real.
    let href = "http://127.0.0.1:47812/redirect/doc.pdf";
    let signed = "http://127.0.0.1:47811/signed/doc.pdf?token=SECRET";
    let mut owed = vec![];
    let rows = [(1, signed, "/s/1-1/doc.pdf")];
    assert!(settle(&pending(&rows), &mut owed, ended(href, None, false), MAC).is_empty());
    // Even with a download of that very href unfinished in the page.
    let rows = [(1, href, "/s/1-1/doc.pdf")];
    assert!(settle(&pending(&rows), &mut owed, ended(href, None, false), MAC).is_empty());
    assert!(owed.is_empty());
}

#[test]
fn a_denied_download_settles_nothing() {
    // `deny`: the hook answered false (a refused playlist), path `None`.
    let hls = "https://media.example.com/master.m3u8";
    let mut owed = vec![];
    assert!(settle(&[], &mut owed, ended(hls, None, false), MAC).is_empty());
    assert!(settle(&[], &mut owed, ended(hls, None, false), WINDOWS).is_empty());
    assert!(
        owed.is_empty(),
        "nothing waits on a URL nothing is downloading"
    );
}

#[test]
fn on_windows_a_pathless_failure_settles_the_only_download_of_its_url() {
    let zip = "https://example.com/data.zip";
    let rows = [
        (1, zip, r"C:\s\1-1\data.zip"),
        (2, "https://example.com/b", r"C:\s\1-2\b"),
    ];
    let mut owed = vec![];
    assert_eq!(
        settle(&pending(&rows), &mut owed, ended(zip, None, false), WINDOWS),
        [(1, false)]
    );
    assert!(owed.is_empty());
}

#[test]
fn on_windows_a_pathless_failure_never_picks_among_several_of_its_url() {
    let zip = "https://example.com/data.zip";
    let rows = [
        (1, zip, r"C:\s\1-1\data.zip"),
        (2, zip, r"C:\s\1-2\data.zip"),
    ];
    let mut owed = vec![];
    assert!(settle(&pending(&rows), &mut owed, ended(zip, None, false), WINDOWS).is_empty());
    assert_eq!(owed.len(), 1, "owed to the URL until the group is in");
    // The other succeeds with its path: the failure was the one left.
    let done = ended(zip, Some(r"C:\s\1-2\data.zip"), true);
    assert_eq!(
        settle(&pending(&rows), &mut owed, done, WINDOWS),
        [(2, true), (1, false)]
    );
    assert!(owed.is_empty());
}

#[test]
fn a_pathless_success_means_the_patch_is_missing_and_settles_only_a_whole_group() {
    let zip = "https://example.com/data.zip";
    let rows = [(1, zip, "/s/1-1/data.zip"), (2, zip, "/s/1-2/data.zip")];
    let mut owed: Vec<Owed> = vec![];
    // One of two: which one is unknown, and the other may be half-written.
    assert!(settle(&pending(&rows), &mut owed, ended(zip, None, true), MAC).is_empty());
    // Both in: every byte of both is written.
    assert_eq!(
        settle(&pending(&rows), &mut owed, ended(zip, None, true), MAC),
        [(1, true), (2, true)]
    );
    // A lone one settles at once.
    let lone = [(3, zip, "/s/1-3/data.zip")];
    assert_eq!(
        settle(&pending(&lone), &mut owed, ended(zip, None, true), MAC),
        [(3, true)]
    );
}

#[test]
fn a_group_with_one_failure_fails_whole() {
    let zip = "https://example.com/data.zip";
    let rows = [
        (1, zip, r"C:\s\1-1\data.zip"),
        (2, zip, r"C:\s\1-2\data.zip"),
    ];
    let mut owed = vec![];
    settle(&pending(&rows), &mut owed, ended(zip, None, true), WINDOWS);
    assert_eq!(
        settle(&pending(&rows), &mut owed, ended(zip, None, false), WINDOWS),
        [(1, false), (2, false)]
    );
}

#[test]
fn url_groups_are_per_page() {
    let zip = "https://example.com/data.zip";
    let rows = [Pending {
        id: 1,
        page: PAGE + 1,
        url_key: key(zip),
        staged: Path::new("/s/1-1/data.zip"),
    }];
    let mut owed = vec![];
    assert!(settle(&rows, &mut owed, ended(zip, None, false), WINDOWS).is_empty());
    assert!(owed.is_empty());
}
