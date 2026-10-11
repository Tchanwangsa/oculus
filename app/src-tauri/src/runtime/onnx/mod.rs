//! ONNX Runtime pieces the app's local models share: a CPU-only session and
//! model packs downloaded at a pinned size and sha256. Studio's narration
//! aligner (`studio::timing`) is the first user. ORT is linked statically
//! (`ort` in Cargo.toml), so there is no library to find at run time; models
//! are downloaded on request, never bundled.

mod pack;
mod session;

pub use pack::{cancel, delete, download, downloading, is_present, Pack, PackFile, CANCELLED};
pub use session::{intra_threads, session};
