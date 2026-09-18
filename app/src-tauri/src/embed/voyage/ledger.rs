//! The allowance, the tier we learned, and the two rate limiters in front of
//! both.
//!
//! This is `parse/mineru/ledger.rs` with one structural difference, and the
//! difference is the whole reason the file is worth reading. MinerU publishes a
//! fixed daily ceiling and never tells you where you are against it, so the
//! Python — and the Rust port — kept a local guess that reset at midnight.
//! Voyage's ceilings are **per minute**, they depend on whether a card is on
//! file, and **the server states them in its own 429 body**. So the ledger here
//! holds two different kinds of fact:
//!
//! * **The allowance** — cumulative pixels, tokens and requests spent, plus a
//!   latch for the server's own "you are out". Cumulative, *not* daily: the
//!   free grant is a lifetime pool (150B pixels), not a per-day refill, so a
//!   Beijing-midnight rollover would quietly hand the counter back every night
//!   and make it meaningless. That is the one piece of the MinerU ledger that
//!   did not transfer.
//! * **The tier** — what this account's per-minute limits actually are. Never
//!   asked of the user, never hardcoded: discovered from 429s and from calm
//!   periods, and written down so the next run starts where this one finished.
//!
//! The reservation discipline *is* MinerU's, unchanged and for the same reason:
//!
//! * **Reservations are taken before the network call and never given back.**
//!   A POST that fails, a connection that dies, a `SIGKILL` halfway through —
//!   all of them keep what they reserved, because Voyage may well have billed
//!   the work and there is no endpoint that says. An uncertain failure counts
//!   against us; the alternative drifts optimistic, which is the one direction
//!   that turns into a wall of server-side refusals.
//! * **Server errors win over the local guess.** A body that says the credit is
//!   gone latches, and nothing goes near the network until the latch expires.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::embed::EmbedError;

// ── What the limits are, on each of the two programs ─────────────────────────
//
// Measured against the live API on 2026-09-17, not read off a docs page.

/// With no payment method on file. ~2.8 pages a minute at 200 DPI (Voyage caps
/// billing at 2M pixels a page, so a full-DPI slide is ~3,572 tokens), which is
/// why
/// the tier has to be *detected*: running a tier-1 account at these numbers
/// turns a ten-minute re-index into a day and a half.
pub const FREE_RPM: f64 = 3.0;
pub const FREE_TPM: f64 = 10_000.0;

/// With a card on file. Doubles past $100 of billed usage and triples past
/// $1000 — which the detector will find on its own, because it raises the
/// ceiling whenever a calm period says it can.
pub const TIER1_RPM: f64 = 2_000.0;
pub const TIER1_TPM: f64 = 2_000_000.0;

/// The free pixel grant. Cumulative for the life of the account, and the
/// library bills 5.18B pixels — 3.5% of it, measured 2026-09-17 over 166 files
/// and 2,980 pages — so this is a backstop against a runaway loop, not a budget
/// anyone is close to.
///
/// **It is granted to every account, not only to free ones.** Voyage's pricing
/// page states the first 200M text tokens and 150B pixels are free "for every
/// account", which is why adding a payment method does not change what this
/// library costs — it changes the per-minute ceiling. The settings page says
/// that out loud, and this constant is where the number comes from.
pub const FREE_PIXELS: u64 = 150_000_000_000;

/// What a pixel costs **past** [`FREE_PIXELS`]: $0.60 per billion, from
/// Voyage's pricing page for `voyage-multimodal-3.5`. A 200-DPI page bills at
/// the 2,000,000-pixel cap, so a page past the grant is $0.0012 — the same
/// maximum the pricing page quotes per image, arrived at from the other end.
pub const USD_PER_BILLION_PIXELS: f64 = 0.60;

/// The default spend guard: stop once the free grant is spent.
///
/// It is 100 rather than "off" because the only thing on the other side of the
/// grant is a bill, and a run that can last most of a day is not something
/// anybody watches to the end. 0 means no guard at all.
pub const DEFAULT_STOP_AT_PERCENT: u8 = 100;

