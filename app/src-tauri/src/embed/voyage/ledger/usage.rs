//! The usage file on disk (`voyage-usage.json`) and the counters in it.

use super::{Tier, DEFAULT_STOP_AT_PERCENT, FREE_PIXELS, QUOTA_LATCH};
use crate::embed::EmbedError;
use crate::providers::ratelimit::hold;
use crate::runtime::clock::now_secs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// Exactly the JSON in `voyage-usage.json`. `#[serde(default)]` lets it gain
/// fields without invalidating what is on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Usage {
    /// The day the ledger was first written. Informational, not a reset.
    pub opened: String,
    pub requests: u64,
    pub tokens: u64,
    pub pixels: u64,
    pub quota_exhausted: bool,
    /// Unix seconds the latch was set, so it can expire. See `QUOTA_LATCH`.
    pub quota_latched_at: u64,
    pub tier: Tier,
    /// Stop sending once `pixels` reaches this percentage of [`FREE_PIXELS`];
    /// 0 is no guard. Kept here rather than in `settings` so the reservation
    /// check reads it atomically with the counters.
    pub stop_at_percent: u8,
}

impl Default for Usage {
    fn default() -> Self {
        Self {
            opened: crate::runtime::clock::today_utc(),
            requests: 0,
            tokens: 0,
            pixels: 0,
            quota_exhausted: false,
            quota_latched_at: 0,
            tier: Tier::default(),
            // A file without this field reads back guarded, not off.
            stop_at_percent: DEFAULT_STOP_AT_PERCENT,
        }
    }
}

impl Usage {
    /// The latch, with its expiry applied. Always read it through here.
    pub fn latched(&self) -> bool {
        self.quota_exhausted
            && now_secs().saturating_sub(self.quota_latched_at) < QUOTA_LATCH.as_secs()
    }

    /// The pixel ceiling the guard imposes, or `None` when it is off. Applies
    /// on a paid account too: past the grant, that account is being billed.
    pub fn budget(&self) -> Option<u64> {
        let percent = self.stop_at_percent.min(100);
        (percent > 0).then(|| (FREE_PIXELS / 100).saturating_mul(percent as u64))
    }
}

pub struct UsageLedger {
    path: PathBuf,
    /// Serialises read-modify-write within this process only; nothing guards
    /// two processes sharing a data directory.
    guard: Mutex<()>,
}

impl UsageLedger {
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            guard: Mutex::new(()),
        }
    }

    /// The one ledger the app uses, beside the database and beside MinerU's.
    pub fn shared() -> Arc<UsageLedger> {
        static SHARED: OnceLock<Arc<UsageLedger>> = OnceLock::new();
        SHARED
            .get_or_init(|| {
                Arc::new(UsageLedger::at(
                    crate::library::paths::data_dir().join("voyage-usage.json"),
                ))
            })
            .clone()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn snapshot(&self) -> Usage {
        let _guard = hold(&self.guard);
        self.read()
    }

    /// Would `pixels` more fit? The server's latch first, then [`Usage::budget`].
    pub fn ensure_available(&self, pixels: u64) -> Result<(), EmbedError> {
        Self::affordable(&self.snapshot(), pixels)
    }

    /// Shared by the pre-run check and the reservation so they cannot drift.
    fn affordable(usage: &Usage, pixels: u64) -> Result<(), EmbedError> {
        if usage.latched() {
            return Err(EmbedError::QuotaExhausted);
        }
        if let Some(ceiling) = usage.budget() {
            if usage.pixels.saturating_add(pixels) > ceiling {
                return Err(EmbedError::BudgetReached {
                    percent: usage.stop_at_percent.min(100),
                });
            }
        }
        Ok(())
    }

    /// Reserve and persist before returning. All three counters move or none.
    pub fn record(&self, requests: u64, tokens: u64, pixels: u64) -> Result<(), EmbedError> {
        let _guard = hold(&self.guard);
        let mut usage = self.read();
        Self::affordable(&usage, pixels)?;
        usage.requests += requests;
        usage.tokens += tokens;
        usage.pixels += pixels;
        self.write(&usage);
        Ok(())
    }

    /// Voyage's `usage.total_tokens` exceeded the estimate: top the
    /// reservation up. Never takes anything back.
    pub fn settle(&self, billed: u64, estimated: u64) {
        let Some(extra) = billed.checked_sub(estimated).filter(|extra| *extra > 0) else {
            return;
        };
        let _guard = hold(&self.guard);
        let mut usage = self.read();
        usage.tokens += extra;
        self.write(&usage);
    }

    /// Voyage said the allowance is gone. Best effort: an unwritable ledger
    /// still refuses this run through the in-flight error.
    pub fn latch_exhausted(&self) {
        let _guard = hold(&self.guard);
        let mut usage = self.read();
        usage.quota_exhausted = true;
        usage.quota_latched_at = now_secs();
        self.write(&usage);
    }

    pub fn tier(&self) -> Tier {
        self.snapshot().tier.sane()
    }

    /// Move the spend guard, clamped to 0..=100.
    pub fn store_stop_at(&self, percent: u8) {
        let _guard = hold(&self.guard);
        let mut usage = self.read();
        usage.stop_at_percent = percent.min(100);
        self.write(&usage);
    }

    /// Persist what was learned, so a restart does not rediscover it.
    pub fn store_tier(&self, tier: Tier) {
        let _guard = hold(&self.guard);
        let mut usage = self.read();
        usage.tier = Tier {
            learned_at: now_secs(),
            ..tier.sane()
        };
        self.write(&usage);
    }

    /// Missing, unreadable or corrupt all mean a fresh record: the server is
    /// the backstop, not this file.
    fn read(&self) -> Usage {
        let usage = fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str::<Usage>(&text).ok())
            .unwrap_or_default();
        Usage {
            tier: usage.tier.sane(),
            ..usage
        }
    }

    /// Temp file (pid + nanos suffix) then rename, so a crash leaves the
    /// previous record.
    fn write(&self, usage: &Usage) {
        crate::runtime::atomic_write::json(&self.path, usage).ok();
    }
}
