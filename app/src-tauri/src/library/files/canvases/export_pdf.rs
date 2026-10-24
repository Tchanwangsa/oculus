//! A notebook as a PDF: one page per notebook page, at its size in points, each
//! page's picture embedded losslessly. The editor draws each page (the assignment
//! under the student's writing) and sends the PNGs here; the save dialog and the
//! write are the image export's (`export.rs`).

use base64::Engine;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder, ImageFormat};
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream};
use serde::Deserialize;
use tauri_plugin_dialog::DialogExt;

const MAX_PAGES: usize = 500;
const MAX_SIDE_POINTS: f64 = 14400.0;
const MAX_PIXELS: u64 = 40_000_000;
const MAX_PNG_BYTES: usize = 64 * 1024 * 1024;

/// One page from the editor: its size in points and its picture as base64 PNG.
#[derive(Deserialize)]
pub struct PdfPage {
    width: f64,
    height: f64,
    png: String,
}

/// A page's picture as the PDF stores it: PNG's own compressed rows, which a Flate stream with a
/// PNG predictor reads directly, so nothing is recompressed or lost.
struct Picture {
    width_points: f64,
    height_points: f64,
    width: u32,
    height: u32,
    rows: Vec<u8>,
}

/// The concatenated IDAT data of a PNG: its zlib stream of filtered rows.
fn idat(png: &[u8]) -> Result<Vec<u8>, String> {
    let mut rows = Vec::new();
    let mut at = 8;
    while at + 8 <= png.len() {
        let length = u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]) as usize;
        let end = at
            .checked_add(12 + length)
            .filter(|end| *end <= png.len())
            .ok_or("A page picture is cut short")?;
        if &png[at + 4..at + 8] == b"IDAT" {
            rows.extend_from_slice(&png[at + 8..at + 8 + length]);
        }
        at = end;
    }
    if rows.is_empty() {
        return Err("A page picture has no image data".into());
    }
    Ok(rows)
}

