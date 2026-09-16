//! The daily allowance, and the two rate limiters in front of it.
//!
//! MinerU publishes per-day and per-minute ceilings but exposes no endpoint
//! that says how much of them is left, so both are *guesses kept locally*. The
//! ledger is a small JSON file next to the database; it exists because the
//! guess has to survive a restart — a user who quits the app mid-import and
//! reopens it would otherwise start the day over with a full allowance and
//! march straight into the server's own refusal.
//!
//! Two rules give this file its shape, and neither is an accident:
//!
//! * **Reservations are taken before the network call and never given back.**
//!   A POST that fails, an upload that dies, a `SIGKILL` halfway through —
//!   all of them keep the files they reserved. The server may well have
//!   counted the work; we cannot ask, so an uncertain failure counts against
//!   us. Adding a rollback would make the local count drift optimistic, which
//!   is the one direction that turns into a wall of server-side rejections.
//! * **Server errors win over the local guess.** `latch_exhausted` is set from
//!   MinerU's own `-60018`, and once it is latched nothing goes near the
//!   network until the day rolls over.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::parse::ParseError;

/// MinerU's upload ceiling. The Python sliced documents over this limit into
/// physically smaller PDFs; this port refuses them instead (see
/// `client::MinerUCloud::build_tasks`).
pub const MAX_FILE_BYTES: u64 = 200 * 1024 * 1024;

/// The longest range one extraction task may cover. A longer document becomes
/// several tasks against the same uploaded file.
pub const MAX_PAGES_PER_TASK: u32 = 200;

/// How many tasks one `POST /file-urls/batch` may carry.
pub const MAX_FILES_PER_BATCH: usize = 50;

pub const SUBMIT_PER_MINUTE: f64 = 50.0;
pub const POLL_PER_MINUTE: f64 = 1000.0;

/// Files per day. Conservative Oculus policy where MinerU's page does not
/// publish an account limit — the server's own answer overrides it.
pub const DAILY_FILES: u64 = 5_000;

// The Python also carried `daily_html_files` and `daily_priority_pages`.
// Neither was ever read or incremented by anything that mattered — the HTML
// counter had no writer at all and the priority one only coloured a status
// field — so they are dropped rather than ported as dead configuration.

/// Exactly the JSON on disk.
///
/// `#[serde(default)]` is what makes a half-written or older record readable:
/// the Python wrote an `html_files` key this struct no longer has, and serde
/// ignores unknown fields, so yesterday's file still parses (and is then
/// discarded for being yesterday's).
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
        Self { date: beijing_day(), files: 0, pages: 0, quota_exhausted: false }
    }
}

pub struct UsageLedger {
    path: PathBuf,
    /// Serialises read-modify-write against *this process*. The Python's lock
    /// was per-process too, so two processes sharing a data directory could
    /// lose an increment. That is no longer reachable — parsing happens in the
    /// one app process now, not in a sidecar the CLI could also start — but
    /// the file format still offers no protection if it ever comes back.
    guard: Mutex<()>,
}

