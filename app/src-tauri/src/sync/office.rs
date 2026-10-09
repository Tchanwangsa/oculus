//! File-type rules, and Office documents converted to PDF with LibreOffice.

use std::path::{Path, PathBuf};

use crate::library::paths;
use crate::sync::{OFFICE_CONVERT_TIMEOUT, OFFICE_TYPES, SHEET_TYPES, VIDEO_EXTS};

pub(super) fn office_ext(ct: &str) -> Option<&'static str> {
    OFFICE_TYPES.iter().find(|(k, _)| *k == ct).map(|(_, v)| *v)
}

/// The only case where the filename decides the type.
pub(super) fn is_generic_binary(ct: &str) -> bool {
    matches!(ct, "" | "application/octet-stream" | "binary/octet-stream")
}

/// Videos are listed in modules and downloaded only on request, by type or,
/// for an untyped upload, by extension.
pub(super) fn is_video(ct: &str, name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    ct.starts_with("video/")
        || (is_generic_binary(ct) && VIDEO_EXTS.iter().any(|e| lower.ends_with(&format!(".{e}"))))
}

/// A spreadsheet type (or an untyped upload) with a spreadsheet's name: every
/// later gate goes by the extension, and Windows browsers label a `.csv` as
/// `application/vnd.ms-excel`.
pub(super) fn is_sheet_type(ct: &str, name: &str) -> bool {
    (SHEET_TYPES.contains(&ct) || is_generic_binary(ct)) && paths::is_sheet(name)
}

/// A `.csv` under any type Canvas labels one with. It is converted like a
/// spreadsheet (`crate::pages::sheets`), so search finds it too.
pub(super) fn is_csv_type(ct: &str, name: &str) -> bool {
    let typed = matches!(
        ct,
        "text/csv"
            | "application/csv"
            | "text/comma-separated-values"
            | "text/plain"
            | "application/vnd.ms-excel"
    );
    (typed || is_generic_binary(ct)) && name.to_ascii_lowercase().ends_with(".csv")
}

/// The converter extension an untyped file's name claims, if known.
pub(crate) fn office_ext_of(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    OFFICE_TYPES
        .iter()
        .map(|(_, e)| *e)
        .find(|e| lower.ends_with(&format!(".{e}")))
}

/// Office → PDF via headless LibreOffice, in a private scratch directory that
/// is removed whatever the outcome.
pub(crate) fn office_to_pdf(bytes: &[u8], ext: &str) -> Result<Vec<u8>, String> {
    let soffice = find_soffice().ok_or_else(|| {
        "LibreOffice not installed — `brew install --cask libreoffice` enables Office → PDF conversion"
            .to_string()
    })?;

    let scratch = std::env::temp_dir().join(format!(
        "oculus-office-{}-{}",
        std::process::id(),
        crate::runtime::clock::now_nanos()
    ));
    std::fs::create_dir_all(&scratch).map_err(|e| format!("scratch dir: {e}"))?;
    let result = convert_in(&soffice, &scratch, bytes, ext);
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

fn convert_in(soffice: &Path, dir: &Path, bytes: &[u8], ext: &str) -> Result<Vec<u8>, String> {
    let input = dir.join(format!("input.{ext}"));
    std::fs::write(&input, bytes).map_err(|e| format!("write temp: {e}"))?;

    // A private UserInstallation lets this run while the LibreOffice GUI is
    // open — soffice otherwise refuses to start a second instance.
    let profile = url::Url::from_file_path(dir.join("profile"))
        .map_err(|_| "profile path not absolute".to_string())?;
    let mut child = std::process::Command::new(soffice)
        .arg(format!("-env:UserInstallation={profile}"))
        .args(["--headless", "--norestore", "--convert-to", "pdf"])
        .arg("--outdir")
        .arg(dir)
        .arg(&input)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("launch soffice: {e}"))?;

    // std has no wait-with-timeout, so poll.
    let deadline = std::time::Instant::now() + OFFICE_CONVERT_TIMEOUT;
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(s) => break s,
            None if std::time::Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("conversion timed out".to_string());
            }
            None => std::thread::sleep(std::time::Duration::from_millis(200)),
        }
    };
    if !status.success() {
        return Err(format!("soffice exited with {status}"));
    }
    std::fs::read(dir.join("input.pdf")).map_err(|e| format!("no PDF produced: {e}"))
}

fn find_soffice() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("OCULUS_SOFFICE") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    [
        "/Applications/LibreOffice.app/Contents/MacOS/soffice",
        "/opt/homebrew/bin/soffice",
        "/usr/local/bin/soffice",
        "/usr/bin/soffice",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|p| p.exists())
}
