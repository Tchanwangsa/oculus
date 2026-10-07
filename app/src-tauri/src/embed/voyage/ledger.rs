//! The allowance, the tier we learned, and the rate limiters in front of both.
//!
//! * **The allowance** — cumulative pixels, tokens and requests, plus a latch
//!   for the server's "you are out". Cumulative, not daily (unlike MinerU's):
//!   the free grant is a lifetime pool.
//! * **The tier** — this account's per-minute limits, never asked of the user:
//!   learned from 429 bodies and calm periods, and persisted for the next run.
//!
//! Reservations are taken before the network call and never given back — a
//! failed request may still have been billed, and erring optimistic ends in a
//! wall of refusals. A server "out of credit" outranks the local count.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::clock::now_secs;
use crate::embed::{EmbedError, Limiter};
use crate::ratelimit::{hold, TokenBucket};

// ── The limits on each programme ─────────────────────────────────────────────

/// With no payment method on file — the home of these numbers. A capped page
/// is `batch::tokens_for` of the cap, so TPM, not RPM, binds: under three
/// pages a minute. Running a tier-1 account at these would waste hours, which
/// is why the tier is detected.
pub const FREE_RPM: f64 = 3.0;
pub const FREE_TPM: f64 = 10_000.0;

/// With a card on file. Higher spend tiers are found by the detector raising
/// the ceiling after calm periods.
pub const TIER1_RPM: f64 = 2_000.0;
pub const TIER1_TPM: f64 = 2_000_000.0;

/// The free pixel grant: cumulative for the life of the account, and granted
/// to every account, paid or not — a payment method changes the per-minute
/// ceiling, not what the library costs.
pub const FREE_PIXELS: u64 = 150_000_000_000;

/// What a pixel costs past [`FREE_PIXELS`], per Voyage's pricing page.
pub const USD_PER_BILLION_PIXELS: f64 = 0.60;

/// The default spend guard: stop once the free grant is spent, since past it is
/// a bill and nobody watches a long run to the end. 0 means no guard.
pub const DEFAULT_STOP_AT_PERCENT: u8 = 100;

/// How long the server's "out of credit" keeps us off the network. Topping up
/// is unobservable, so the latch expires on a timer rather than at a reset.
pub const QUOTA_LATCH: Duration = Duration::from_secs(6 * 3600);

// ── The tier ─────────────────────────────────────────────────────────────────

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
        Self { rpm: TIER1_RPM, tpm: TIER1_TPM, source: TierSource::Assumed, learned_at: now_secs() }
    }
}

impl Tier {
    pub fn free() -> Self {
        Self { rpm: FREE_RPM, tpm: FREE_TPM, source: TierSource::Stated, learned_at: now_secs() }
    }

    /// Clamped from disk and the wire: zero would park every request forever.
    fn sane(mut self) -> Self {
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

    fn same_limits(&self, other: &Tier) -> bool {
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
            let Ok(mut parsed) = value.parse::<f64>() else { continue };

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

    let rpm = find(&["rpm", "requests per minute", "requests/min", "request per minute"])
        .filter(|value| (1.0..=100_000.0).contains(value));
    let tpm = find(&["tpm", "tokens per minute", "tokens/min", "token per minute"])
        .filter(|value| (1_000.0..=100_000_000.0).contains(value));
    (rpm, tpm)
}

/// Does this body describe the account's money rather than its pace? Pace is
/// a wait, money is a stop.
pub fn is_about_credit(text: &str) -> bool {
    let text = text.to_lowercase();
    ["out of credit", "insufficient", "exceeded your quota", "quota exceeded", "balance", "billing"]
        .iter()
        .any(|needle| text.contains(needle))
}

// ── The file on disk ─────────────────────────────────────────────────────────

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
            opened: crate::clock::today_utc(),
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
        Self { path: path.into(), guard: Mutex::new(()) }
    }

