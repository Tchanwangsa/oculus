use std::path::PathBuf;

use super::dpi::*;
use super::workers::*;
use super::*;
use crate::library::pdf_render::budget::{self, Budget};
use crate::test_support::Scratch;

/// A real PDF built in memory, one line of text per page; `rotate` turns
/// every page.
fn synthetic_pdf(pages: usize, rotate: i64) -> (Scratch, PathBuf) {
    use lopdf::{dictionary, Document, Object};

    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let font_id = document.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
    });
    let resources_id = document.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });

    let mut kids = Vec::new();
    for index in 0..pages {
        let content = lopdf::content::Content {
            operations: vec![
                lopdf::content::Operation::new("BT", vec![]),
                lopdf::content::Operation::new("Tf", vec!["F1".into(), 24.into()]),
                lopdf::content::Operation::new("Td", vec![40.into(), 700.into()]),
                lopdf::content::Operation::new(
                    "Tj",
                    vec![Object::string_literal(format!("page {}", index + 1))],
                ),
                lopdf::content::Operation::new("ET", vec![]),
            ],
        };
        let content_id = document.add_object(lopdf::Stream::new(
            dictionary! {},
            content.encode().unwrap(),
        ));
        let page_id = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            // US Letter.
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Rotate" => rotate,
        });
        kids.push(page_id.into());
    }

    let count = kids.len() as i64;
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => count,
            "Resources" => resources_id,
        }),
    );
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog", "Pages" => pages_id,
    });
    document.trailer.set("Root", catalog_id);

    let dir = Scratch::new("raster-synthetic");
    let path = dir.join("synthetic.pdf");
    document.save(&path).unwrap();
    (dir, path)
}

#[test]
fn render_dpi_matches_the_stored_vectors() {
    assert_eq!(RENDER_DPI, 200);
}

#[test]
fn pixel_maths_matches_mupdf_rounding() {
    // Landscape A4 as PyMuPDF renders it.
    assert_eq!(pixels_for(842.0, 200), 2339);
    assert_eq!(pixels_for(595.0, 200), 1653);
    assert_eq!(pixels_for(612.0, 200), 1700);
    assert_eq!(pixels_for(792.0, 200), 2200);
    // An exactly-integral edge gains no blank column.
    assert_eq!(pixels_for(360.0, 72), 360);
    assert_eq!(pixels_for(72.0, 200), 200);
    // Degenerate boxes still produce a renderable bitmap.
    assert_eq!(pixels_for(0.0, 200), 1);
}

#[test]
fn an_oversized_page_is_rendered_smaller_rather_than_refused() {
    // Quoted, not imported: raster does not know which backend asks.
    const MAX: u64 = 16_000_000;

    // A0 landscape.
    let raw = u64::from(pixels_for(3370.0, RENDER_DPI)) * u64::from(pixels_for(2384.0, RENDER_DPI));
    assert!(raw > MAX, "fixture is not actually oversized: {raw}");

    let dpi = dpi_for_page(3370.0, 2384.0, RENDER_DPI, Some(MAX));
    assert!(dpi < RENDER_DPI);
    let clamped = u64::from(pixels_for(3370.0, dpi)) * u64::from(pixels_for(2384.0, dpi));
    assert!(
        clamped <= MAX,
        "still over the ceiling: {clamped} at {dpi} DPI"
    );
    // Only just under: one DPI more must not fit.
    let over = u64::from(pixels_for(3370.0, dpi + 1)) * u64::from(pixels_for(2384.0, dpi + 1));
    assert!(over > MAX, "clamped further than it had to: {dpi} DPI");
    assert!(clamped > 2_000_000 * 4);
}

#[test]
fn an_ordinary_page_is_untouched_by_the_clamp() {
    assert_eq!(
        dpi_for_page(842.0, 595.0, RENDER_DPI, Some(16_000_000)),
        RENDER_DPI
    );
    assert_eq!(
        dpi_for_page(612.0, 792.0, RENDER_DPI, Some(16_000_000)),
        RENDER_DPI
    );
    assert_eq!(dpi_for_page(3370.0, 2384.0, RENDER_DPI, None), RENDER_DPI);
}