/// How long the server's "out of credit" keeps us off the network.
///
/// MinerU's latch cleared at the day boundary because its quota was daily.
/// Voyage's is not: a spent allowance clears when the user adds credit, which
/// nothing here can observe. So the latch expires on a timer instead — long
/// enough that a wedged run does not hammer a dead account, short enough that
/// topping up does not need a setting, a restart or a support search.
pub const QUOTA_LATCH: Duration = Duration::from_secs(6 * 3600);

// ── The tier ─────────────────────────────────────────────────────────────────

/// Where a set of limits came from, in increasing order of how much we believe
/// it. Only the ordering matters, and only `Stated` is a fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TierSource {
    /// Nobody has told us anything yet.
    Assumed,
    /// Inferred from behaviour — a 429 with no numbers in it, or a stretch of
    /// calm that says the ceiling is too low.
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
    /// Unix seconds, for the settings page. The *decisions* here are made from
    /// `Instant`s held in the gate; this is only what a human reads.
    pub learned_at: u64,
}

impl Default for Tier {
    /// **Optimistic, deliberately.**
    ///
    /// Guessing high costs one 429 — which is routine, carries its own
    /// `Retry-After`, and is the very thing that teaches us the real number.
    /// Guessing low costs 34 hours against 10 minutes and teaches us nothing,
    /// because a throttle that never pushes never learns. So a fresh install
    /// starts at tier 1 and is corrected downwards within a request or two.
    fn default() -> Self {
        Self { rpm: TIER1_RPM, tpm: TIER1_TPM, source: TierSource::Assumed, learned_at: now_secs() }
    }
}

impl Tier {
    pub fn free() -> Self {
        Self { rpm: FREE_RPM, tpm: FREE_TPM, source: TierSource::Stated, learned_at: now_secs() }
    }

    /// Clamped on the way in from disk and from the wire: a ceiling of zero
    /// would park every request forever, and one of a billion would burn the
    /// whole allowance into 429s.
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

    /// Is this the free programme? Used for the pixel-grant check — a paid
    /// account is not drawing on the free pool at all.
    pub fn is_free(&self) -> bool {
        self.tpm <= FREE_TPM * 1.5
    }

    fn same_limits(&self, other: &Tier) -> bool {
        (self.rpm - other.rpm).abs() < 0.5 && (self.tpm - other.tpm).abs() < 0.5
    }
}

/// Pull the numbers out of a 429 body.
///
/// Voyage states the cap it just enforced in prose — something on the order of
/// "you have hit the rate limit of 3 RPM and 10000 TPM" — and that sentence is
/// the single most valuable thing in this whole module, because it is the only
/// place the account's programme is ever named. It is also prose, so this
/// parser is written to be *tolerant and bounded* rather than exact: find the
/// marker, take the nearest number in front of it, and refuse anything outside
/// a sane range. A body that says nothing useful yields `(None, None)` and the
/// caller falls back to halving.
///
/// Nothing from `text` is kept beyond these two numbers — the body may echo the
/// request, which for this API means an echo of a page image.
pub fn stated_limits(text: &str) -> (Option<f64>, Option<f64>) {
    /// How far back from a marker a number may sit and still be its number.
    /// "3 requests per minute (RPM)" is 24 characters; a model id four
    /// sentences away is not.
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
            // A decimal point means a version string ("voyage-multimodal-3.5"),
            // never a limit; skip the whole thing rather than read "3".
            let decimal = index < bytes.len() && bytes[index] == b'.';
            if !decimal && start > 0 && (bytes[start - 1] == b'.' || bytes[start - 1] == b'-') {
                continue;
            }
            if decimal {
                continue;
            }
            let Ok(mut parsed) = value.parse::<f64>() else { continue };

            // **"10K TPM" is what Voyage actually writes**, and without this the
            // token limit reads as 10, falls outside the sanity range, and comes
            // back as "not stated" — which would leave the throttle at half of
            // whatever it was guessing while the request limit was correctly
            // learned as 3. A suffix only counts when it is a whole word, so
            // "10kb" is still ten.
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

/// Does this body describe the *account's money* rather than its pace?
///
/// The same judgement `voyage.rs` makes about a key probe, for the same reason
/// and with a different consequence: pace is a wait, money is a stop.
pub fn is_about_credit(text: &str) -> bool {
    let text = text.to_lowercase();
    ["out of credit", "insufficient", "exceeded your quota", "quota exceeded", "balance", "billing"]
        .iter()
        .any(|needle| text.contains(needle))
}

// ── The file on disk ─────────────────────────────────────────────────────────

/// Exactly the JSON in `voyage-usage.json`.
///
/// `#[serde(default)]` everywhere is what lets this file gain a field without
/// invalidating what is already on disk — and what makes a half-written record
/// readable rather than fatal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Usage {
    /// The day the ledger was first written. Informational only — unlike
    /// MinerU's `date` this is **not** a reset boundary, because the allowance
    /// it guards is cumulative.
    pub opened: String,
    pub requests: u64,
    pub tokens: u64,
    pub pixels: u64,
    pub quota_exhausted: bool,
    /// Unix seconds the latch was set, so it can expire. See `QUOTA_LATCH`.
    pub quota_latched_at: u64,
    pub tier: Tier,
    /// Stop sending once `pixels` reaches this percentage of [`FREE_PIXELS`].
    /// 0 is no guard. Set from Settings → Library and kept **here**, in the
    /// ledger, rather than in the `settings` row: the check that enforces it
    /// already reads this file on every reservation, the two are then one
    /// atomic read, and a process-wide singleton ledger picks a change up
    /// without being rebuilt. See [`UsageLedger::budget`].
    pub stop_at_percent: u8,
}

