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

mod gate;
#[cfg(test)]
mod tests;
mod tier;
mod usage;

pub use gate::{RateGate, CALM};
pub use tier::{is_about_credit, stated_limits, Tier, TierSource};
pub use usage::{Usage, UsageLedger};

use std::time::Duration;

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
