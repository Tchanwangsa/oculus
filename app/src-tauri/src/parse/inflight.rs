//! One parse per PDF at a time.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex};

/// PDFs with a parse running in this process. A sync, the sweep and "Parse
/// now" can ask for the same file at once; a second caller waits here for the
/// first, then finds the record and skips. No timeout: a parse takes minutes.
pub struct InFlight {
    running: Mutex<BTreeSet<PathBuf>>,
    freed: Condvar,
}

/// Held for one parse; dropping it (unwinding included) frees the PDF.
pub struct InFlightClaim<'a> {
    owner: &'a InFlight,
    pdf: PathBuf,
}

impl InFlight {
    pub const fn new() -> Self {
        Self {
            running: Mutex::new(BTreeSet::new()),
            freed: Condvar::new(),
        }
    }

    /// The process-wide set every parse goes through.
    pub fn shared() -> &'static InFlight {
        static SHARED: InFlight = InFlight::new();
        &SHARED
    }

    /// Block until no other parse holds `pdf`, then hold it.
    pub fn claim(&self, pdf: &Path) -> InFlightClaim<'_> {
        let mut running = self.running.lock().unwrap_or_else(|p| p.into_inner());
        while running.contains(pdf) {
            running = self.freed.wait(running).unwrap_or_else(|p| p.into_inner());
        }
        running.insert(pdf.to_path_buf());
        InFlightClaim {
            owner: self,
            pdf: pdf.to_path_buf(),
        }
    }
}

impl Drop for InFlightClaim<'_> {
    fn drop(&mut self) {
        self.owner
            .running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.pdf);
        self.owner.freed.notify_all();
    }
}