impl Default for Usage {
    fn default() -> Self {
        Self {
            opened: today(),
            requests: 0,
            tokens: 0,
            pixels: 0,
            quota_exhausted: false,
            quota_latched_at: 0,
            tier: Tier::default(),
            // Container-level `#[serde(default)]` fills a missing field from
            // *this* value, so a `voyage-usage.json` written before the guard
            // existed reads back as 100 rather than as "off" — the guard
            // arrives switched on, which is the safe direction.
            stop_at_percent: DEFAULT_STOP_AT_PERCENT,
        }
    }
}

impl Usage {
    /// The latch, with its expiry applied. Reading it anywhere else would let a
    /// stale latch outlive its six hours.
    pub fn latched(&self) -> bool {
        self.quota_exhausted
            && now_secs().saturating_sub(self.quota_latched_at) < QUOTA_LATCH.as_secs()
    }

    /// The pixel ceiling the guard imposes, or `None` when it is off.
    ///
    /// **This applies on a paid account too, and that is the point.** The old
    /// check only consulted the grant when the tier read as free, on the
    /// grounds that refusing a paid account's work against a free pool would
    /// be a limit the app invented. A percentage the user set is not invented:
    /// past 150B pixels a paid account is being billed, and the default of 100
    /// is "stop before this starts costing money".
    pub fn budget(&self) -> Option<u64> {
        let percent = self.stop_at_percent.min(100);
        (percent > 0).then(|| (FREE_PIXELS / 100).saturating_mul(percent as u64))
    }
}

pub struct UsageLedger {
    path: PathBuf,
    /// Serialises read-modify-write against *this* process, exactly as the
    /// MinerU ledger's does — and with the same caveat: the file format offers
    /// no protection between two processes sharing a data directory. Embedding
    /// happens in the one app process, and the CLI's `oculus index` is that
    /// process's own binary, so the window is not reachable today.
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

    /// Would `pixels` more fit? Checks the server's latch first: once Voyage
    /// has said the allowance is gone, nothing else is worth sending.
    ///
    /// The pixel ceiling is [`Usage::budget`] — the user's percentage of the
    /// free grant, on **either** programme. It used to be the whole grant and
    /// only on the free one; see `budget` for why that moved.
    pub fn ensure_available(&self, pixels: u64) -> Result<(), EmbedError> {
        Self::affordable(&self.snapshot(), pixels)
    }

    /// The two refusals, in one place so the check before a run and the check
    /// inside a reservation cannot drift apart.
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

    /// Reserve, and persist the reservation before returning.
    ///
    /// Either all three counters move or none does: a refusal must not leave
    /// the token count advanced for pixels that were never sent.
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

