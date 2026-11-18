//! Moving a waiting parse ahead of the others.
//!
//! The student can push a queued file to the front. Each press takes the next
//! rank, so several prioritised files keep the order they were pressed in, and
//! every file that was never pressed sorts after them in arrival order. A
//! parse already running is never interrupted: rank only decides who is picked
//! *next* — by the cloud batcher (`mineru::batch`), its upload order
//! (`client::run_batch`) and the local server's gate (`mineru::local::gate`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The lower sorts first; a file never pressed has no rank and sorts last.
pub type Rank = Option<u64>;

#[derive(Default)]
struct Ranks {
    next: u64,
    by_pdf: BTreeMap<PathBuf, u64>,
}

/// PDFs prioritised this session, keyed like `InFlight` and `Skips`. Memory
/// only: a restart re-queues work in its own order.
pub struct Priority {
    ranks: Mutex<Ranks>,
}

impl Default for Priority {
    fn default() -> Self {
        Self::new()
    }
}

impl Priority {
    pub const fn new() -> Self {
        Self {
            ranks: Mutex::new(Ranks {
                next: 0,
                by_pdf: BTreeMap::new(),
            }),
        }
    }

    pub fn shared() -> &'static Priority {
        static SHARED: Priority = Priority::new();
        &SHARED
    }

    /// Take the next rank for `pdf`, behind every file prioritised before it.
    /// Pressing again moves the file to the back of the prioritised ones.
    pub fn prioritise(&self, pdf: &Path) {
        let mut ranks = self.ranks.lock().unwrap_or_else(|p| p.into_inner());
        let rank = ranks.next;
        ranks.next += 1;
        ranks.by_pdf.insert(pdf.to_path_buf(), rank);
    }

    pub fn rank(&self, pdf: &Path) -> Rank {
        self.ranks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .by_pdf
            .get(pdf)
            .copied()
    }

    /// Forget `pdf`'s rank once its parse has ended, so a later re-parse of
    /// the same file does not jump the queue on a press that was spent.
    pub fn clear(&self, pdf: &Path) {
        self.ranks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .by_pdf
            .remove(pdf);
    }
}

/// `Rank` as a sort key: prioritised files by press order, then the rest.
pub fn sort_key(rank: Rank) -> u64 {
    rank.unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presses_rank_in_the_order_they_were_made() {
        let priority = Priority::new();
        priority.prioritise(Path::new("/a.pdf"));
        priority.prioritise(Path::new("/b.pdf"));
        assert!(priority.rank(Path::new("/a.pdf")) < priority.rank(Path::new("/b.pdf")));
    }

    #[test]
    fn a_file_never_pressed_sorts_after_every_pressed_one() {
        let priority = Priority::new();
        priority.prioritise(Path::new("/a.pdf"));
        assert_eq!(priority.rank(Path::new("/other.pdf")), None);
        assert!(sort_key(priority.rank(Path::new("/a.pdf"))) < sort_key(None));
    }

    #[test]
    fn pressing_again_moves_a_file_behind_the_others() {
        let priority = Priority::new();
        priority.prioritise(Path::new("/a.pdf"));
        priority.prioritise(Path::new("/b.pdf"));
        priority.prioritise(Path::new("/a.pdf"));
        assert!(priority.rank(Path::new("/b.pdf")) < priority.rank(Path::new("/a.pdf")));
    }

    #[test]
    fn a_cleared_rank_is_spent() {
        let priority = Priority::new();
        priority.prioritise(Path::new("/a.pdf"));
        priority.clear(Path::new("/a.pdf"));
        assert_eq!(priority.rank(Path::new("/a.pdf")), None);
    }
}
