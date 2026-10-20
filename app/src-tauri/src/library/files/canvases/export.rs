//! Image export writes only a destination returned by this command's save dialog.
use std::io::Cursor;
use std::path::{Path, PathBuf};

use base64::Engine;
use image::ImageDecoder;
use tauri_plugin_dialog::DialogExt;

const MAX_EXPORT_BYTES: usize = 64 * 1024 * 1024;

fn extension(format: &str) -> Result<&'static str, String> {
    match format {
        "png" => Ok("png"),
        "svg" => Ok("svg"),
        _ => Err("Canvas export supports PNG and SVG".into()),
    }
}

fn validate(format: &str, bytes: &[u8]) -> Result<(), String> {
    extension(format)?;
    if bytes.is_empty() || bytes.len() > MAX_EXPORT_BYTES {
        return Err("Canvas image must be between 1 byte and 64 MB".into());
    }
    if format == "png" {
        let decoder = image::codecs::png::PngDecoder::new(Cursor::new(bytes))
            .map_err(|e| format!("Invalid PNG image: {e}"))?;
        let (width, height) = decoder.dimensions();
        if width == 0 || height == 0 || width > 8192 || height > 8192 {
            return Err("Canvas PNG dimensions must be between 1 and 8192 pixels".into());
        }
    } else {
        let svg = std::str::from_utf8(bytes)
            .map_err(|_| "SVG image must be UTF-8")?
            .trim_start_matches('\u{feff}')
            .trim();
        let rest = svg.strip_prefix("<svg").ok_or("Invalid SVG image")?;
        if !rest.starts_with(|ch: char| ch.is_ascii_whitespace() || ch == '>')
            || !(svg.ends_with("</svg>") || svg.ends_with("/>"))
        {
            return Err("Invalid SVG image".into());
        }
    }
    Ok(())
}

fn decode(format: &str, data: &str) -> Result<Vec<u8>, String> {
    if data.len() > MAX_EXPORT_BYTES.div_ceil(3) * 4 {
        return Err("Canvas image exceeds 64 MB".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| format!("Invalid image encoding: {e}"))?;
    validate(format, &bytes)?;
    Ok(bytes)
}

fn destination(path: &Path, format: &str) -> Result<PathBuf, String> {
    let ext = extension(format)?;
    let mut chosen = path.to_path_buf();
    if chosen.extension().is_none() {
        chosen.set_extension(ext);
    }
    if !chosen
        .extension()
        .is_some_and(|value| value.eq_ignore_ascii_case(ext))
    {
        return Err(format!("Choose a .{ext} filename"));
    }
    let parent = chosen.parent().ok_or("Export destination has no folder")?;
    let parent = parent.canonicalize().map_err(|e| e.to_string())?;
    let name = chosen
        .file_name()
        .ok_or("Export destination has no filename")?;
    let path = parent.join(name);
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => {
            return Err("Export destination must be a regular file".into());
        }
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
        _ => {}
    }
    Ok(path)
}

fn write_chosen(path: &Path, format: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    validate(format, bytes)?;
    let path = destination(path, format)?;
    let temporary = path.with_file_name(format!(
        ".canvas-export-{}-{}.tmp",
        std::process::id(),
        crate::runtime::clock::now_nanos()
    ));
    crate::runtime::atomic_write::write(&path, &temporary, bytes)?;
    Ok(path)
}

#[tauri::command]
pub async fn export_canvas_image(
    app: tauri::AppHandle,
    format: String,
    data: String,
) -> Result<Option<String>, String> {
    crate::runtime::blocking::run(move || {
        let bytes = decode(&format, &data)?;
        let ext = extension(&format)?;
        let picked = app
            .dialog()
            .file()
            .set_title("Export canvas")
            .set_file_name(format!("Canvas.{ext}"))
            .add_filter(
                if ext == "png" {
                    "PNG image"
                } else {
                    "SVG image"
                },
                &[ext],
            )
            .blocking_save_file();
        let Some(picked) = picked else {
            return Ok(None);
        };
        let picked = picked.into_path().map_err(|e| e.to_string())?;
        let path = write_chosen(&picked, &format, &bytes)?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    const SVG: &[u8] =
        br#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="1" height="1"/></svg>"#;

    #[test]
    fn exports_validate_format_and_write_only_matching_files_atomically() {
        let root = crate::test_support::Scratch::new("canvas-export");
        let target = root.join("drawing.svg");
        std::fs::write(&target, "old").unwrap();
        write_chosen(&target, "svg", SVG).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), SVG);
        assert_eq!(std::fs::read_dir(&*root).unwrap().count(), 1);
        assert!(write_chosen(&root.join("drawing.rs"), "svg", SVG).is_err());
        assert!(write_chosen(&target, "svg", b"not svg").is_err());
        assert!(write_chosen(&target, "png", SVG).is_err());
        assert!(decode("svg", "invalid base64!").is_err());
        assert!(decode(
            "html",
            &base64::engine::general_purpose::STANDARD.encode(SVG)
        )
        .is_err());
        assert_eq!(std::fs::read(&target).unwrap(), SVG);
        assert_eq!(
            write_chosen(&root.join("extensionless"), "svg", SVG).unwrap(),
            root.canonicalize().unwrap().join("extensionless.svg")
        );
    }

    #[test]
    fn png_export_requires_png_bytes_and_real_dimensions() {
        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 2)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let bytes = decode(
            "png",
            &base64::engine::general_purpose::STANDARD.encode(png.into_inner()),
        )
        .unwrap();
        let root = crate::test_support::Scratch::new("canvas-export-png");
        write_chosen(&root.join("drawing.PNG"), "png", &bytes).unwrap();
        assert!(validate("png", b"\x89PNG\r\n\x1a\n").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn export_refuses_a_symlink_destination() {
        let root = crate::test_support::Scratch::new("canvas-export-links");
        let other = root.join("other.svg");
        std::fs::write(&other, "private").unwrap();
        let alias = root.join("alias.svg");
        std::os::unix::fs::symlink(&other, &alias).unwrap();
        assert!(write_chosen(&alias, "svg", SVG).is_err());
        assert_eq!(std::fs::read_to_string(other).unwrap(), "private");
    }
}
