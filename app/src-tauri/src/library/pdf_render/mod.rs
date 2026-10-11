//! Every PDF raster in the app comes from here, on the pure-Rust renderer
//! hayro: the viewer's pages (`library/pdf_view/`, `docs/viewers.md`) and the
//! embedder's page images (`embed/raster/`, `docs/retrieval.md`).
//!
//! * Opening a document, uncached ([`open`]) or through the viewer's small
//!   LRU keyed by path and modification stamp ([`open_cached`]).
//! * A page at exactly the pixel size asked for, opaque on white
//!   ([`render_rgba`]), on a thread with room for hayro's recursion.
//! * Every hayro call runs under `catch_unwind` ([`guarded`]): a malformed PDF
//!   fails its own call, never the process.
//! * Every render first reserves memory from one process-wide budget
//!   ([`budget`]), which also caps how many run at once.

pub mod budget;
mod documents;
mod render;
#[cfg(test)]
mod tests;

pub use documents::{forget, guarded, open, open_cached, page_count, OpenError, Panicked};
#[cfg(test)]
pub(crate) use render::scale_to;
pub(crate) use render::{on_render_thread, RENDER_STACK};
pub use render::{render_rgba, RenderError, MAX_RENDER_SIDE};