    /// The one ledger the app uses, beside the database and beside MinerU's.
    pub fn shared() -> Arc<UsageLedger> {
        static SHARED: OnceLock<Arc<UsageLedger>> = OnceLock::new();
        SHARED
            .get_or_init(|| {
                Arc::new(UsageLedger::at(crate::paths::data_dir().join("voyage-usage.json")))
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
                return Err(EmbedError::BudgetReached { percent: usage.stop_at_percent.min(100) });
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
        usage.tier = Tier { learned_at: now_secs(), ..tier.sane() };
        self.write(&usage);
    }

    /// Missing, unreadable or corrupt all mean a fresh record: the server is
    /// the backstop, not this file.
    fn read(&self) -> Usage {
        let usage = fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str::<Usage>(&text).ok())
            .unwrap_or_default();
        Usage { tier: usage.tier.sane(), ..usage }
    }

    /// Temp file (pid + nanos suffix) then rename, so a crash leaves the
    /// previous record.
    fn write(&self, usage: &Usage) {
        crate::atomic_write::json(&self.path, usage).ok();
    }
}

// ── Rate limiting ────────────────────────────────────────────────────────────

/// The adaptive throttle: two buckets, a pause, and the tier they are tuned to.
///
/// Start at what the ledger remembers (tier 1 if nothing). Every 429 lowers
/// the ceiling — to the stated numbers, else by half — and pauses. Every `calm`
/// period with no 429, below tier 1, doubles it. Both directions persist. On a
/// free account that costs one 429 per calm period, and notices a new card.
pub struct RateGate {
    requests: TokenBucket,
    tokens: TokenBucket,
    inner: Mutex<GateState>,
    resume: Condvar,
    ledger: Arc<UsageLedger>,
    calm: Duration,
    /// A test clock dilation; production is 1.0. Multiplies the rates and
    /// divides the pause, leaving every decision identical.
    pace: f64,
}

struct GateState {
    tier: Tier,
    paused_until: Option<Instant>,
    last_throttle: Instant,
    last_raise: Instant,
}

/// How long without a 429 before the throttle tries a higher ceiling.
pub const CALM: Duration = Duration::from_secs(300);

impl RateGate {
    pub fn new(ledger: Arc<UsageLedger>) -> Self {
        Self::with_calm(ledger, CALM)
    }

    pub fn with_calm(ledger: Arc<UsageLedger>, calm: Duration) -> Self {
        Self::with_pace(ledger, calm, 1.0)
    }

    pub fn with_pace(ledger: Arc<UsageLedger>, calm: Duration, pace: f64) -> Self {
        let pace = pace.max(1.0);
        let tier = ledger.tier();
        // In the past, so a fresh gate may probe immediately.
        let long_ago = Instant::now() - calm.min(Duration::from_secs(3600));
        Self {
            requests: TokenBucket::new(tier.rpm * pace),
            tokens: TokenBucket::new(tier.tpm * pace),
            inner: Mutex::new(GateState {
                tier,
                paused_until: None,
                last_throttle: long_ago,
                last_raise: Instant::now(),
            }),
            resume: Condvar::new(),
            ledger,
            calm,
            pace,
        }
    }

    /// The process-wide gate: one account, one set of per-minute limits.
    pub fn shared() -> Arc<RateGate> {
        static SHARED: OnceLock<Arc<RateGate>> = OnceLock::new();
        SHARED.get_or_init(|| Arc::new(RateGate::new(UsageLedger::shared()))).clone()
    }

    pub fn tier(&self) -> Tier {
        hold(&self.inner).tier
    }

    /// Block until this request may go out, spending one request and `tokens`
    /// tokens of the minute's budget.
    pub fn admit(&self, tokens: u64) {
        self.admit_reporting(tokens, &mut |_, _| {});
    }

    /// `admit`, telling `on_wait` before each block how long it expects to
    /// last and which limit imposes it. Called under a lock: keep it cheap.
    pub fn admit_reporting(&self, tokens: u64, on_wait: &mut dyn FnMut(Duration, Limiter)) {
        loop {
            let mut state = hold(&self.inner);
            let Some(until) = state.paused_until else { break };
            let now = Instant::now();
            if now >= until {
                state.paused_until = None;
                break;
            }
            // The buckets were drained by the 429 and refill during the pause,
            // so the request goes out at whichever ends later.
            on_wait((until - now).max(self.refill(tokens)), Limiter::Throttled);
            let _ = self.resume.wait_timeout(state, until - now);
        }
        let tier = self.tier();
        self.requests.acquire_n_reporting(1.0, &mut |wait| {
            on_wait(wait, Limiter::Requests { per_minute: tier.rpm.round() as u32 })
        });
        self.tokens.acquire_n_reporting(tokens as f64, &mut |wait| {
            on_wait(wait, Limiter::Tokens { per_minute: tier.tpm.round() as u32 })
        });
    }

