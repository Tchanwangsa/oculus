use std::path::PathBuf;

use super::dpi::*;
use super::pdfium::*;
use super::*;
use crate::test_support::Scratch;

/// A real PDF built in memory: pdfium parses it for real.
fn synthetic_pdf(pages: usize) -> (Scratch, PathBuf) {
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

/// The count `parse/` writes into the record.
fn hayro_page_count(pdf: &Path) -> u32 {
    let bytes = std::fs::read(pdf).unwrap();
    hayro_syntax::Pdf::new(bytes).unwrap().pages().len() as u32
}

/// Tests needing libpdfium skip without it (`bun run pdfium`).
fn library_present() -> bool {
    let available = pdfium().is_ok();
    if !available {
        eprintln!("skipping: libpdfium not fetched (run `bun run pdfium` in app/)");
    }
    available
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
    if !library_present() {
        return;
    }
    let (_dir, pdf) = synthetic_pdf(3);

    let mut seen = Vec::new();
    let count = render_pages(&pdf, None, |page| {
        assert!(page.png.starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG");
        seen.push((page.page_no, page.width, page.height));
        Ok(())
    })
    .unwrap();

    assert_eq!(count, 3);
    assert_eq!(
        seen,
        vec![(1, 1700, 2200), (2, 1700, 2200), (3, 1700, 2200)]
    );
}

/// The estimator's sizes are the renderer's sizes.
#[test]
fn measured_sizes_are_the_sizes_that_get_rendered() {
    if !library_present() {
        return;
    }
    let (_dir, pdf) = synthetic_pdf(3);

    let measured = page_sizes(&pdf).unwrap();
    let mut rendered = Vec::new();
    render_pages(&pdf, None, |page| {
        rendered.push((page.width, page.height));
        Ok(())
    })
    .unwrap();

    assert_eq!(measured, rendered);
}

/// The invariant [`SESSION`] exists for.
#[test]
fn two_threads_reading_the_same_document_both_get_the_truth() {
    if !library_present() {
        return;
    }
    let (_dir, pdf) = synthetic_pdf(6);
    let expected = page_sizes(&pdf).unwrap();

    let workers: Vec<_> = (0..4)
        .map(|_| {
            let path = pdf.clone();
            std::thread::spawn(move || (0..12).map(|_| page_sizes(&path)).collect::<Vec<_>>())
        })
        .collect();

    for worker in workers {
        for attempt in worker.join().unwrap() {
            assert_eq!(attempt.as_ref().map_err(|e| e.to_string()), Ok(&expected));
        }
    }
}

#[test]
fn a_missing_file_is_measurable_as_an_error_not_a_panic() {
    if !library_present() {
        return;
    }
    assert!(page_sizes(Path::new("/nope/does-not-exist.pdf")).is_err());
}

#[test]
fn page_count_agrees_with_hayro_on_a_well_formed_file() {
    if !library_present() {
        return;
    }
    let (_dir, pdf) = synthetic_pdf(4);
    assert_eq!(page_count(&pdf).unwrap(), hayro_page_count(&pdf));
}

#[test]
fn a_caller_can_stop_early() {
    if !library_present() {
        return;
    }
    let (_dir, pdf) = synthetic_pdf(5);
    let mut rendered = 0;
    let result = render_pages(&pdf, None, |_| {
        rendered += 1;
        if rendered == 2 {
            return Err(RasterError::Empty);
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(rendered, 2, "rendering continued past the caller's error");
}

#[test]
fn a_malformed_file_is_an_error_not_a_panic() {
    if !library_present() {
        return;
    }
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
    if !library_present() {
        return;
    }
    let missing = Path::new("/nonexistent/oculus/raster/missing.pdf");
    assert!(render_pages(missing, None, |_| Ok(())).is_err());
    assert!(page_count(missing).is_err());
}

#[test]
fn an_empty_file_is_an_error_not_a_panic() {
    if !library_present() {
        return;
    }
    let dir = Scratch::new("raster-empty");
    let pdf = dir.join("empty.pdf");
    std::fs::write(&pdf, b"").unwrap();
    assert!(render_pages(&pdf, None, |_| Ok(())).is_err());
}

/// Off unless `OCULUS_RASTER_PDF` points at a real library PDF.
#[test]
fn renders_a_real_library_pdf() {
    let Some(path) = std::env::var_os("OCULUS_RASTER_PDF") else {
        eprintln!("skipping: set OCULUS_RASTER_PDF to a real library PDF");
        return;
    };
    if !library_present() {
        return;
    }
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
    let hayro_count = hayro_page_count(&path);
    eprintln!("pages: pdfium {count}, hayro {hayro_count}");
    assert_eq!(count, hayro_count);

    let first = first.expect("no page 1");
    assert!(first.png.len() > 1024, "page 1 PNG is suspiciously small");
    eprintln!(
        "page 1: {} x {} ({} bytes)",
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
