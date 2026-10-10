//! How pages are packed into requests, and how many requests run at once.
//!
//! Voyage batches pages, not documents, and never across documents: TPM binds
//! at every tier, so cross-document packing would buy nothing and cost a
//! failure spanning two files.
//!
//! * Both per-request ceilings apply (inputs and tokens), with tokens computed
//!   from each page's real pixels, never a page count.
//! * Progress is summed from finished requests. A request held back by a
//!   rate limit reports the wait, so a paced run is not mistaken for a hang.
//! * Nothing partial escapes: a document that did not embed every page is an
//!   error and no record is written.

mod limits;
mod plan;
mod request;
mod run;
#[cfg(test)]
mod tests;

pub use limits::Limits;
pub use plan::{
    billed_pixels, plan, raw_pixels, refuse_oversized, tokens_for, BILLED_PIXEL_CAP,
    MAX_BYTES_PER_IMAGE, MAX_INPUTS_PER_REQUEST, MAX_PIXELS_PER_IMAGE, MAX_TOKENS_PER_INPUT,
    MAX_TOKENS_PER_REQUEST, PIXELS_PER_TOKEN,
};
pub use request::RequestRun;
pub use run::run_document;
