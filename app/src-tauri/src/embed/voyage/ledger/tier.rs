//! The tier: this account's per-minute limits, and how they are read off a 429.

use super::{FREE_RPM, FREE_TPM, TIER1_RPM, TIER1_TPM};
use crate::runtime::clock::now_secs;
use serde::{Deserialize, Serialize};

/// Where a set of limits came from, in increasing order of trust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TierSource {
    Assumed,
    /// Inferred from a 429 with no numbers, or from a calm period.
    Observed,
    /// Voyage said it, in a 429 body.
    Stated,
}

impl TierSource {
    pub fn as_str(self) -> &'static str {
        match self {
            TierSource::Assumed => "assumed",
            TierSource::Observed => "observed",
            TierSource::Stated => "stated",
        }
    }
}

/// One account's per-minute ceilings.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tier {
    pub rpm: f64,
    pub tpm: f64,
    pub source: TierSource,
    /// Unix seconds, for display only; decisions use the gate's `Instant`s.
    pub learned_at: u64,
}

impl Default for Tier {
    /// Optimistic: guessing high costs one 429, which teaches the real limits;
    /// guessing low costs hours and teaches nothing.
    fn default() -> Self {
        Self {
            rpm: TIER1_RPM,
            tpm: TIER1_TPM,
            source: TierSource::Assumed,
            learned_at: now_secs(),
        }
    }
}

impl Tier {
    pub fn free() -> Self {
        Self {
            rpm: FREE_RPM,
            tpm: FREE_TPM,
            source: TierSource::Stated,
            learned_at: now_secs(),
        }
    }

    /// Clamped from disk and the wire: zero would park every request forever.
    pub(super) fn sane(mut self) -> Self {
        if !self.rpm.is_finite() || self.rpm <= 0.0 {
            self.rpm = FREE_RPM;
        }
        if !self.tpm.is_finite() || self.tpm <= 0.0 {
            self.tpm = FREE_TPM;
        }
        self.rpm = self.rpm.clamp(1.0, 100_000.0);
        self.tpm = self.tpm.clamp(1_000.0, 100_000_000.0);
        self
    }

    /// Is this the free programme?
    pub fn is_free(&self) -> bool {
        self.tpm <= FREE_TPM * 1.5
    }

    pub(super) fn same_limits(&self, other: &Tier) -> bool {
        (self.rpm - other.rpm).abs() < 0.5 && (self.tpm - other.tpm).abs() < 0.5
    }
}

/// Pull `(rpm, tpm)` out of a 429 body — the only place the account's limits
/// are named, in prose. Tolerant and bounded: find the marker, take the nearest
/// number before it, refuse anything out of range. Nothing else is kept.
pub fn stated_limits(text: &str) -> (Option<f64>, Option<f64>) {
    /// How far back from a marker a number may sit and still be its number.
    const REACH: usize = 40;

    let lower = text.to_lowercase();
    let bytes = lower.as_bytes();

    // Every run of digits, with the index just past it.
    let mut numbers: Vec<(usize, f64)> = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_digit() {
            let start = index;
            let mut value = String::new();
            while index < bytes.len() && (bytes[index].is_ascii_digit() || bytes[index] == b',') {
                if bytes[index] != b',' {
                    value.push(bytes[index] as char);
                }
                index += 1;
            }
            // A decimal point means a version string, never a limit.
            let decimal = index < bytes.len() && bytes[index] == b'.';
            if !decimal && start > 0 && (bytes[start - 1] == b'.' || bytes[start - 1] == b'-') {
                continue;
            }
            if decimal {
                continue;
            }
            let Ok(mut parsed) = value.parse::<f64>() else {
                continue;
            };

            // Voyage writes "10K TPM". A suffix counts only as a whole word,
            // so "10kb" is still ten.
            let suffix = bytes.get(index).copied();
            let after = bytes.get(index + 1).copied();
            let standalone = !matches!(after, Some(c) if c.is_ascii_alphanumeric());
            if standalone {
                match suffix {
                    Some(b'k') => {
                        parsed *= 1_000.0;
                        index += 1;
                    }
                    Some(b'm') => {
                        parsed *= 1_000_000.0;
                        index += 1;
                    }
                    _ => {}
                }
            }
            numbers.push((index, parsed));
            continue;
        }
        index += 1;
    }

    let nearest = |marker: usize| -> Option<f64> {
        numbers
            .iter()
            .filter(|(end, _)| *end <= marker && marker - *end <= REACH)
            .max_by_key(|(end, _)| *end)
            .map(|(_, value)| *value)
    };
    let find = |needles: &[&str]| -> Option<f64> {
        needles
            .iter()
            .filter_map(|needle| lower.find(needle))
            .filter_map(nearest)
            .next()
    };

    let rpm = find(&[
        "rpm",
        "requests per minute",
        "requests/min",
        "request per minute",
    ])
    .filter(|value| (1.0..=100_000.0).contains(value));
    let tpm = find(&["tpm", "tokens per minute", "tokens/min", "token per minute"])
        .filter(|value| (1_000.0..=100_000_000.0).contains(value));
    (rpm, tpm)
}

/// Does this body describe the account's money rather than its pace? Pace is
/// a wait, money is a stop.
pub fn is_about_credit(text: &str) -> bool {
    let text = text.to_lowercase();
    [
        "out of credit",
        "insufficient",
        "exceeded your quota",
        "quota exceeded",
        "balance",
        "billing",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}