    /// Roughly how long `admit(tokens)` would block if called now: the pause,
    /// or the slower bucket's refill. Other waiters are not counted.
    pub fn expected_wait(&self, tokens: u64) -> Duration {
        let paused = hold(&self.inner)
            .paused_until
            .map(|until| until.saturating_duration_since(Instant::now()))
            .unwrap_or_default();
        paused.max(self.refill(tokens))
    }

    fn refill(&self, tokens: u64) -> Duration {
        self.requests.shortfall(1.0).max(self.tokens.shortfall(tokens as f64))
    }

    /// Voyage said 429 — routine, not a failure. Returns the wait decided on.
    pub fn throttled(&self, retry_after: Option<f64>, body: &str) -> Duration {
        let (stated_rpm, stated_tpm) = stated_limits(body);
        let mut state = hold(&self.inner);
        let previous = state.tier;
        let learned = match (stated_rpm, stated_tpm) {
            // No numbers: halve, floored at the free programme.
            (None, None) => Tier {
                rpm: (previous.rpm / 2.0).max(FREE_RPM),
                tpm: (previous.tpm / 2.0).max(FREE_TPM),
                source: TierSource::Observed,
                learned_at: now_secs(),
            },
            (rpm, tpm) => Tier {
                rpm: rpm.unwrap_or(previous.rpm.min(FREE_RPM.max(previous.rpm / 2.0))),
                tpm: tpm.unwrap_or(previous.tpm.min(FREE_TPM.max(previous.tpm / 2.0))),
                source: TierSource::Stated,
                learned_at: now_secs(),
            },
        }
        .sane();

        // Voyage sends no `Retry-After`, so this is the ordinary path: one
        // request interval at the just-learned RPM.
        let wait = Duration::from_secs_f64(match retry_after {
            Some(seconds) => seconds.clamp(1.0, 120.0),
            None => (60.0 / learned.rpm).clamp(1.0, 60.0),
        });

        state.tier = learned;
        state.last_throttle = Instant::now();
        state.last_raise = Instant::now();
        let until = Instant::now() + wait.div_f64(self.pace);
        state.paused_until = Some(state.paused_until.unwrap_or_else(Instant::now).max(until));
        drop(state);

        self.requests.retune(learned.rpm * self.pace, true);
        self.tokens.retune(learned.tpm * self.pace, true);
        if !learned.same_limits(&previous) || learned.source != previous.source {
            self.ledger.store_tier(learned);
        }
        wait
    }