impl UsageLedger {
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), guard: Mutex::new(()) }
    }

    /// The one ledger the app uses, beside the database.
    pub fn shared() -> Arc<UsageLedger> {
        static SHARED: OnceLock<Arc<UsageLedger>> = OnceLock::new();
        SHARED
            .get_or_init(|| {
                Arc::new(UsageLedger::at(crate::paths::data_dir().join("mineru-usage.json")))
            })
            .clone()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Today's counters. Evaluated lazily rather than reset by a timer: the
    /// app is asleep for most of any given day, so a scheduled reset would
    /// simply not fire. A record dated other than today *is* the reset, and
    /// that is also the only thing that clears `quota_exhausted`.
    pub fn snapshot(&self) -> Usage {
        let _guard = hold(&self.guard);
        self.read()
    }

    /// Would `files` more fit? Checks the latch first: once MinerU has said no,
    /// nothing else is worth sending.
    pub fn ensure_available(&self, files: u64) -> Result<(), ParseError> {
        let usage = self.snapshot();
        if usage.quota_exhausted || usage.files + files > DAILY_FILES {
            return Err(ParseError::QuotaExhausted);
        }
        Ok(())
    }

    /// Reserve, and persist the reservation before returning.
    ///
    /// Either both counters move or neither does: a refusal must not leave the
    /// page count advanced for files that were never sent.
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

    /// MinerU itself said the quota is gone. Best effort: a ledger we cannot
    /// write still refuses for the rest of this run through the in-flight
    /// error, and tomorrow's file is a fresh day anyway.
    pub fn latch_exhausted(&self) {
        let _guard = hold(&self.guard);
        let mut usage = self.read();
        usage.quota_exhausted = true;
        self.write(&usage);
    }

    /// Missing, unreadable, corrupt or stale all mean the same thing: a fresh
    /// day. A ledger that refused to parse must never refuse the user's work —
    /// the server is the backstop, not this file.
    fn read(&self) -> Usage {
        let today = beijing_day();
        let usage = fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str::<Usage>(&text).ok())
            .unwrap_or_default();
        if usage.date != today {
            return Usage { date: today, ..Usage::default() };
        }
        usage
    }

    /// Temp file in the same directory, then rename — a crash mid-write leaves
    /// yesterday's record rather than half a line of JSON. The suffix carries
    /// the pid and a nanosecond stamp so two writers cannot pick the same
    /// scratch name.
    fn write(&self, usage: &Usage) {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let temporary =
            self.path.with_extension(format!("json.tmp-{}-{stamp}", std::process::id()));
        let Ok(body) = serde_json::to_vec_pretty(usage) else {
            return;
        };
        let staged = fs::File::create(&temporary)
            .and_then(|mut file| file.write_all(&body).and_then(|()| file.sync_all()));
        if staged.is_err() || fs::rename(&temporary, &self.path).is_err() {
            fs::remove_file(&temporary).ok();
        }
    }
}

/// The day boundary MinerU appears to reset on: a fixed UTC+8, no DST.
///
/// **Unconfirmed.** The provider does not publish the timezone its daily
/// counters roll over in; Beijing is the informed guess for a Chinese service
/// and it is what the Python assumed. Being wrong costs at most a few hours of
/// over-conservative refusals at one end of the day, which the server's own
/// answer corrects the moment a request goes out.
fn beijing_day() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default();
    let (year, month, day) = civil_from_days((seconds + 8 * 3600).div_euclid(86_400));
    format!("{year:04}-{month:02}-{day:02}")
}

/// Days since the epoch to a calendar date (Howard Hinnant's `civil_from_days`).
/// Hand-rolled because the app has no date crate and this is the only place
/// that needs one.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 { shifted_month + 3 } else { shifted_month - 9 };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

// ── Rate limiting ────────────────────────────────────────────────────────────

/// A classic token bucket: starts full, refills at `per_minute / 60` a second,
/// and `acquire` blocks until there is a token to take.
///
/// Both buckets are process-global and shared by every in-flight batch, which
/// is the only way the limit means anything — eight batches each politely
/// pacing themselves to 50/min would send 400.
pub struct TokenBucket {
    capacity: f64,
    rate: f64,
    state: Mutex<BucketState>,
    wake: Condvar,
}

struct BucketState {
    tokens: f64,
    updated: Instant,
}

impl TokenBucket {
    pub fn new(per_minute: f64) -> Self {
        Self {
            capacity: per_minute,
            rate: per_minute / 60.0,
            state: Mutex::new(BucketState { tokens: per_minute, updated: Instant::now() }),
            wake: Condvar::new(),
        }
    }

    pub fn acquire(&self) {
        let mut state = hold(&self.state);
        loop {
            let now = Instant::now();
            let elapsed = now.duration_since(state.updated).as_secs_f64();
            state.tokens = self.capacity.min(state.tokens + elapsed * self.rate);
            state.updated = now;
            if state.tokens >= 1.0 {
                state.tokens -= 1.0;
                return;
            }
            let short_by = 1.0 - state.tokens;
            let wait = Duration::from_secs_f64((short_by / self.rate).max(0.001));
            state = self.wake.wait_timeout(state, wait).unwrap_or_else(PoisonError::into_inner).0;
        }
    }
}