    /// The server billed more than we estimated. Tops the reservation up and
    /// **never** takes anything back — see the header. Voyage reports
    /// `usage.total_tokens` on every response, so the estimate is corrected
    /// upwards from the only authority there is.
    pub fn settle(&self, billed: u64, estimated: u64) {
        let Some(extra) = billed.checked_sub(estimated).filter(|extra| *extra > 0) else {
            return;
        };
        let _guard = hold(&self.guard);
        let mut usage = self.read();
        usage.tokens += extra;
        self.write(&usage);
    }

    /// Voyage itself said the allowance is gone. Best effort: a ledger we
    /// cannot write still refuses for the rest of this run through the
    /// in-flight error.
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

    /// Move the spend guard. Clamped to 0..=100 here rather than trusted from
    /// the caller, because this number is the only thing between a long
    /// unattended run and a bill.
    pub fn store_stop_at(&self, percent: u8) {
        let _guard = hold(&self.guard);
        let mut usage = self.read();
        usage.stop_at_percent = percent.min(100);
        self.write(&usage);
    }

    /// Write down what was learned, so a restart does not rediscover it. Only
    /// called when the numbers actually moved — the gate compares first.
    pub fn store_tier(&self, tier: Tier) {
        let _guard = hold(&self.guard);
        let mut usage = self.read();
        usage.tier = Tier { learned_at: now_secs(), ..tier.sane() };
        self.write(&usage);
    }

