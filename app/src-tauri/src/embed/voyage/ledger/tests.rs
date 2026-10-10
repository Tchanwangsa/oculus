use crate::embed::EmbedError;
use crate::runtime::clock::now_secs;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::*;
use crate::test_support::Scratch;

fn scratch(name: &str) -> Scratch {
    Scratch::new(&format!("voyage-ledger-{name}"))
}

fn ledger_at(dir: &Path) -> Arc<UsageLedger> {
    Arc::new(UsageLedger::at(dir.join("voyage-usage.json")))
}

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

    assert!(matches!(
        ledger.record(1, 3_572, 1),
        Err(EmbedError::BudgetReached { .. })
    ));
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
    assert!(matches!(
        reopened.ensure_available(0),
        Err(EmbedError::QuotaExhausted)
    ));
    assert!(matches!(
        reopened.record(1, 1, 1),
        Err(EmbedError::QuotaExhausted)
    ));

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
    fs::write(
        &path,
        br#"{"tier":{"rpm":0,"tpm":-4,"source":"stated","learned_at":1}}"#,
    )
    .unwrap();
    let tier = UsageLedger::at(&path).tier();
    assert!(tier.rpm >= 1.0 && tier.tpm >= 1_000.0, "{tier:?}");
}

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
    assert_eq!(
        stated_limits(LIVE_FREE_TIER_429),
        (Some(FREE_RPM), Some(FREE_TPM))
    );
}

#[test]
fn a_suffixed_number_is_only_scaled_when_it_is_a_whole_word() {
    assert_eq!(
        stated_limits("limit of 2 RPM and 2M TPM"),
        (Some(2.0), Some(2_000_000.0))
    );
    // "10kb" is ten, which is out of range, so nothing is learned.
    assert_eq!(stated_limits("payload of 10kb exceeded (tpm)").1, None);
}

#[test]
fn the_absent_retry_after_falls_back_to_the_pace_the_tier_allows() {
    let dir = scratch("fallback");
    let gate = RateGate::with_calm(ledger_at(&dir), Duration::from_secs(3_600));
    // No header (the live path): one request interval at 3 RPM.
    assert_eq!(
        gate.throttled(None, LIVE_FREE_TIER_429),
        Duration::from_secs(20)
    );
    assert_eq!(gate.tier().tpm, FREE_TPM);

    // An explicit header wins.
    assert_eq!(
        gate.throttled(Some(5.0), LIVE_FREE_TIER_429),
        Duration::from_secs(5)
    );
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
    assert_eq!(
        stated_limits("voyage-multimodal-3.5 rpm exceeded"),
        (None, None)
    );
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
    assert!(
        start.elapsed() >= Duration::from_millis(900),
        "{:?}",
        start.elapsed()
    );
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
