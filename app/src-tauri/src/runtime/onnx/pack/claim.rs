//! Which packs are downloading, each with its cancel flag.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

static DOWNLOADS: Mutex<Option<HashMap<String, Arc<AtomicBool>>>> = Mutex::new(None);

/// One download's place in [`DOWNLOADS`], released however it ends so a late
/// cancel cannot poison a retry.
pub(super) struct Claim(String, Arc<AtomicBool>);

impl Claim {
    pub(super) fn take(id: &str) -> Result<Self, String> {
        let mut held = DOWNLOADS.lock().unwrap();
        let held = held.get_or_insert_with(HashMap::new);
        if held.contains_key(id) {
            return Err("that model is already downloading".into());
        }
        let flag = Arc::new(AtomicBool::new(false));
        held.insert(id.to_string(), Arc::clone(&flag));
        Ok(Self(id.to_string(), flag))
    }

    pub(super) fn flag(&self) -> &AtomicBool {
        &self.1
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        if let Some(held) = DOWNLOADS.lock().unwrap().as_mut() {
            held.remove(&self.0);
        }
    }
}

/// Ask the pack's download to stop; false when none runs.
pub fn cancel(id: &str) -> bool {
    match DOWNLOADS.lock().unwrap().as_ref().and_then(|d| d.get(id)) {
        Some(flag) => {
            flag.store(true, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

/// A download of the pack is in flight in this process.
pub fn downloading(id: &str) -> bool {
    DOWNLOADS
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|d| d.contains_key(id))
}
