//! The adaptive rate gate in front of the network.

use super::{
    stated_limits, Tier, TierSource, UsageLedger, FREE_RPM, FREE_TPM, TIER1_RPM, TIER1_TPM,
};
use crate::embed::Limiter;
use crate::providers::ratelimit::{hold, TokenBucket};
use crate::runtime::clock::now_secs;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

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
        SHARED
            .get_or_init(|| Arc::new(RateGate::new(UsageLedger::shared())))
            .clone()
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
            let Some(until) = state.paused_until else {
                break;
            };
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
            on_wait(
                wait,
                Limiter::Requests {
                    per_minute: tier.rpm.round() as u32,
                },
            )
        });
        self.tokens.acquire_n_reporting(tokens as f64, &mut |wait| {
            on_wait(
                wait,
                Limiter::Tokens {
                    per_minute: tier.tpm.round() as u32,
                },
            )
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
        self.requests
            .shortfall(1.0)
            .max(self.tokens.shortfall(tokens as f64))
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