    /// Missing, unreadable or corrupt all mean the same thing: a fresh record.
    /// A ledger that refused to parse must never refuse the user's work — the
    /// server is the backstop, not this file.
    fn read(&self) -> Usage {
        let usage = fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str::<Usage>(&text).ok())
            .unwrap_or_default();
        Usage { tier: usage.tier.sane(), ..usage }
    }

    /// Temp file in the same directory, then rename — a crash mid-write leaves
    /// the previous record rather than half a line of JSON. The suffix carries
    /// the pid and a nanosecond stamp so two writers cannot pick the same
    /// scratch name.
    fn write(&self, usage: &Usage) {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let temporary = self
            .path
            .with_extension(format!("json.tmp-{}-{}", std::process::id(), now_nanos()));
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

// ── Rate limiting ────────────────────────────────────────────────────────────

/// A token bucket that can be **retuned while it is running**, which is the one
/// thing MinerU's could not do.
///
/// Everything else is the same classic shape: starts full, refills at
/// `per_minute / 60` a second, and `acquire_n` blocks until there is enough to
/// take. Process-global and shared by every in-flight request, which is the
/// only way a limit means anything — four workers each politely pacing
/// themselves to 3 RPM would send 12.
pub struct TokenBucket {
    state: Mutex<BucketState>,
    wake: Condvar,
}

struct BucketState {
    capacity: f64,
    rate: f64,
    tokens: f64,
    updated: Instant,
}

impl TokenBucket {
    pub fn new(per_minute: f64) -> Self {
        let per_minute = per_minute.max(1.0);
        Self {
            state: Mutex::new(BucketState {
                capacity: per_minute,
                rate: per_minute / 60.0,
                tokens: per_minute,
                updated: Instant::now(),
            }),
            wake: Condvar::new(),
        }
    }

    pub fn acquire(&self) {
        self.acquire_n(1.0);
    }

    /// Take `cost`, waiting for the refill if the bucket is short.
    ///
    /// **`cost` is clamped to the capacity.** One request may legitimately ask
    /// for more tokens than a minute's worth — a request holds up to 320,000
    /// and the free ceiling is 10,000 — and an unclamped wait for a bucket that
    /// can never hold that much is a deadlock. Clamped, the request waits for a
    /// full minute's budget and then goes, and the server's 429 (which is
    /// routine, honoured and not a failure) handles the rest.
    pub fn acquire_n(&self, cost: f64) {
        let mut state = hold(&self.state);
        loop {
            let now = Instant::now();
            let elapsed = now.duration_since(state.updated).as_secs_f64();
            state.tokens = state.capacity.min(state.tokens + elapsed * state.rate);
            state.updated = now;

            let want = cost.max(0.0).min(state.capacity);
            if state.tokens >= want {
                state.tokens -= want;
                return;
            }
            let short_by = want - state.tokens;
            let wait = Duration::from_secs_f64((short_by / state.rate).clamp(0.001, 60.0));
            state = self.wake.wait_timeout(state, wait).unwrap_or_else(PoisonError::into_inner).0;
        }
    }

    /// Move the ceiling. `drain` empties the bucket on the way — which is what
    /// a 429 wants, because the burst that earned the 429 is exactly the thing
    /// that must not happen again the moment the pause lifts.
    pub fn retune(&self, per_minute: f64, drain: bool) {
        let per_minute = per_minute.max(1.0);
        let mut state = hold(&self.state);
        state.capacity = per_minute;
        state.rate = per_minute / 60.0;
        state.tokens = if drain { 0.0 } else { state.tokens.min(per_minute) };
        state.updated = Instant::now();
        drop(state);
        self.wake.notify_all();
    }

    pub fn per_minute(&self) -> f64 {
        hold(&self.state).capacity
    }
}

/// The adaptive throttle: two buckets, a pause, and the tier they are tuned to.
///
/// The detection loop in one paragraph. Start at whatever the ledger
/// remembers, or tier 1 if it remembers nothing. Every 429 lowers the ceiling —
/// to the numbers the body states if it states any, otherwise by half — and
/// pauses for `Retry-After`. Every stretch of `calm` with no 429 at all, while
/// the ceiling is still below tier 1, doubles it back up. Both directions are
/// written to the ledger, so a restart resumes where the last run finished.
///
/// On a free account that settles into a gentle oscillation: throttled to
/// 3 RPM, five quiet minutes, a probe at 6 RPM, one 429, back to 3. One 429
/// per calm period is the entire cost of never asking the user which plan they
/// are on — and of noticing, without being told, the day they add a card.
pub struct RateGate {
    requests: TokenBucket,
    tokens: TokenBucket,
    inner: Mutex<GateState>,
    resume: Condvar,
    ledger: Arc<UsageLedger>,
    calm: Duration,
    /// **A test dilation, and nothing else.** Production is 1.0. The buckets
    /// and the pause are the only waits in this module that are not routed
    /// through the client's `time_scale`, and at 3 RPM a faithful test of the
    /// shrink-and-continue path would take a minute of wall clock. Multiplying
    /// the per-minute rates (and dividing the pause) leaves every *decision*
    /// identical and only compresses the clock.
    pace: f64,
}

struct GateState {
    tier: Tier,
    paused_until: Option<Instant>,
    last_throttle: Instant,
    last_raise: Instant,
}

/// How long the account must go without a 429 before the throttle tries a
/// higher ceiling. Long enough that a struggling free account never reaches it
/// mid-run; short enough that adding a card is noticed inside one sitting.
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
        // Both clocks start in the past so a fresh gate is immediately
        // eligible to probe rather than serving out a calm period it never
        // spent.
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

    /// The process-wide gate. One account, one set of per-minute limits — a
    /// per-client gate would let two documents each spend the whole budget.
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
        loop {
            let mut state = hold(&self.inner);
            let Some(until) = state.paused_until else { break };
            let now = Instant::now();
            if now >= until {
                state.paused_until = None;
                break;
            }
            let _ = self.resume.wait_timeout(state, until - now);
        }
        self.requests.acquire();
        self.tokens.acquire_n(tokens as f64);
    }

    /// Voyage said 429. **Not a failure** — this is what a working run looks
    /// like most of the time on the free programme.
    ///
    /// Returns the wait it decided on, so the caller can report it honestly
    /// when it is long enough to notice.
    pub fn throttled(&self, retry_after: Option<f64>, body: &str) -> Duration {
        let (stated_rpm, stated_tpm) = stated_limits(body);
        let mut state = hold(&self.inner);
        let previous = state.tier;
        let learned = match (stated_rpm, stated_tpm) {
            // The server named the programme. Nothing else we could infer
            // outranks that.
            (None, None) => Tier {
                // No numbers to read: the ceiling we were using is too high by
                // an unknown amount, so halve it and floor at the free
                // programme, which is the lowest Voyage runs.
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

        // **`Retry-After` is absent on these 429s** — measured live, the header
        // is simply not sent — so this fallback is the ordinary path and not a
        // safety net. It is derived rather than magic: one request per
        // `60 / rpm` seconds is the pace the ceiling we just learned actually
        // permits, which on the free programme is the 20 seconds that 3 RPM
        // means. Taken *after* learning, so the first 429 of a run waits the
        // real interval rather than the optimistic one.
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

        // Drained: the burst that earned the 429 must not repeat the instant
        // the pause lifts.
        self.requests.retune(learned.rpm * self.pace, true);
        self.tokens.retune(learned.tpm * self.pace, true);
        if !learned.same_limits(&previous) || learned.source != previous.source {
            self.ledger.store_tier(learned);
        }
        wait
    }

    /// A request came back. Feeds the *upward* half of the detector and
    /// reconciles the ledger against what Voyage actually billed.
    pub fn succeeded(&self, estimated_tokens: u64, billed_tokens: Option<u64>) {
        if let Some(billed) = billed_tokens {
            self.ledger.settle(billed, estimated_tokens);
        }

        let mut state = hold(&self.inner);
        let previous = state.tier;
        // Both ceilings, not either: TPM doubles from 10K to its cap in eight
        // steps and RPM needs ten from 3, so stopping at the first one to
        // arrive would leave a tier-1 account permanently pinned to 768 RPM.
        if (previous.tpm >= TIER1_TPM && previous.rpm >= TIER1_RPM)
            || state.last_throttle.elapsed() < self.calm
            || state.last_raise.elapsed() < self.calm
        {
            return;
        }
        // Calm for a whole period at a ceiling below tier 1: the ceiling is a
        // guess, and the only way to find out it is too low is to raise it and
        // see. A wrong guess costs exactly one 429.
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

        // Not drained: this is a widening, and the tokens already earned at the
        // old rate are still owed to us.
        self.requests.retune(raised.rpm * self.pace, false);
        self.tokens.retune(raised.tpm * self.pace, false);
        self.ledger.store_tier(raised);
    }
}

// ── Small shared things ──────────────────────────────────────────────────────

/// A poisoned lock here means another thread panicked mid-update; the counters
/// are still readable and refusing to embed over it would be worse than
/// carrying on. Same call as `parse/mineru/ledger.rs::hold`, kept local so the
/// two seams do not reach into each other.
pub fn hold<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default()
}

