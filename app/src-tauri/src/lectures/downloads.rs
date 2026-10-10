//! The cancel flags of in-flight Echo360 downloads.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// One cancel flag per in-flight download, keyed `"{media_id}:{source}"` like
/// the frontend's progress bars. `stream_to_file` checks it between chunks.
#[derive(Clone, Default)]
pub struct DownloadCancels(pub Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>);

pub(super) fn cancel_key(media_id: &str, source: u8) -> String {
    format!("{media_id}:{source}")
}