/// Decodes a page picture and stores it as opaque RGB: the editor paints every page on white,
/// and a PDF image with no alpha needs no soft mask.
fn picture(page: PdfPage) -> Result<Picture, String> {
    let side = |points: f64| points.is_finite() && points > 0.0 && points <= MAX_SIDE_POINTS;
    if !side(page.width) || !side(page.height) {
        return Err("A page has an invalid size".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(page.png.as_bytes())
        .map_err(|_| "A page picture is not valid base64".to_string())?;
    if bytes.len() > MAX_PNG_BYTES {
        return Err("A page picture is over 64 MB".into());
    }
    let decoded = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
        .map_err(|e| format!("A page picture is not a PNG: {e}"))?
        .to_rgb8();
    let (width, height) = decoded.dimensions();
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err("A page picture is too large".into());
    }
    let mut encoded = Vec::new();
    PngEncoder::new_with_quality(&mut encoded, CompressionType::Default, FilterType::Adaptive)
        .write_image(decoded.as_raw(), width, height, ExtendedColorType::Rgb8)
        .map_err(|e| e.to_string())?;
    Ok(Picture {
        width_points: page.width,
        height_points: page.height,
        width,
        height,
        rows: idat(&encoded)?,
    })
}

/// The PDF's bytes: each picture drawn to fill its page.
fn build(pictures: Vec<Picture>) -> Result<Vec<u8>, String> {
    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let mut kids: Vec<Object> = Vec::with_capacity(pictures.len());
    for picture in pictures {
        let (w, h) = (picture.width_points as f32, picture.height_points as f32);
        let image = document.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => i64::from(picture.width),
                "Height" => i64::from(picture.height),
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
                "Filter" => "FlateDecode",
                "DecodeParms" => dictionary! {
                    "Predictor" => 15,
                    "Colors" => 3,
                    "BitsPerComponent" => 8,
                    "Columns" => i64::from(picture.width),
                },
            },
            picture.rows,
        ));
        let content = Content {
            operations: vec![
                Operation::new("q", vec![]),
                Operation::new(
                    "cm",
                    vec![w.into(), 0.into(), 0.into(), h.into(), 0.into(), 0.into()],
                ),
                Operation::new("Do", vec!["Page".into()]),
                Operation::new("Q", vec![]),
            ],
        };
        let content = document.add_object(Stream::new(
            dictionary! {},
            content.encode().map_err(|e| e.to_string())?,
        ));
        let page = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), w.into(), h.into()],
            "Contents" => content,
            "Resources" => dictionary! { "XObject" => dictionary! { "Page" => image } },
        });
        kids.push(page.into());
    }
    let count = kids.len() as i64;
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
    );
    let catalog = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    document.trailer.set("Root", catalog);
    let mut out = Vec::new();
    document.save_to(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// A PDF from the editor's pages, in order.
pub fn pdf_from_pages(pages: Vec<PdfPage>) -> Result<Vec<u8>, String> {
    if pages.is_empty() || pages.len() > MAX_PAGES {
        return Err(format!("A notebook has between 1 and {MAX_PAGES} pages"));
    }
    build(pages.into_iter().map(picture).collect::<Result<_, _>>()?)
}

#[tauri::command]
pub async fn canvas_export_pdf(
    app: tauri::AppHandle,
    title: String,
    pages: Vec<PdfPage>,
) -> Result<Option<String>, String> {
    crate::runtime::blocking::run(move || {
        let bytes = pdf_from_pages(pages)?;
        let name = crate::library::paths::safe_filename(title.trim().trim_end_matches(".pdf"));
        let name = if name.trim_matches('_').is_empty() {
            "Notebook".to_string()
        } else {
            name
        };
        let picked = app
            .dialog()
            .file()
            .set_title("Export notebook as PDF")
            .set_file_name(format!("{name}.pdf"))
            .add_filter("PDF document", &["pdf"])
            .blocking_save_file();
        let Some(picked) = picked else {
            return Ok(None);
        };
        let picked = picked.into_path().map_err(|e| e.to_string())?;
        let path = super::export::write_chosen(&picked, "pdf", &bytes)?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::pdf_render::{parse, render_rgba};
    use image::{Rgba, RgbaImage};

    /// An RGBA PNG (as a canvas produces) filled with `colour`, as base64.
    fn png(width: u32, height: u32, colour: [u8; 4]) -> String {
        let image = RgbaImage::from_pixel(width, height, Rgba(colour));
        let mut out = Vec::new();
        PngEncoder::new(&mut out)
            .write_image(image.as_raw(), width, height, ExtendedColorType::Rgba8)
            .unwrap();
        base64::engine::general_purpose::STANDARD.encode(out)
    }
    fn page(width: f64, height: f64, png: String) -> PdfPage {
        PdfPage { width, height, png }
    }

    #[test]
    fn each_page_keeps_its_size_and_its_picture() {
        let pdf = pdf_from_pages(vec![
            page(595.0, 842.0, png(60, 84, [200, 30, 30, 255])),
            page(842.0, 595.0, png(84, 60, [30, 30, 200, 255])),
        ])
        .unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        let document = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(document.get_pages().len(), 2);

        let parsed = parse(pdf).unwrap();
        let pages = parsed.pages();
        assert_eq!(pages.len(), 2);
        let (w0, h0) = pages[0].render_dimensions();
        let (w1, h1) = pages[1].render_dimensions();
        assert_eq!(
            (w0.round(), h0.round(), w1.round(), h1.round()),
            (595.0, 842.0, 842.0, 595.0)
        );
        let centre = |rgba: &[u8], width: usize, height: usize| {
            let at = ((height / 2) * width + width / 2) * 4;
            [rgba[at], rgba[at + 1], rgba[at + 2]]
        };
        let first = render_rgba(&pages[0], 60, 84).unwrap();
        let second = render_rgba(&pages[1], 84, 60).unwrap();
        let red = centre(&first, 60, 84);
        assert!(
            red[0] > 180 && red[1] < 60 && red[2] < 60,
            "page 1 is red: {red:?}"
        );
        let blue = centre(&second, 84, 60);
        assert!(blue[2] > 180 && blue[0] < 60, "page 2 is blue: {blue:?}");
    }

    #[test]
    fn a_picture_with_detail_survives_the_trip() {
        // A checkerboard: any wrong predictor or row length shows as a shifted or smeared image.
        let (width, height) = (64u32, 48u32);
        let mut image = RgbaImage::new(width, height);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = if (x / 8 + y / 8) % 2 == 0 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            };
        }
        let mut out = Vec::new();
        PngEncoder::new(&mut out)
            .write_image(image.as_raw(), width, height, ExtendedColorType::Rgba8)
            .unwrap();
        let encoded = base64::engine::general_purpose::STANDARD.encode(out);
        let pdf = pdf_from_pages(vec![page(64.0, 48.0, encoded)]).unwrap();
        let parsed = parse(pdf).unwrap();
        let rgba = render_rgba(&parsed.pages()[0], width, height).unwrap();
        let dark = |x: usize, y: usize| rgba[(y * width as usize + x) * 4] < 128;
        for (x, y) in [(4, 4), (12, 4), (4, 12), (60, 44), (20, 28)] {
            let expected = (x / 8 + y / 8) % 2 == 0;
            assert_eq!(dark(x, y), expected, "pixel ({x}, {y})");
        }
    }

    #[test]
    fn refuses_pages_it_cannot_embed() {
        let ok = png(4, 4, [255, 255, 255, 255]);
        assert!(pdf_from_pages(vec![]).is_err());
        assert!(pdf_from_pages(vec![page(0.0, 842.0, ok.clone())]).is_err());
        assert!(pdf_from_pages(vec![page(595.0, f64::NAN, ok.clone())]).is_err());
        assert!(pdf_from_pages(vec![page(595.0, 842.0, "***".into())]).is_err());
        let not_png = base64::engine::general_purpose::STANDARD.encode(b"GIF89a....");
        assert!(pdf_from_pages(vec![page(595.0, 842.0, not_png)]).is_err());
    }

    #[test]
    fn the_export_writer_takes_a_pdf_and_nothing_else_under_that_name() {
        let root = crate::test_support::Scratch::new("canvas-pdf");
        let pdf =
            pdf_from_pages(vec![page(100.0, 100.0, png(4, 4, [255, 255, 255, 255]))]).unwrap();
        let path = super::super::export::write_chosen(&root.join("Work.pdf"), "pdf", &pdf).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), pdf);
        assert!(
            super::super::export::write_chosen(&root.join("Bad.pdf"), "pdf", b"not a pdf").is_err()
        );
        assert!(super::super::export::write_chosen(&root.join("Work.png"), "pdf", &pdf).is_err());
    }
}
