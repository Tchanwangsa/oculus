//! The daily allowance, and the two rate limiters in front of it.
//!
//! MinerU exposes no endpoint for what is left, so the allowance is a local
//! guess, kept in a JSON file beside the database so it survives a restart.
//!
//! * **Reservations are taken before the network call and never given back**:
//!   the server may have counted the work, and a rollback would drift the
//!   count optimistic.
//! * **Server errors win over the local guess.** `latch_exhausted` is set from
//!   MinerU's own `-60018` and holds until the day rolls over.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::clock;
use crate::parse::ParseError;
use crate::ratelimit::{hold, TokenBucket};

/// MinerU's upload ceiling; a larger file is refused before upload.
pub const MAX_FILE_BYTES: u64 = 200 * 1024 * 1024;

/// The longest range one extraction task may cover.
pub const MAX_PAGES_PER_TASK: u32 = 200;

/// How many tasks one `POST /file-urls/batch` may carry.
pub const MAX_FILES_PER_BATCH: usize = 50;

pub const SUBMIT_PER_MINUTE: f64 = 50.0;
pub const POLL_PER_MINUTE: f64 = 1000.0;

/// Files per day: a conservative local policy; the server's answer overrides it.
pub const DAILY_FILES: u64 = 5_000;

/// Exactly the JSON on disk. `#[serde(default)]` keeps a partial or older
/// record readable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Usage {
    pub date: String,
    pub files: u64,
    pub pages: u64,
    pub quota_exhausted: bool,
}

impl Default for Usage {
    fn default() -> Self {
        Self {
            date: beijing_day(),
            files: 0,
            pages: 0,
            quota_exhausted: false,
        }
    }
}

pub struct UsageLedger {
    path: PathBuf,
    /// Serialises read-modify-write within this process only.
    guard: Mutex<()>,
}

impl UsageLedger {
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            guard: Mutex::new(()),
        }
    }

    /// The one ledger the app uses, beside the database.
    pub fn shared() -> Arc<UsageLedger> {
        static SHARED: OnceLock<Arc<UsageLedger>> = OnceLock::new();
        SHARED
            .get_or_init(|| {
                Arc::new(UsageLedger::at(
                    crate::paths::data_dir().join("mineru-usage.json"),
                ))
            })
            .clone()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Today's counters. No timer: a record dated other than today *is* the
    /// reset, and the only thing that clears `quota_exhausted`.
    pub fn snapshot(&self) -> Usage {
        let _guard = hold(&self.guard);
        self.read()
    }

    /// Would `files` more fit?
    pub fn ensure_available(&self, files: u64) -> Result<(), ParseError> {
        let usage = self.snapshot();
        if usage.quota_exhausted || usage.files + files > DAILY_FILES {
            return Err(ParseError::QuotaExhausted);
        }
        Ok(())
    }

    /// Reserve, and persist before returning. Both counters move or neither.
    pub fn record(&self, files: u64, pages: u64) -> Result<(), ParseError> {
        let _guard = hold(&self.guard);
        let mut usage = self.read();
        if usage.quota_exhausted || usage.files + files > DAILY_FILES {
            return Err(ParseError::QuotaExhausted);
        }
        usage.files += files;
        usage.pages += pages;
        self.write(&usage);
        Ok(())
    }

    /// MinerU itself said the quota is gone. Best effort.
    pub fn latch_exhausted(&self) {
        let _guard = hold(&self.guard);
        let mut usage = self.read();
        usage.quota_exhausted = true;
        self.write(&usage);
    }

    /// Missing, unreadable, corrupt or stale all mean a fresh day: the server
    /// is the backstop, not this file.
    fn read(&self) -> Usage {
        let today = beijing_day();
        let usage = fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str::<Usage>(&text).ok())
            .unwrap_or_default();
        if usage.date != today {
            return Usage {
                date: today,
                ..Usage::default()
            };
        }
        usage
    }

    /// Temp file in the same directory, then rename.
    fn write(&self, usage: &Usage) {
        crate::atomic_write::json(&self.path, usage).ok();
    }
}

