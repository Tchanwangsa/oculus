//! Writes the row off the async runtime, one thread per write. Writes are
//! numbered when queued (under the state's lock, so in order); one that runs
//! after a newer one has landed is skipped.

use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use super::{encode, LastPlayed, SETTING};

#[derive(Debug, Default)]
pub(in crate::music) struct Saver {
    queued: u64,
    written: Arc<Mutex<u64>>,
}

impl Saver {
    pub(in crate::music) fn save(&mut self, row: &LastPlayed) -> JoinHandle<()> {
        self.queued += 1;
        let number = self.queued;
        let text = encode(row);
        let written = self.written.clone();
        std::thread::spawn(move || {
            let mut landed = written.lock().unwrap_or_else(|e| e.into_inner());
            if *landed > number {
                return;
            }
            if let Err(why) = crate::db::store::set_setting_blocking(SETTING, &text) {
                elog!("[music] could not remember what was playing: {why}");
            }
            *landed = number;
        })
    }
}
