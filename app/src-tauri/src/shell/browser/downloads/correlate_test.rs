use std::path::Path;

use super::correlate::{match_finished, Pending};

const PDF: &str = "https://example.com/files/1/download";
const ZIP: &str = "https://example.com/data.zip";

fn pending<'a>(rows: &'a [(u32, &'a str, &'a str)]) -> Vec<Pending<'a>> {
    rows.iter()
        .map(|&(id, url, staged)| Pending {
            id,
            url,
            staged: Path::new(staged),
        })
        .collect()
}

#[test]
fn without_a_path_the_oldest_download_of_the_url_finishes_first() {
    let rows = [
        (1, PDF, "/s/1/a.pdf"),
        (2, ZIP, "/s/2/d.zip"),
        (3, PDF, "/s/3/a.pdf"),
    ];
    let p = pending(&rows);
    assert_eq!(match_finished(&p, PDF, None), Some(1));
    assert_eq!(match_finished(&p[1..], PDF, None), Some(3), "1 is done");
    assert_eq!(match_finished(&p, ZIP, None), Some(2));
    assert_eq!(match_finished(&p, "https://example.com/other", None), None);
}

#[test]
fn a_reported_path_names_its_download_over_url_order() {
    let rows = [(1, PDF, r"C:\s\1\a.pdf"), (2, PDF, r"C:\s\2\a.pdf")];
    let p = pending(&rows);
    assert_eq!(
        match_finished(&p, PDF, Some(Path::new(r"C:\s\2\a.pdf"))),
        Some(2)
    );
    assert_eq!(
        match_finished(&p, PDF, Some(Path::new(r"\\?\C:\s\2\a.pdf"))),
        Some(2),
        "WebView2's verbatim form"
    );
}

#[test]
fn a_path_that_names_none_falls_back_to_the_url() {
    let rows = [(1, PDF, "/s/1/a.pdf")];
    let p = pending(&rows);
    assert_eq!(
        match_finished(&p, PDF, Some(Path::new("/elsewhere/a.pdf"))),
        Some(1)
    );
    assert_eq!(
        match_finished(&p, ZIP, Some(Path::new("/elsewhere/a.pdf"))),
        None
    );
}
