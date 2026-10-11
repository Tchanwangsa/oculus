//! Small real PDFs.

use std::path::Path;

/// A real PDF of `pages` blank 144pt-square pages.
pub fn write_pdf(path: &Path, pages: usize) {
    write_pdf_sized(path, pages, 144, 144);
}

pub fn write_pdf_sized(path: &Path, pages: usize, width: i64, height: i64) {
    use lopdf::{dictionary, Document, Object};
    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let kids: Vec<Object> = (0..pages)
        .map(|_| {
            document
                .add_object(dictionary! {
                    "Type" => "Page",
                    "Parent" => pages_id,
                    "MediaBox" => vec![0.into(), 0.into(), width.into(), height.into()],
                })
                .into()
        })
        .collect();
    let count = kids.len() as i64;
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
    );
    let catalog = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    document.trailer.set("Root", catalog);
    document.save(path).unwrap();
}
