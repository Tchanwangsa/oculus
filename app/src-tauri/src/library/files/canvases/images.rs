//! Read dropped raster images without writing a copy into the library.
use std::io::Read;

use base64::Engine;

const MAX_BYTES: u64 = 24 * 1024 * 1024;

fn read_image(path: &std::path::Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Cannot read image: {e}"))?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_BYTES {
        return Err("Choose an image between 1 byte and 24 MB".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Choose an image up to 24 MB".into());
    }
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else {
        return Err("Choose a PNG, JPEG, WebP or GIF image".into());
    };
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[tauri::command]
pub async fn canvas_read_dropped_image(path: String) -> Result<String, String> {
    crate::runtime::blocking::run(move || read_image(std::path::Path::new(&path))).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_raster_bytes_and_caps_reads() {
        let dir = crate::test_support::Scratch::new("canvas-image-drop");
        let path = dir.join("image.bin");
        for (bytes, mime) in [
            (b"\x89PNG\r\n\x1a\n".as_slice(), "image/png"),
            (b"\xff\xd8\xff", "image/jpeg"),
            (b"GIF89a", "image/gif"),
            (b"RIFF0000WEBP", "image/webp"),
        ] {
            std::fs::write(&path, bytes).unwrap();
            assert!(read_image(&path)
                .unwrap()
                .starts_with(&format!("data:{mime};base64,")));
        }
        std::fs::write(&path, b"<svg/>").unwrap();
        assert!(read_image(&path).is_err());
        std::fs::write(&path, b"").unwrap();
        assert!(read_image(&path).is_err());
        std::fs::File::create(&path)
            .unwrap()
            .set_len(MAX_BYTES + 1)
            .unwrap();
        assert!(read_image(&path).is_err());
        assert!(read_image(&dir).is_err());
    }
}
