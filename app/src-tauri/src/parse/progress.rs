//! Where a running parse is.

use serde::Serialize;

/// Where a running parse is. Only the cloud uploads; the local engine and a
/// cloud file whose bytes are sent both report `Processing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Its batch was submitted; another file of the batch is uploading first.
    UploadWait,
    Uploading,
    /// Uploaded (or local): the engine is extracting.
    Processing,
}

impl Phase {
    /// The `phase` on the `parse-status` wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::UploadWait => "upload_wait",
            Phase::Uploading => "uploading",
            Phase::Processing => "processing",
        }
    }
}

/// Reported while a parse runs. `total_pages` is zero until the backend knows;
/// the byte counts are zero outside the upload phases.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Progress {
    pub pages_done: u32,
    pub total_pages: u32,
    pub backend: &'static str,
    pub phase: Phase,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

impl Progress {
    /// The engine is extracting: pages only.
    pub fn processing(pages_done: u32, total_pages: u32, backend: &'static str) -> Self {
        Self {
            pages_done,
            total_pages,
            backend,
            phase: Phase::Processing,
            bytes_done: 0,
            bytes_total: 0,
        }
    }
}
