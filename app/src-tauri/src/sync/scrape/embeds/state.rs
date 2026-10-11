//! The embed commands' shared state: the one Bearer every link uses, and a
//! cancel flag per in-flight download.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crate::sources::echo360::embed::Bearer;

/// One cancel flag per in-flight embed download, keyed by link id;
/// `copy_to_file` polls it between chunks.
#[derive(Clone, Default)]
pub struct EmbedCancels(pub Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>);

impl EmbedCancels {
    /// A new flag for `link_id`, or an error when one is already running. The
    /// returned guard removes it on every exit, so a late cancel cannot
    /// poison a retry.
    pub(super) fn register(&self, link_id: &str) -> Result<CancelGuard, String> {
        let flag = Arc::new(AtomicBool::new(false));
        let mut running = self.0.lock().unwrap();
        if running.contains_key(link_id) {
            return Err("That video is already downloading.".to_string());
        }
        running.insert(link_id.to_string(), Arc::clone(&flag));
        Ok(CancelGuard {
            cancels: self.clone(),
            link_id: link_id.to_string(),
            flag,
        })
    }
}

pub(super) struct CancelGuard {
    cancels: EmbedCancels,
    link_id: String,
    pub flag: Arc<AtomicBool>,
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        self.cancels.0.lock().unwrap().remove(&self.link_id);
    }
}

/// The Bearer the embed player API accepts. One works for every link, so it
/// is kept for the app's run and replaced from the next launch once it is
/// within five minutes of expiry or Echo360 turns it down.
#[derive(Clone, Default)]
pub struct EmbedBearer(pub Arc<Mutex<Option<Bearer>>>);

impl EmbedBearer {
    pub(super) fn usable(&self) -> Option<Bearer> {
        let now = crate::runtime::clock::now_secs();
        self.0
            .lock()
            .unwrap()
            .as_ref()
            .filter(|b| b.usable_at(now))
            .cloned()
    }

    pub(super) fn set(&self, bearer: Bearer) {
        *self.0.lock().unwrap() = Some(bearer);
    }

    pub(super) fn clear(&self) {
        *self.0.lock().unwrap() = None;
    }
}