#[test]
fn renders_every_page_once_in_order() {
    let (_dir, pdf) = synthetic_pdf(9, 0);

    let mut seen = Vec::new();
    let count = render_pages(&pdf, None, |page| {
        assert!(page.png.starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG");
        seen.push((page.page_no, page.width, page.height));
        Ok(())
    })
    .unwrap();

    assert_eq!(count, 9);
    let expected: Vec<_> = (1..=9).map(|page| (page, 1700, 2200)).collect();
    assert_eq!(seen, expected);
}

#[test]
fn a_rotated_page_renders_with_its_sides_swapped() {
    let (_dir, pdf) = synthetic_pdf(1, 90);
    let mut seen = Vec::new();
    render_pages(&pdf, None, |page| {
        seen.push((page.width, page.height));
        Ok(())
    })
    .unwrap();
    assert_eq!(seen, [(2200, 1700)]);
    assert_eq!(page_sizes(&pdf).unwrap(), seen);
}

/// One worker or many, the pages and their bytes are the same.
#[test]
fn the_worker_count_changes_nothing_but_speed() {
    let (_dir, pdf) = synthetic_pdf(7, 0);
    let document = open(&pdf).unwrap();
    let collect = |budget: &Budget| {
        let mut pages = Vec::new();
        render_document(&document, budget, None, |page| {
            pages.push(page);
            Ok(())
        })
        .unwrap();
        pages
    };
    let serial = collect(&Budget::new(budget::BUDGET_BYTES, 1));
    let parallel = collect(&Budget::new(budget::BUDGET_BYTES, 4));
    assert_eq!(serial.len(), 7);
    for (one, many) in serial.iter().zip(&parallel) {
        assert_eq!(one.page_no, many.page_no);
        assert_eq!(one.png, many.png, "page {}", one.page_no);
    }
}

#[test]
fn the_png_is_rgb_on_white_with_the_text_painted() {
    let (_dir, pdf) = synthetic_pdf(1, 0);
    let mut png = Vec::new();
    render_pages(&pdf, None, |page| {
        png = page.png;
        Ok(())
    })
    .unwrap();
    let image = image::load_from_memory(&png).unwrap();
    assert_eq!(image.color(), image::ColorType::Rgb8);
    let rgb = image.to_rgb8();
    assert_eq!(rgb.get_pixel(1699, 2199).0, [255, 255, 255]);
    let dark = rgb.pixels().filter(|px| px.0[0] < 128).count();
    assert!(dark > 100, "the text paints: {dark} dark pixels");
}

#[test]
fn rgba_becomes_rgb_in_place() {
    let rgba = vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255];
    assert_eq!(into_rgb(rgba), [1, 2, 3, 4, 5, 6, 7, 8, 9]);
    assert!(into_rgb(Vec::new()).is_empty());
}

/// The estimator's sizes are the renderer's sizes.
#[test]
fn measured_sizes_are_the_sizes_that_get_rendered() {
    let (_dir, pdf) = synthetic_pdf(3, 0);

    let measured = page_sizes(&pdf).unwrap();
    let mut rendered = Vec::new();
    render_pages(&pdf, None, |page| {
        rendered.push((page.width, page.height));
        Ok(())
    })
    .unwrap();

    assert_eq!(measured, rendered);
}

#[test]
fn page_count_is_the_parse_records_count() {
    let (_dir, pdf) = synthetic_pdf(4, 0);
    assert_eq!(page_count(&pdf).unwrap(), 4);
    assert_eq!(crate::library::pdf_render::page_count(&pdf), Ok(4));
}

#[test]
fn a_caller_can_stop_early() {
    let (_dir, pdf) = synthetic_pdf(12, 0);
    let mut rendered = 0;
    let result = render_pages(&pdf, None, |_| {
        rendered += 1;
        if rendered == 2 {
            return Err(RasterError::Empty);
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(rendered, 2, "pages were delivered past the caller's error");
}

#[test]
fn a_malformed_file_is_an_error_not_a_panic() {
    let dir = Scratch::new("raster-not-a");
    let pdf = dir.join("not-a.pdf");
    std::fs::write(&pdf, b"%PDF-1.7\nthis is not a pdf at all\n").unwrap();
    assert!(matches!(
        render_pages(&pdf, None, |_| Ok(())),
        Err(RasterError::Unreadable(_) | RasterError::Empty)
    ));
}

#[test]
fn a_missing_file_is_an_error_not_a_panic() {
    let missing = Path::new("/nonexistent/oculus/raster/missing.pdf");
    assert!(matches!(
        render_pages(missing, None, |_| Ok(())),
        Err(RasterError::Unreadable(_))
    ));
    assert!(page_count(missing).is_err());
    assert!(page_sizes(missing).is_err());
}

#[test]
fn an_empty_file_is_an_error_not_a_panic() {
    let dir = Scratch::new("raster-empty");
    let pdf = dir.join("empty.pdf");
    std::fs::write(&pdf, b"").unwrap();
    assert!(render_pages(&pdf, None, |_| Ok(())).is_err());

    let pageless = dir.join("pageless.pdf");
    crate::test_support::write_pdf(&pageless, 0);
    assert!(matches!(
        render_pages(&pageless, None, |_| Ok(())),
        Err(RasterError::Empty)
    ));
}

/// Off unless `OCULUS_RASTER_PDF` points at a real library PDF.
#[test]
fn renders_a_real_library_pdf() {
    let Some(path) = std::env::var_os("OCULUS_RASTER_PDF") else {
        eprintln!("skipping: set OCULUS_RASTER_PDF to a real library PDF");
        return;
    };
    let path = PathBuf::from(path);
    let mut first = None;
    let count = render_pages(&path, None, |page| {
        if page.page_no == 1 {
            first = Some(page);
        }
        Ok(())
    })
    .unwrap();

    assert!(count > 0);
    let first = first.expect("no page 1");
    assert!(first.png.len() > 1024, "page 1 PNG is suspiciously small");
    eprintln!(
        "{count} pages; page 1: {} x {} ({} bytes)",
        first.width,
        first.height,
        first.png.len()
    );

    // `OCULUS_RASTER_OUT` keeps page 1 for a visual check.
    if let Some(out) = std::env::var_os("OCULUS_RASTER_OUT") {
        std::fs::write(&out, &first.png).unwrap();
        eprintln!("wrote {}", PathBuf::from(out).display());
    }
}
