//! Scaffolding the unit tests share: scratch directories, small real PDFs, and
//! a fake HTTP server on loopback.

mod http;
mod keyd;
mod pdf;
mod scratch;

pub use http::*;
pub use keyd::FakeKeyd;
pub use pdf::{write_pdf, write_pdf_sized};
pub use scratch::Scratch;

/// Write `body` at `path`, creating its folders: a parse or embed record
/// lives under the subject's `.oculus/derived/`, which nothing made yet.
pub fn write_creating(path: impl AsRef<std::path::Path>, body: impl AsRef<[u8]>) {
    let path = path.as_ref();
    std::fs::create_dir_all(path.parent().expect("a file has a folder")).unwrap();
    std::fs::write(path, body).unwrap();
}
