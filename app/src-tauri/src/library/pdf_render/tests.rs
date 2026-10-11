use std::sync::Arc;

use super::*;
use crate::test_support::{write_pdf_sized, Scratch};

#[test]
fn a_panic_inside_hayro_fails_only_its_call() {
    assert_eq!(guarded(|| -> u8 { panic!("malformed") }), Err(Panicked));
    assert_eq!(guarded(|| 7), Ok(7));
}

#[tokio::test]
async fn a_render_thread_has_room_to_recurse_and_survives_a_panic() {
    // ~16 MB of stack: past a default thread's, inside a render thread's.
    fn deep(depth: u32) -> u64 {
        let frame = std::hint::black_box([depth as u8; 1024]);
        if depth == 0 {
            frame[0] as u64
        } else {
            deep(depth - 1) + frame[1] as u64
        }
    }
    assert!(on_render_thread(|| Ok(deep(16_000))).await.is_ok());
    assert_eq!(
        on_render_thread(|| -> Result<(), String> { panic!("malformed") }).await,
        Err("render-failed".to_string())
    );
    assert_eq!(on_render_thread(|| Ok(7)).await, Ok(7));
}

#[test]
fn scale_truncates_to_the_requested_pixels() {
    for side in [612.0f32, 792.0, 595.28, 841.89, 1.0, 13.7] {
        for pixels in [1u32, 13, 97, 300, 1001, 1600, 4096, 8192] {
            let scale = scale_to(side, pixels);
            assert_eq!((side * scale) as u16 as u32, pixels, "{side} -> {pixels}");
        }
    }
}

#[test]
fn open_reports_each_failure_as_its_own_error() {
    let scratch = Scratch::new("pdf-render-open");
    let good = scratch.join("good.pdf");
    write_pdf_sized(&good, 3, 612, 792);
    assert_eq!(page_count(&good), Ok(3));

    let junk = scratch.join("junk.pdf");
    std::fs::write(&junk, b"not a pdf at all").unwrap();
    assert_eq!(page_count(&junk), Err(OpenError::Invalid));

    let empty = scratch.join("empty.pdf");
    write_pdf_sized(&empty, 0, 612, 792);
    assert_eq!(page_count(&empty), Ok(0));

    assert!(matches!(
        page_count(&scratch.join("missing.pdf")),
        Err(OpenError::Read(_))
    ));
}

#[test]
fn the_cache_reopens_a_rewritten_file() {
    let scratch = Scratch::new("pdf-render-cache");
    let path = scratch.join("a.pdf");
    write_pdf_sized(&path, 2, 612, 792);
    let first = open_cached(&path).unwrap();
    assert!(Arc::ptr_eq(&first, &open_cached(&path).unwrap()));

    write_pdf_sized(&path, 5, 612, 792);
    let rewritten = open_cached(&path).unwrap();
    assert_eq!(rewritten.pages().len(), 5);

    forget(&path);
    assert!(!Arc::ptr_eq(&rewritten, &open_cached(&path).unwrap()));
}

#[test]
fn a_render_is_refused_outside_hayros_sizes() {
    let scratch = Scratch::new("pdf-render-size");
    let path = scratch.join("a.pdf");
    write_pdf_sized(&path, 1, 612, 792);
    let pdf = open(&path).unwrap();
    let page = &pdf.pages()[0];
    assert_eq!(render_rgba(page, 0, 10), Err(RenderError::BadSize));
    assert_eq!(
        render_rgba(page, MAX_RENDER_SIDE + 1, 10),
        Err(RenderError::BadSize)
    );
    let rgba = render_rgba(page, 97, 13).unwrap();
    assert_eq!(rgba.len(), 97 * 13 * 4);
}
