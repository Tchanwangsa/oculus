//! A parsed file renamed: its pictures folder is named off its stem
//! (`<stem>_images/`), and the markdown links to them by that name, so the
//! links move with it. The parse record's block offsets point into each
//! page's markdown (UTF-16 code units), so they shift by what each rewritten
//! link grew or shrank.

use std::path::Path;

/// `text` with every `](<old>/` link made `](<new>/`, and where each one was:
/// the UTF-16 offset of its `](` in `text`, and how many code units it grew.
pub fn relink_text(text: &str, old: &str, new: &str) -> (String, Vec<(usize, isize)>) {
    let from = format!("]({old}/");
    let to = format!("]({new}/");
    let grew = utf16_len(&to) as isize - utf16_len(&from) as isize;
    let mut out = String::with_capacity(text.len());
    let mut shifts = Vec::new();
    let mut rest = text;
    let mut at = 0;
    while let Some(found) = rest.find(&from) {
        let before = &rest[..found];
        out.push_str(before);
        at += utf16_len(before);
        shifts.push((at, grew));
        out.push_str(&to);
        at += utf16_len(&from);
        rest = &rest[found + from.len()..];
    }
    out.push_str(rest);
    (out, shifts)
}

/// An offset into the old text, in the new one.
pub fn shifted(offset: usize, shifts: &[(usize, isize)]) -> usize {
    let grew: isize = shifts
        .iter()
        .filter(|(at, _)| *at < offset)
        .map(|(_, grew)| grew)
        .sum();
    (offset as isize + grew).max(0) as usize
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Rewrite the parse record at `record`: its `pdf` name, each page's links
/// and the block offsets into them. Read as plain JSON, so fields this
/// version does not know survive.
pub fn relink_record(record: &Path, old: &str, new: &str, pdf_name: &str) -> Result<(), String> {
    let text = std::fs::read_to_string(record).map_err(|e| e.to_string())?;
    let mut value: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if let Some(pdf) = value.get_mut("pdf") {
        *pdf = serde_json::Value::String(pdf_name.to_string());
    }
    for page in value
        .get_mut("pages")
        .and_then(|p| p.as_array_mut())
        .into_iter()
        .flatten()
    {
        let Some(markdown) = page.get("markdown").and_then(|m| m.as_str()) else {
            continue;
        };
        let (relinked, shifts) = relink_text(markdown, old, new);
        if shifts.is_empty() {
            continue;
        }
        page["markdown"] = serde_json::Value::String(relinked);
        for block in page
            .get_mut("blocks")
            .and_then(|b| b.as_array_mut())
            .into_iter()
            .flatten()
        {
            for key in ["start", "end"] {
                if let Some(offset) = block.get(key).and_then(|o| o.as_u64()) {
                    block[key] = shifted(offset as usize, &shifts).into();
                }
            }
        }
    }
    let body = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    let tmp = record.with_extension(format!("json.drive-{}", std::process::id()));
    crate::runtime::atomic_write::write(record, &tmp, &body)
}

/// Rewrite the links in the parsed text at `md`.
pub fn relink_markdown(md: &Path, old: &str, new: &str) -> Result<(), String> {
    let text = std::fs::read_to_string(md).map_err(|e| e.to_string())?;
    let (relinked, shifts) = relink_text(&text, old, new);
    if shifts.is_empty() {
        return Ok(());
    }
    let tmp = crate::library::files::documents::note_temp_path(md)?;
    crate::runtime::atomic_write::write(md, &tmp, relinked.as_bytes())
}