    /// A request came back: settle the bill and maybe raise the ceiling.
    pub fn succeeded(&self, estimated_tokens: u64, billed_tokens: Option<u64>) {
        if let Some(billed) = billed_tokens {
            self.ledger.settle(billed, estimated_tokens);
        }

        let mut state = hold(&self.inner);
        let previous = state.tier;
        // Both ceilings must arrive: they reach tier 1 in different numbers
        // of doublings.
        if (previous.tpm >= TIER1_TPM && previous.rpm >= TIER1_RPM)
            || state.last_throttle.elapsed() < self.calm
            || state.last_raise.elapsed() < self.calm
        {
            return;
        }
        let raised = Tier {
            rpm: (previous.rpm * 2.0).min(TIER1_RPM),
            tpm: (previous.tpm * 2.0).min(TIER1_TPM),
            source: TierSource::Observed,
            learned_at: now_secs(),
        }
        .sane();
        state.tier = raised;
        state.last_raise = Instant::now();
        drop(state);

        // Not drained: a widening keeps what was already earned.
        self.requests.retune(raised.rpm * self.pace, false);
        self.tokens.retune(raised.tpm * self.pace, false);
        self.ledger.store_tier(raised);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    fn scratch(name: &str) -> Scratch {
        Scratch::new(&format!("voyage-ledger-{name}"))
    }

    fn ledger_at(dir: &Path) -> Arc<UsageLedger> {
        Arc::new(UsageLedger::at(dir.join("voyage-usage.json")))
    }

    // ── The allowance ────────────────────────────────────────────────────────

    #[test]
    fn counts_survive_a_new_ledger_over_the_same_file() {
        let dir = scratch("persist");
        let path = dir.join("voyage-usage.json");
        UsageLedger::at(&path).record(2, 13_808, 7_732_734).unwrap();

        let usage = UsageLedger::at(&path).snapshot();
        assert_eq!(usage.requests, 2);
        assert_eq!(usage.tokens, 13_808);
        assert_eq!(usage.pixels, 7_732_734);
        assert!(!usage.quota_exhausted);
        // No scratch file left behind.
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn a_refused_reservation_moves_no_counter_at_all() {
        let dir = scratch("refuse");
        let ledger = ledger_at(&dir);
        ledger.store_tier(Tier::free());
        ledger.record(1, 3_572, FREE_PIXELS).unwrap();

        assert!(matches!(ledger.record(1, 3_572, 1), Err(EmbedError::BudgetReached { .. })));
        let usage = ledger.snapshot();
        assert_eq!(usage.pixels, FREE_PIXELS);
        assert_eq!(usage.tokens, 3_572);
        assert_eq!(usage.requests, 1);
    }

    /// The guard applies on a paid account too: past the grant it is billed.
    #[test]
    fn the_spend_guard_binds_a_paid_account_too() {
        let dir = scratch("paid");
        let ledger = ledger_at(&dir);
        ledger.store_tier(Tier {
            rpm: TIER1_RPM,
            tpm: TIER1_TPM,
            source: TierSource::Stated,
            learned_at: now_secs(),
        });
        ledger.record(1, 1, FREE_PIXELS - 1).unwrap();
        assert!(matches!(
            ledger.ensure_available(2),
            Err(EmbedError::BudgetReached { percent: 100 })
        ));
    }

    /// Turning the guard off lets a run go past the grant.
    #[test]
    fn turning_the_guard_off_lets_a_paid_account_past_the_grant() {
        let dir = scratch("guard-off");
        let ledger = ledger_at(&dir);
        ledger.store_stop_at(0);
        ledger.record(1, 1, FREE_PIXELS * 3).unwrap();
        assert!(ledger.ensure_available(FREE_PIXELS).is_ok());
    }

    /// A partial guard stops where it says it will, not at the grant.
    #[test]
    fn a_partial_guard_stops_at_its_own_percentage() {
        let dir = scratch("guard-half");
        let ledger = ledger_at(&dir);
        ledger.store_stop_at(50);
        ledger.record(1, 1, FREE_PIXELS / 2).unwrap();
        assert!(matches!(
            ledger.ensure_available(1),
            Err(EmbedError::BudgetReached { percent: 50 })
        ));
    }

    /// A ledger without the field reads back guarded: container-level
    /// `#[serde(default)]` uses `Usage::default()`, not `u8::default()` (off).
    #[test]
    fn an_older_ledger_reads_back_with_the_guard_on() {
        let dir = scratch("legacy");
        let path = dir.join("voyage-usage.json");
        fs::write(
            &path,
            r#"{"opened":"2026-09-01","requests":4,"tokens":10,"pixels":20,
                "quota_exhausted":false,"quota_latched_at":0}"#,
        )
        .unwrap();

        let usage = UsageLedger::at(&path).snapshot();
        assert_eq!(usage.stop_at_percent, DEFAULT_STOP_AT_PERCENT);
        assert_eq!(usage.budget(), Some(FREE_PIXELS));
        assert_eq!(usage.pixels, 20);
    }

    #[test]
    fn the_latch_refuses_from_a_fresh_instance_and_then_expires() {
        let dir = scratch("latch");
        let path = dir.join("voyage-usage.json");
        UsageLedger::at(&path).latch_exhausted();

        let reopened = UsageLedger::at(&path);
        assert!(matches!(reopened.ensure_available(0), Err(EmbedError::QuotaExhausted)));
        assert!(matches!(reopened.record(1, 1, 1), Err(EmbedError::QuotaExhausted)));

        let stale = Usage {
            quota_exhausted: true,
            quota_latched_at: now_secs() - QUOTA_LATCH.as_secs() - 1,
            ..Usage::default()
        };
        fs::write(&path, serde_json::to_vec(&stale).unwrap()).unwrap();
        assert!(UsageLedger::at(&path).ensure_available(0).is_ok());
    }

    #[test]
    fn settling_only_ever_tops_up() {
        let dir = scratch("settle");
        let ledger = ledger_at(&dir);
        ledger.record(1, 3_572, 2_000_000).unwrap();
        // Voyage billed more than the estimate: the difference is added.
        ledger.settle(3_600, 3_572);
        assert_eq!(ledger.snapshot().tokens, 3_600);
        // Less than the estimate gives nothing back.
        ledger.settle(10, 3_572);
        assert_eq!(ledger.snapshot().tokens, 3_600);
    }

    #[test]
    fn an_unreadable_ledger_is_a_fresh_record() {
        let dir = scratch("corrupt");
        let path = dir.join("voyage-usage.json");
        fs::write(&path, b"{not json at all").unwrap();

        let ledger = UsageLedger::at(&path);
        assert_eq!(ledger.snapshot().tokens, 0);
        ledger.record(1, 10, 100).unwrap();
        assert_eq!(UsageLedger::at(&path).snapshot().pixels, 100);
    }

    #[test]
    fn a_nonsense_tier_on_disk_never_parks_a_run_forever() {
        let dir = scratch("insane");
        let path = dir.join("voyage-usage.json");
        fs::write(&path, br#"{"tier":{"rpm":0,"tpm":-4,"source":"stated","learned_at":1}}"#)
            .unwrap();
        let tier = UsageLedger::at(&path).tier();
        assert!(tier.rpm >= 1.0 && tier.tpm >= 1_000.0, "{tier:?}");
    }

    // ── Reading the 429 ──────────────────────────────────────────────────────

    #[test]
    fn the_free_cap_is_read_straight_out_of_the_error_body() {
        let body = "Rate limit exceeded. You have hit the rate limit of 3 requests per \
                    minute (RPM) and 10000 tokens per minute (TPM) for voyage-multimodal-3.5. \
                    Add a payment method to raise it.";
        assert_eq!(stated_limits(body), (Some(3.0), Some(10_000.0)));
    }

    /// A live 429 body, verbatim: it writes the token limit as `10K`.
    const LIVE_FREE_TIER_429: &str = "You have not yet added your payment method in the \
        billing page and will have reduced rate limits of 3 RPM and 10K TPM. To unlock our \
        standard rate limits, please add a payment method in the billing page...";

    #[test]
    fn the_verbatim_live_429_body_yields_the_free_programme() {
        assert_eq!(stated_limits(LIVE_FREE_TIER_429), (Some(FREE_RPM), Some(FREE_TPM)));
    }

    #[test]
    fn a_suffixed_number_is_only_scaled_when_it_is_a_whole_word() {
        assert_eq!(stated_limits("limit of 2 RPM and 2M TPM"), (Some(2.0), Some(2_000_000.0)));
        // "10kb" is ten, which is out of range, so nothing is learned.
        assert_eq!(stated_limits("payload of 10kb exceeded (tpm)").1, None);
    }

    #[test]
    fn the_absent_retry_after_falls_back_to_the_pace_the_tier_allows() {
        let dir = scratch("fallback");
        let gate = RateGate::with_calm(ledger_at(&dir), Duration::from_secs(3_600));
        // No header (the live path): one request interval at 3 RPM.
        assert_eq!(gate.throttled(None, LIVE_FREE_TIER_429), Duration::from_secs(20));
        assert_eq!(gate.tier().tpm, FREE_TPM);

        // An explicit header wins.
        assert_eq!(gate.throttled(Some(5.0), LIVE_FREE_TIER_429), Duration::from_secs(5));
    }

    #[test]
    fn tier_one_numbers_read_too_including_separators() {
        let body = "You have exceeded 2,000 RPM / 2,000,000 TPM.";
        assert_eq!(stated_limits(body), (Some(2_000.0), Some(2_000_000.0)));
    }

    #[test]
    fn a_body_with_no_numbers_in_it_teaches_nothing() {
        assert_eq!(stated_limits("Too Many Requests"), (None, None));
        assert_eq!(stated_limits(""), (None, None));
        // A model id is not a rate limit, however close it sits to the word.
        assert_eq!(stated_limits("voyage-multimodal-3.5 rpm exceeded"), (None, None));
        // Nor is a distant number.
        let far = format!("512 dimensions.{} rate limited (rpm)", " ".repeat(60));
        assert_eq!(stated_limits(&far), (None, None));
    }

    #[test]
    fn money_and_pace_are_told_apart() {
        assert!(is_about_credit("Your account has run out of credit."));
        assert!(is_about_credit("insufficient balance"));
        assert!(!is_about_credit("Rate limit exceeded, 3 RPM"));
    }

    // ── Tier detection ───────────────────────────────────────────────────────

    #[test]
    fn a_fresh_account_starts_optimistic_and_is_corrected_by_the_first_429() {
        let dir = scratch("detect");
        let ledger = ledger_at(&dir);
        let gate = RateGate::with_calm(ledger.clone(), Duration::from_millis(50));

        // Optimistic (see `Tier::default`).
        assert_eq!(gate.tier().source, TierSource::Assumed);
        assert_eq!(gate.tier().tpm, TIER1_TPM);

        let waited = gate.throttled(
            Some(17.0),
            r#"{"detail":"rate limit of 3 RPM and 10000 TPM reached"}"#,
        );
        assert_eq!(waited, Duration::from_secs(17));

        let tier = gate.tier();
        assert_eq!(tier.source, TierSource::Stated);
        assert_eq!((tier.rpm, tier.tpm), (FREE_RPM, FREE_TPM));
        // Persisted.
        assert_eq!(ledger.tier().tpm, FREE_TPM);
    }

    #[test]
    fn a_learned_tier_is_where_the_next_run_starts() {
        let dir = scratch("resume");
        let ledger = ledger_at(&dir);
        RateGate::new(ledger.clone()).throttled(None, "rate limit of 3 RPM and 10000 TPM");

        let restarted = RateGate::new(ledger_at(&dir));
        assert_eq!(restarted.tier().tpm, FREE_TPM);
        assert_eq!(restarted.tier().source, TierSource::Stated);
    }

    #[test]
    fn a_429_with_nothing_readable_in_it_halves_rather_than_guessing() {
        let dir = scratch("halve");
        let gate = RateGate::with_calm(ledger_at(&dir), Duration::from_secs(3600));
        gate.throttled(Some(1.0), "<html>429 Too Many Requests</html>");
        let once = gate.tier();
        assert_eq!(once.tpm, TIER1_TPM / 2.0);
        assert_eq!(once.source, TierSource::Observed);

        // ...and keeps halving, floored at the free programme.
        for _ in 0..20 {
            gate.throttled(Some(1.0), "");
        }
        let floored = gate.tier();
        assert_eq!((floored.rpm, floored.tpm), (FREE_RPM, FREE_TPM));
    }

    #[test]
    fn a_calm_stretch_raises_the_ceiling_back_so_a_new_card_is_noticed() {
        let dir = scratch("raise");
        let ledger = ledger_at(&dir);
        let gate = RateGate::with_calm(ledger.clone(), Duration::from_millis(40));
        gate.throttled(Some(1.0), "rate limit of 3 RPM and 10000 TPM");
        assert_eq!(gate.tier().tpm, FREE_TPM);

        // Too soon: the calm period has not elapsed, so nothing moves.
        gate.succeeded(100, Some(100));
        assert_eq!(gate.tier().tpm, FREE_TPM);

        std::thread::sleep(Duration::from_millis(60));
        gate.succeeded(100, Some(100));
        assert_eq!(gate.tier().tpm, FREE_TPM * 2.0);
        assert_eq!(gate.tier().source, TierSource::Observed);
        // Persisted upwards as well as downwards.
        assert_eq!(ledger.tier().tpm, FREE_TPM * 2.0);

        // Climbs to tier 1 if the calm holds, never past it.
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(60));
            gate.succeeded(100, None);
        }
        assert_eq!(gate.tier().tpm, TIER1_TPM);
        assert_eq!(gate.tier().rpm, TIER1_RPM);
    }

    #[test]
    fn the_pause_from_a_retry_after_is_actually_served() {
        let dir = scratch("pause");
        let gate = RateGate::with_calm(ledger_at(&dir), Duration::from_secs(3600));
        // Tier-1 numbers, so the drained buckets refill in milliseconds and
        // only the pause is measured.
        gate.throttled(Some(1.0), "rate limit of 2000 RPM and 2000000 TPM");

        let start = Instant::now();
        gate.admit(1);
        assert!(start.elapsed() >= Duration::from_millis(900), "{:?}", start.elapsed());
    }

    #[test]
    fn a_billed_total_larger_than_the_estimate_reaches_the_ledger() {
        let dir = scratch("bill");
        let ledger = ledger_at(&dir);
        let gate = RateGate::with_calm(ledger.clone(), Duration::from_secs(3600));
        ledger.record(1, 3_572, 2_000_000).unwrap();
        gate.succeeded(3_572, Some(3_610));
        assert_eq!(ledger.snapshot().tokens, 3_610);
    }
}
