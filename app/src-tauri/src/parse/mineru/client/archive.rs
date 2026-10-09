//! The result archive: page counts, safe extraction, and finding the content list.

use crate::parse::ParseError;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

pub(in crate::parse::mineru) fn page_count(pdf: &Path) -> Result<u32, ParseError> {
    let document = lopdf::Document::load(pdf).map_err(|_| ParseError::Document {
        code: "unreadable-pdf".into(),
    })?;
    let pages = document.get_pages().len() as u32;
    if pages == 0 {
        return Err(ParseError::Document {
            code: "empty-pdf".into(),
        });
    }
    Ok(pages)
}

/// Extract with a zip-slip guard: every member must resolve inside the
/// destination.
pub(in crate::parse::mineru) fn safe_extract(
    zip_path: &Path,
    destination: &Path,
) -> Result<(), ParseError> {
    fs::create_dir_all(destination)
        .map_err(|e| ParseError::Io(format!("create {}: {e}", destination.display())))?;
    let file = fs::File::open(zip_path)
        .map_err(|e| ParseError::Io(format!("open {}: {e}", zip_path.display())))?;
    let mut archive =
        zip::ZipArchive::new(BufReader::new(file)).map_err(|_| ParseError::Document {
            code: "invalid-result-zip".into(),
        })?;

    for index in 0..archive.len() {
        let mut member = archive.by_index(index).map_err(|_| ParseError::Document {
            code: "invalid-result-zip".into(),
        })?;
        let name = member.enclosed_name().ok_or(ParseError::Document {
            code: "unsafe-zip-path".into(),
        })?;
        let target = destination.join(&name);
        if !target.starts_with(destination) {
            return Err(ParseError::Document {
                code: "unsafe-zip-path".into(),
            });
        }
        if member.is_dir() {
            fs::create_dir_all(&target)
                .map_err(|e| ParseError::Io(format!("create {}: {e}", target.display())))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ParseError::Io(format!("create {}: {e}", parent.display())))?;
        }
        let mut out = fs::File::create(&target)
            .map_err(|e| ParseError::Io(format!("create {}: {e}", target.display())))?;
        std::io::copy(&mut member, &mut out)
            .map_err(|e| ParseError::Io(format!("extract {}: {e}", target.display())))?;
    }
    Ok(())
}

/// The first `*_content_list.json` under the extracted result, in stable order.
pub(in crate::parse::mineru) fn find_content_list(root: &Path) -> Option<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .map(|name| name.to_string_lossy().ends_with("_content_list.json"))
                .unwrap_or(false)
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found.into_iter().next()
}