fn now_nanos() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default()
}

/// Local calendar day, for the `opened` stamp only. Nothing branches on it —
/// unlike MinerU's ledger, where the date *is* the reset.
fn today() -> String {
    let days = (now_secs() as i64).div_euclid(86_400);
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
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oculus-voyage-ledger-{name}-{}", now_nanos()));
        fs::create_dir_all(&dir).unwrap();
        dir
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

        fs::remove_dir_all(&dir).ok();
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

        fs::remove_dir_all(&dir).ok();
    }

    /// **This reverses an earlier rule, deliberately.** The grant check used to
    /// be skipped on a paid account, on the grounds that refusing its work
    /// against a free pool would be a limit the app invented. The guard is not
    /// invented — it is a percentage the user set — and past the grant a paid
    /// account is being *billed*, which is the one moment a long unattended run
    /// most needs a brake.
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

        fs::remove_dir_all(&dir).ok();
    }

    /// And the escape hatch, because past the grant is a *price*, not a wall:
    /// somebody who means to pay for it turns the guard off and the app stops
    /// having an opinion.
    #[test]
    fn turning_the_guard_off_lets_a_paid_account_past_the_grant() {
        let dir = scratch("guard-off");
        let ledger = ledger_at(&dir);
        ledger.store_stop_at(0);
        ledger.record(1, 1, FREE_PIXELS * 3).unwrap();
        assert!(ledger.ensure_available(FREE_PIXELS).is_ok());

        fs::remove_dir_all(&dir).ok();
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

        fs::remove_dir_all(&dir).ok();
    }

    /// The guard must arrive switched on over a ledger written before it
    /// existed. Container-level `#[serde(default)]` fills a missing field from
    /// `Usage::default()`, so this pins that and not serde's own `u8::default`,
    /// which would be 0 — "off", silently, on every install that had already
    /// indexed anything.
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

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_latch_refuses_from_a_fresh_instance_and_then_expires() {
        let dir = scratch("latch");
        let path = dir.join("voyage-usage.json");
        UsageLedger::at(&path).latch_exhausted();

        let reopened = UsageLedger::at(&path);
        assert!(matches!(reopened.ensure_available(0), Err(EmbedError::QuotaExhausted)));
        assert!(matches!(reopened.record(1, 1, 1), Err(EmbedError::QuotaExhausted)));

        // Unlike MinerU's day boundary, this one is a timer — a topped-up
        // account recovers without a setting or a restart.
        let stale = Usage {
            quota_exhausted: true,
            quota_latched_at: now_secs() - QUOTA_LATCH.as_secs() - 1,
            ..Usage::default()
        };
        fs::write(&path, serde_json::to_vec(&stale).unwrap()).unwrap();
        assert!(UsageLedger::at(&path).ensure_available(0).is_ok());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn settling_only_ever_tops_up() {
        let dir = scratch("settle");
        let ledger = ledger_at(&dir);
        ledger.record(1, 3_572, 2_000_000).unwrap();
        // Voyage billed more than the estimate: the difference is added.
        ledger.settle(3_600, 3_572);
        assert_eq!(ledger.snapshot().tokens, 3_600);
        // And less than the estimate gives nothing back, which is the rule the
        // whole ledger is built on.
        ledger.settle(10, 3_572);
        assert_eq!(ledger.snapshot().tokens, 3_600);

        fs::remove_dir_all(&dir).ok();
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

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_nonsense_tier_on_disk_never_parks_a_run_forever() {
        let dir = scratch("insane");
        let path = dir.join("voyage-usage.json");
        fs::write(&path, br#"{"tier":{"rpm":0,"tpm":-4,"source":"stated","learned_at":1}}"#)
            .unwrap();
        let tier = UsageLedger::at(&path).tier();
        assert!(tier.rpm >= 1.0 && tier.tpm >= 1_000.0, "{tier:?}");

        fs::remove_dir_all(&dir).ok();
    }

    // ── Reading the 429 ──────────────────────────────────────────────────────

    #[test]
    fn the_free_cap_is_read_straight_out_of_the_error_body() {
        // The shape the live API answers with: the numbers are in the prose.
        let body = "Rate limit exceeded. You have hit the rate limit of 3 requests per \
                    minute (RPM) and 10000 tokens per minute (TPM) for voyage-multimodal-3.5. \
                    Add a payment method to raise it.";
        assert_eq!(stated_limits(body), (Some(3.0), Some(10_000.0)));
    }

    /// The body Voyage actually sends, verbatim from a live 429 on this
    /// account. Paraphrases are how a parser passes its tests and fails the
    /// wire: the real string writes the token limit as `10K`, not `10000`.
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
        // "10kb" is ten kilobytes, not ten thousand of anything — and ten is
        // outside the sane token range, so it teaches nothing rather than
        // teaching something wrong.
        assert_eq!(stated_limits("payload of 10kb exceeded (tpm)").1, None);
    }

    #[test]
    fn the_absent_retry_after_falls_back_to_the_pace_the_tier_allows() {
        let dir = scratch("fallback");
        let gate = RateGate::with_calm(ledger_at(&dir), Duration::from_secs(3_600));
        // No header, which is the live path: 3 RPM is one request per 20s.
        assert_eq!(gate.throttled(None, LIVE_FREE_TIER_429), Duration::from_secs(20));
        assert_eq!(gate.tier().tpm, FREE_TPM);

        // And an explicit header still wins when there is one.
        assert_eq!(gate.throttled(Some(5.0), LIVE_FREE_TIER_429), Duration::from_secs(5));

        fs::remove_dir_all(&dir).ok();
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
        // Nor is a number four sentences away.
        let far = format!("512 dimensions.{} rate limited (rpm)", " ".repeat(60));
        assert_eq!(stated_limits(&far), (None, None));
    }

    #[test]
    fn money_and_pace_are_told_apart() {
        assert!(is_about_credit("Your account has run out of credit."));
        assert!(is_about_credit("insufficient balance"));
        assert!(!is_about_credit("Rate limit exceeded, 3 RPM"));
    }

    // ── The bucket ───────────────────────────────────────────────────────────

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

    #[test]
    fn a_cost_larger_than_a_whole_minute_does_not_deadlock() {
        // A request may hold 320,000 tokens against a 10,000 TPM ceiling. The
        // clamp is what stops that being an unbounded wait.
        let bucket = TokenBucket::new(6_000.0);
        let start = Instant::now();
        bucket.acquire_n(320_000.0);
        assert!(start.elapsed() < Duration::from_millis(200), "{:?}", start.elapsed());
    }

    #[test]
    fn retuning_narrows_the_ceiling_and_can_drain_the_burst() {
        let bucket = TokenBucket::new(60_000.0);
        bucket.retune(6_000.0, true);
        assert_eq!(bucket.per_minute(), 6_000.0);
        let start = Instant::now();
        bucket.acquire_n(100.0);
        // Drained: even a token has to be waited for.
        assert!(start.elapsed() >= Duration::from_millis(500), "{:?}", start.elapsed());
    }

    // ── Tier detection ───────────────────────────────────────────────────────

    #[test]
    fn a_fresh_account_starts_optimistic_and_is_corrected_by_the_first_429() {
        let dir = scratch("detect");
        let ledger = ledger_at(&dir);
        let gate = RateGate::with_calm(ledger.clone(), Duration::from_millis(50));

        // Optimistic: guessing high costs one 429, guessing low costs 34 hours.
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
        // And it is on disk, so the next run does not rediscover it.
        assert_eq!(ledger.tier().tpm, FREE_TPM);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_learned_tier_is_where_the_next_run_starts() {
        let dir = scratch("resume");
        let ledger = ledger_at(&dir);
        RateGate::new(ledger.clone()).throttled(None, "rate limit of 3 RPM and 10000 TPM");

        let restarted = RateGate::new(ledger_at(&dir));
        assert_eq!(restarted.tier().tpm, FREE_TPM);
        assert_eq!(restarted.tier().source, TierSource::Stated);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_429_with_nothing_readable_in_it_halves_rather_than_guessing() {
        let dir = scratch("halve");
        let gate = RateGate::with_calm(ledger_at(&dir), Duration::from_secs(3600));
        gate.throttled(Some(1.0), "<html>429 Too Many Requests</html>");
        let once = gate.tier();
        assert_eq!(once.tpm, TIER1_TPM / 2.0);
        assert_eq!(once.source, TierSource::Observed);

        // ...and keeps halving, with the free programme as the floor. Nothing
        // Voyage runs is slower than that.
        for _ in 0..20 {
            gate.throttled(Some(1.0), "");
        }
        let floored = gate.tier();
        assert_eq!((floored.rpm, floored.tpm), (FREE_RPM, FREE_TPM));

        fs::remove_dir_all(&dir).ok();
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

        // And it climbs all the way to tier 1 if the calm holds, never past it.
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(60));
            gate.succeeded(100, None);
        }
        assert_eq!(gate.tier().tpm, TIER1_TPM);
        assert_eq!(gate.tier().rpm, TIER1_RPM);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_pause_from_a_retry_after_is_actually_served() {
        let dir = scratch("pause");
        let gate = RateGate::with_calm(ledger_at(&dir), Duration::from_secs(3600));
        // Tier-1 numbers on purpose: the drained buckets refill in
        // milliseconds, so what this measures is the pause and nothing else.
        // (At the free ceiling the same call would also wait the 20 seconds
        // that 3 RPM actually means, which is correct and untestable.)
        gate.throttled(Some(1.0), "rate limit of 2000 RPM and 2000000 TPM");

        let start = Instant::now();
        gate.admit(1);
        assert!(start.elapsed() >= Duration::from_millis(900), "{:?}", start.elapsed());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_billed_total_larger_than_the_estimate_reaches_the_ledger() {
        let dir = scratch("bill");
        let ledger = ledger_at(&dir);
        let gate = RateGate::with_calm(ledger.clone(), Duration::from_secs(3600));
        ledger.record(1, 3_572, 2_000_000).unwrap();
        gate.succeeded(3_572, Some(3_610));
        assert_eq!(ledger.snapshot().tokens, 3_610);

        fs::remove_dir_all(&dir).ok();
    }
}
