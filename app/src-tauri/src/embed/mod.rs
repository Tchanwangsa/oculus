//! The seam every embedder plugs into: Voyage implements it in-process, and a
//! local embedder would implement the same trait. Nothing here may assume the
//! cloud; an API root, a key or a rate limit arrives as configuration.
//!
//! The seam owns the contract — the on-disk record, the vector space every
//! row in `pages` must belong to, the encoding of the blob column, and an
//! error vocabulary that mirrors `parse/error.rs` so one failure UI reads both.
//!
//! What gets embedded is the rendered page image, never extracted text (see
//! `docs/retrieval.md`), which is why `embed` takes a PDF and a
//! page count rather than a string.

mod config;
mod embedder;
mod error;
mod health;
mod output;
mod progress;
mod record;
#[cfg(test)]
mod tests;

pub mod commands;
pub mod estimate;
pub mod events;
pub mod raster;
pub mod voyage;

pub use config::{
    backend, embed_config, CredentialSource, EmbedConfig, Engine, CLOUD_BASE_URL, LOCAL_BASE_URL,
};
pub use embedder::Embedder;
pub use error::EmbedError;
pub use health::{preflight, Health};
pub use output::{EmbedOutput, EmbedPage};
pub use progress::{Limiter, Progress, Wait};
#[cfg(test)]
pub use record::decode_vector;
pub use record::{emb_path, encode_vector, is_embedded, pack_vector, read_record, unpack_vector};

/// The stored vector width: 1024-byte blobs of 512 x f16 in `pages.embedding`,
/// and a Matryoshka width the cloud model honours via `output_dimension`.
/// Changing it means migrating every row.
pub const EMBED_DIM: usize = 512;

/// The model that defines the space, stamped into every record. It names the
/// space, not the vendor: a local backend producing the same vectors would
/// claim this same id. A mismatch is a different geometry (see `Health::check`).
pub const EMBED_MODEL: &str = "voyage-multimodal-3.5";

/// How a vector is written down: little-endian float16, base64 in the record,
/// raw bytes in the blob column.
pub const EMBED_DTYPE: &str = "float16";

/// Names the query side of the asymmetry (`input_type: "query"` against
/// `"document"`). Compared, never sent. Documents and queries must use
/// opposite sides; using one for both is undetectable and just ranks worse.
pub const QUERY_INSTRUCTION: &str = "input_type:query";