/// The day boundary MinerU appears to reset on: a fixed UTC+8, no DST.
/// **Unconfirmed** — MinerU does not publish it; the server's answer corrects
/// a wrong guess.
fn beijing_day() -> String {
    clock::ymd((clock::now_secs() as i64 + 8 * 3600).div_euclid(86_400))
}

// ── Rate limiting ────────────────────────────────────────────────────────────

/// Process-global, or concurrent batches would each spend the whole limit.
pub fn submit_bucket() -> Arc<TokenBucket> {
    static BUCKET: OnceLock<Arc<TokenBucket>> = OnceLock::new();
    BUCKET
        .get_or_init(|| Arc::new(TokenBucket::new(SUBMIT_PER_MINUTE)))
        .clone()
}

pub fn poll_bucket() -> Arc<TokenBucket> {
    static BUCKET: OnceLock<Arc<TokenBucket>> = OnceLock::new();
    BUCKET
        .get_or_init(|| Arc::new(TokenBucket::new(POLL_PER_MINUTE)))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    #[test]
    fn counts_survive_a_new_ledger_over_the_same_file() {
        let dir = Scratch::new("ledger-persist");
        let path = dir.join("mineru-usage.json");
        UsageLedger::at(&path).record(3, 401).unwrap();

        let usage = UsageLedger::at(&path).snapshot();
        assert_eq!(usage.files, 3);
        assert_eq!(usage.pages, 401);
        assert!(!usage.quota_exhausted);
        assert_eq!(usage.date, beijing_day());
        // No scratch file left behind.
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn a_refused_reservation_does_not_move_the_page_counter() {
        let dir = Scratch::new("ledger-limit");
        let ledger = UsageLedger::at(dir.join("mineru-usage.json"));
        ledger.record(DAILY_FILES, 2_001).unwrap();

        assert!(matches!(
            ledger.record(1, 1),
            Err(ParseError::QuotaExhausted)
        ));
        // A refused reservation leaves both counters where they were.
        let usage = ledger.snapshot();
        assert_eq!(usage.pages, 2_001);
        assert_eq!(usage.files, DAILY_FILES);
    }

    #[test]
    fn the_latch_refuses_from_a_fresh_instance() {
        let dir = Scratch::new("ledger-latch");
        let path = dir.join("mineru-usage.json");
        let ledger = UsageLedger::at(&path);
        ledger.ensure_available(0).unwrap();
        ledger.latch_exhausted();

        let reopened = UsageLedger::at(&path);
        assert!(matches!(
            reopened.ensure_available(0),
            Err(ParseError::QuotaExhausted)
        ));
        assert!(matches!(
            reopened.record(1, 1),
            Err(ParseError::QuotaExhausted)
        ));
    }

    #[test]
    fn a_stale_day_resets_the_counters_and_clears_the_latch() {
        let dir = Scratch::new("ledger-rollover");
        let path = dir.join("mineru-usage.json");
        fs::write(
            &path,
            r#"{"date":"2001-01-01","files":4000,"pages":9,"quota_exhausted":true,"html_files":7}"#,
        )
        .unwrap();

        let usage = UsageLedger::at(&path).snapshot();
        assert_eq!(usage.date, beijing_day());
        assert_eq!(usage.files, 0);
        assert_eq!(usage.pages, 0);
        assert!(!usage.quota_exhausted);
    }

    #[test]
    fn an_unreadable_ledger_is_a_fresh_day() {
        let dir = Scratch::new("ledger-corrupt");
        let path = dir.join("mineru-usage.json");
        fs::write(&path, b"{not json at all").unwrap();

        let ledger = UsageLedger::at(&path);
        assert_eq!(ledger.snapshot().files, 0);
        ledger.record(1, 10).unwrap();
        assert_eq!(UsageLedger::at(&path).snapshot().pages, 10);
    }
}