pub fn submit_bucket() -> Arc<TokenBucket> {
    static BUCKET: OnceLock<Arc<TokenBucket>> = OnceLock::new();
    BUCKET.get_or_init(|| Arc::new(TokenBucket::new(SUBMIT_PER_MINUTE))).clone()
}

pub fn poll_bucket() -> Arc<TokenBucket> {
    static BUCKET: OnceLock<Arc<TokenBucket>> = OnceLock::new();
    BUCKET.get_or_init(|| Arc::new(TokenBucket::new(POLL_PER_MINUTE))).clone()
}

/// A poisoned lock here means another thread panicked mid-update; the counters
/// are still readable and refusing to parse over it would be worse than
/// carrying on.
pub(super) fn hold<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("oculus-ledger-{name}-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn counts_survive_a_new_ledger_over_the_same_file() {
        let dir = scratch("persist");
        let path = dir.join("mineru-usage.json");
        UsageLedger::at(&path).record(3, 401).unwrap();

        let usage = UsageLedger::at(&path).snapshot();
        assert_eq!(usage.files, 3);
        assert_eq!(usage.pages, 401);
        assert!(!usage.quota_exhausted);
        assert_eq!(usage.date, beijing_day());
        // No scratch file left behind.
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_refused_reservation_does_not_move_the_page_counter() {
        let dir = scratch("limit");
        let ledger = UsageLedger::at(dir.join("mineru-usage.json"));
        ledger.record(DAILY_FILES, 2_001).unwrap();

        assert!(matches!(ledger.record(1, 1), Err(ParseError::QuotaExhausted)));
        // The partial application the Python test also pinned: a file check
        // that fails must leave both counters exactly where they were.
        let usage = ledger.snapshot();
        assert_eq!(usage.pages, 2_001);
        assert_eq!(usage.files, DAILY_FILES);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_latch_refuses_from_a_fresh_instance() {
        let dir = scratch("latch");
        let path = dir.join("mineru-usage.json");
        let ledger = UsageLedger::at(&path);
        ledger.ensure_available(0).unwrap();
        ledger.latch_exhausted();

        let reopened = UsageLedger::at(&path);
        assert!(matches!(reopened.ensure_available(0), Err(ParseError::QuotaExhausted)));
        assert!(matches!(reopened.record(1, 1), Err(ParseError::QuotaExhausted)));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_stale_day_resets_the_counters_and_clears_the_latch() {
        let dir = scratch("rollover");
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

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_unreadable_ledger_is_a_fresh_day() {
        let dir = scratch("corrupt");
        let path = dir.join("mineru-usage.json");
        fs::write(&path, b"{not json at all").unwrap();

        let ledger = UsageLedger::at(&path);
        assert_eq!(ledger.snapshot().files, 0);
        ledger.record(1, 10).unwrap();
        assert_eq!(UsageLedger::at(&path).snapshot().pages, 10);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn beijing_midnight_is_a_fixed_eight_hour_offset() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_days(19_617), (2023, 9, 17));

        // 2023-09-16T16:30:00Z — still the 16th in UTC, already the 17th in
        // Beijing, which is the whole reason the offset is applied before the
        // date is taken.
        let utc_evening = 1_694_881_800i64;
        assert_eq!(civil_from_days(utc_evening.div_euclid(86_400)), (2023, 9, 16));
        assert_eq!(civil_from_days((utc_evening + 8 * 3600).div_euclid(86_400)), (2023, 9, 17));
    }

    #[test]
    fn the_bucket_starts_full_and_then_paces() {
        let bucket = TokenBucket::new(600.0);
        let start = Instant::now();
        for _ in 0..600 {
            bucket.acquire();
        }
        assert!(start.elapsed() < Duration::from_millis(500), "a full bucket should not wait");

        let paced = Instant::now();
        bucket.acquire();
        assert!(paced.elapsed() >= Duration::from_millis(50), "an empty bucket must wait");
    }
}
