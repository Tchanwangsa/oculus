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
