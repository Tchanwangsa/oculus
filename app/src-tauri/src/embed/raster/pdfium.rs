//! Binding to libpdfium: found once per process, then shared.

use std::sync::OnceLock;

use pdfium_render::prelude::Pdfium;

use super::{library, RasterError};

/// `Pdfium::new` may only be called once per process, so the binding is a
/// singleton (`Send + Sync` under the `thread_safe` feature).
static PDFIUM: OnceLock<Result<Pdfium, String>> = OnceLock::new();

pub(super) fn pdfium() -> Result<&'static Pdfium, RasterError> {
    match PDFIUM.get_or_init(bind) {
        Ok(pdfium) => Ok(pdfium),
        Err(message) => Err(RasterError::Library(message.clone())),
    }
}

fn bind() -> Result<Pdfium, String> {
    let mut tried = Vec::new();
    for candidate in library::candidates() {
        if !candidate.exists() {
            continue;
        }
        match Pdfium::bind_to_library(&candidate) {
            Ok(bindings) => return Ok(Pdfium::new(bindings)),
            Err(error) => tried.push(format!("{} ({error})", candidate.display())),
        }
    }
    library::bind_system(tried)
}
