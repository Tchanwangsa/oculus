use super::*;
use crate::test_support::{write_pdf_sized, Scratch};

#[tokio::test]
async fn selected_pdf_pages_are_independent_pngs_and_handles_close() {
    let root = Scratch::new("canvas-import");
    let path = root.join("pages.pdf");
    write_pdf_sized(&path, 2, 612, 792);
    let opened = register(
        "canvas-test".into(),
        "pages.pdf".into(),
        load(&path).unwrap(),
    )
    .unwrap();
    assert_eq!(opened.pages.len(), 2);
    assert_eq!(opened.pages[0].width, 612.0);
    assert!(document("another-window", &opened.id).is_err());
    close("another-window", &opened.id);
    let pdf = document("canvas-test", &opened.id).unwrap();
    let png = pdf_render::on_render_thread(move || {
        assert!(raster(&pdf, 0, 120, 160).is_err());
        assert!(raster(&pdf, 3, 120, 160).is_err());
        assert!(raster(&pdf, 1, 2401, 160).is_err());
        assert!(raster(&pdf, 1, 2400, 2400).is_err());
        assert!(raster(&pdf, 1, 0, 160).is_err());
        raster(&pdf, 2, 120, 160)
    })
    .await
    .unwrap();
    let image = image::load_from_memory_with_format(&png, image::ImageFormat::Png).unwrap();
    assert_eq!((image.width(), image.height()), (120, 160));
    close("canvas-test", &opened.id);
    assert!(document("canvas-test", &opened.id).is_err());
    close("canvas-test", &opened.id);
}

#[test]
fn library_sources_require_canonical_containment() {
    let root = Scratch::new("canvas-import-scope");
    let courses = root.join("courses");
    std::fs::create_dir_all(&courses).unwrap();
    let pdf = courses.join("lesson.pdf");
    write_pdf_sized(&pdf, 1, 612, 792);
    assert_eq!(
        library_path(&root, "courses/lesson.pdf").unwrap(),
        pdf.canonicalize().unwrap()
    );
    for path in [
        "../private.pdf",
        "/private.pdf",
        "courses/../private.pdf",
        "private.pdf",
    ] {
        assert!(library_path(&root, path).is_err(), "{path}");
    }
    #[cfg(unix)]
    {
        let outside = Scratch::new("canvas-import-outside");
        let private = outside.join("private.pdf");
        write_pdf_sized(&private, 1, 612, 792);
        std::os::unix::fs::symlink(&private, courses.join("alias.pdf")).unwrap();
        assert!(library_path(&root, "courses/alias.pdf").is_err());
    }
}

#[test]
fn invalid_and_empty_pdfs_do_not_open_imports() {
    let root = Scratch::new("canvas-import-invalid");
    let junk = root.join("invalid.pdf");
    std::fs::write(&junk, "not a PDF").unwrap();
    assert!(load(&junk).is_err());
    let empty = root.join("empty.pdf");
    write_pdf_sized(&empty, 0, 612, 792);
    assert!(register(
        "canvas-empty".into(),
        "empty.pdf".into(),
        load(&empty).unwrap()
    )
    .is_err());
}
