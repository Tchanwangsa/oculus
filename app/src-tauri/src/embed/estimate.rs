//! What the outstanding index run will cost, before it is started — computed
//! from the same arithmetic the run bills against, without sending anything:
//!
//! * Voyage bills pixels, capped per image (`batch::BILLED_PIXEL_CAP`,
//!   `ledger::USD_PER_BILLION_PIXELS`); page boxes come from pdfium without
//!   rasterising (`raster::page_sizes`).
//! * Packing is deterministic: `batch::plan` is the run's own function.
//! * The pace is the learned tier: tokens over TPM or requests over RPM,
//!   whichever is slower.
//!
//! It predicts; `ledger` decides. It only reports where the run would land
//! against the spend guard, never enforces it.

use std::path::{Path, PathBuf};

use serde::Serialize;
use sqlx::Row;

use super::raster;
use super::voyage::batch;
use super::voyage::ledger::{self, UsageLedger};
use super::{EMBED_DIM, EMBED_MODEL};

/// One bucket of the breakdown, by file extension.
#[derive(Serialize, Default, Clone)]
pub struct Bucket {
    pub label: String,
    pub files: u32,
    pub pages: u32,
}

/// The whole prediction, in the units the banner quotes.
#[derive(Serialize)]
pub struct EmbedEstimate {
    /// Files that would be embedded, and the pages inside them.
    pub files: u32,
    pub pages: u32,
    /// Files pdfium could not open to measure; still counted in `files`.
    pub unreadable: u32,
    /// Billed (capped) pixels and the tokens they pace against.
    pub pixels: u64,
    pub tokens: u64,
    /// How many HTTP requests, at the ceiling in force now.
    pub requests: u32,
    pub kinds: Vec<Bucket>,

    // ── Against the account ──────────────────────────────────────────────────
    /// Of `pixels`, how many fall past the free grant and are therefore billed.
    pub billable_pixels: u64,
    pub cost_usd: f64,
    /// Free pixels left before this run, and whether the run fits in them.
    pub free_pixels_left: u64,
    /// Where the run would stop, in pages, if the spend guard caught it first.
    /// `None` when the guard is off or the run fits inside it.
    pub stops_after_pages: Option<u32>,

    // ── Against the clock ────────────────────────────────────────────────────
    /// Seconds at the learned tier, and at tier 1 — what a payment method buys.
    pub seconds: f64,
    pub seconds_tier1: f64,
    pub tier_rpm: f64,
    pub tier_tpm: f64,
    pub tier_free: bool,
    /// `ledger::TierSource`: how much the two numbers above are worth.
    pub tier_source: &'static str,
}

/// Measure the backlog. `base` is the app data dir. Opens every outstanding
/// PDF (page boxes only), so callers run it off the UI thread.
pub async fn estimate(db_file: &Path, base: &Path) -> Result<EmbedEstimate, String> {
    let backlog = backlog(db_file).await?;
    let usage = UsageLedger::shared().snapshot();
    let tier = usage.tier;
    let ceiling = batch::MAX_TOKENS_PER_REQUEST.min(tier.tpm.max(1.0) as u64);

    let mut out = EmbedEstimate {
        files: backlog.len() as u32,
        pages: 0,
        unreadable: 0,
        pixels: 0,
        tokens: 0,
        requests: 0,
        kinds: Vec::new(),
        billable_pixels: 0,
        cost_usd: 0.0,
        free_pixels_left: ledger::FREE_PIXELS.saturating_sub(usage.pixels),
        stops_after_pages: None,
        seconds: 0.0,
        seconds_tier1: 0.0,
        tier_rpm: tier.rpm,
        tier_tpm: tier.tpm,
        tier_free: tier.is_free(),
        tier_source: tier.source.as_str(),
    };

    let mut kinds: Vec<Bucket> = Vec::new();
    // Per-document page costs, so the tier-1 comparison re-plans without
    // reopening every PDF.
    let mut per_file: Vec<Vec<u64>> = Vec::with_capacity(backlog.len());
    // Running pixel total, so the guard's cut-off is reported as a page.
    let mut running = usage.pixels;
    let budget = usage.budget();

    for file in &backlog {
        let Ok(sizes) = raster::page_sizes(&base.join(&file.pdf)) else {
            out.unreadable += 1;
            continue;
        };

        let mut costs = Vec::with_capacity(sizes.len());
        for (width, height) in &sizes {
            costs.push(batch::tokens_for(*width, *height));

            let billed = (*width as u64 * *height as u64).min(batch::BILLED_PIXEL_CAP);
            out.pixels += billed;
            out.pages += 1;
            if out.stops_after_pages.is_none()
                && budget.is_some_and(|ceiling| running + billed > ceiling)
            {
                // The page *before* this one is the last that fits.
                out.stops_after_pages = Some(out.pages - 1);
            }
            running += billed;
        }

        out.tokens += costs.iter().sum::<u64>();
        out.requests += batch::plan(&costs, batch::MAX_INPUTS_PER_REQUEST, ceiling).len() as u32;
        per_file.push(costs);

        bump(&mut kinds, &file.kind, 1, sizes.len() as u32);
    }

    kinds.sort_by(|a, b| b.pages.cmp(&a.pages));
    out.kinds = kinds;

    out.billable_pixels = out.pixels.saturating_sub(out.free_pixels_left);
    out.cost_usd =
        out.billable_pixels as f64 / 1_000_000_000.0 * ledger::USD_PER_BILLION_PIXELS;
    out.seconds = seconds_for(out.tokens, out.requests, tier.tpm, tier.rpm);
    // Tier 1 packs more pages per request, so its request count is its own.
    let tier1_ceiling = batch::MAX_TOKENS_PER_REQUEST.min(ledger::TIER1_TPM as u64);
    let tier1_requests: u32 = per_file
        .iter()
        .map(|costs| batch::plan(costs, batch::MAX_INPUTS_PER_REQUEST, tier1_ceiling).len() as u32)
        .sum();
    out.seconds_tier1 =
        seconds_for(out.tokens, tier1_requests, ledger::TIER1_TPM, ledger::TIER1_RPM);
    Ok(out)
}

fn bump(buckets: &mut Vec<Bucket>, label: &str, files: u32, pages: u32) {
    match buckets.iter_mut().find(|bucket| bucket.label == label) {
        Some(bucket) => {
            bucket.files += files;
            bucket.pages += pages;
        }
        None => buckets.push(Bucket { label: label.to_string(), files, pages }),
    }
}

/// Wall clock, from whichever ceiling binds. TPM usually does, but tiny pages
/// on the free programme run out of requests first.
fn seconds_for(tokens: u64, requests: u32, tpm: f64, rpm: f64) -> f64 {
    let by_tokens = tokens as f64 / tpm.max(1.0) * 60.0;
    let by_requests = requests as f64 / rpm.max(1.0) * 60.0;
    by_tokens.max(by_requests)
}

// ── The backlog ──────────────────────────────────────────────────────────────

struct Outstanding {
    /// Relative to the app data dir, resolved to an Office document's PDF.
    pdf: PathBuf,
    /// Lower-cased extension; what the breakdown groups by.
    kind: String,
}

/// Parsed, PDF-backed files with no usable vectors in the current space.
///
/// Must match `getUnembeddedPdfs` in `app/src/lib/retrieval.ts` (duplicated
/// because each side queries its own pool). Both count current-space page
/// vectors, since `files.embed_status` does not record which space set it.
async fn backlog(db_file: &Path) -> Result<Vec<Outstanding>, String> {
    let db = crate::store::pool(db_file).await?;
    let sql = format!(
        r#"SELECT f.relative_path, lower(f.file_type) AS kind FROM files f
           WHERE lower(f.file_type) IN {}
             AND f.parse_status = 'quality'
             AND (SELECT COUNT(*) FROM pages p
                   WHERE p.file_id = f.id AND p.embedding IS NOT NULL
                     AND p.embed_model = ?1 AND p.embed_dim = ?2)
                 < max((SELECT COUNT(*) FROM pages p2 WHERE p2.file_id = f.id), 1)
           ORDER BY f.relative_path ASC"#,
        crate::paths::pdf_backed_sql_list()
    );
    let rows = sqlx::query(&sql)
    .bind(EMBED_MODEL)
    .bind(EMBED_DIM as i64)
    .fetch_all(&db)
    .await
    .map_err(|e| e.to_string())?;
    db.close().await;

    Ok(rows
        .iter()
        .filter_map(|row| {
            let relative: String = row.try_get("relative_path").ok()?;
            let kind: String = row.try_get("kind").unwrap_or_default();
            Some(Outstanding { pdf: crate::paths::doc_pdf_rel(&relative)?.into(), kind })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On the free programme TPM binds, not RPM.
    #[test]
    fn the_free_tier_is_governed_by_tokens_not_requests() {
        // 2,980 pages at the 2M-pixel cap.
        let tokens = 2_980 * batch::tokens_for(2339, 1653);
        // Two pages a request at a 10,000-token ceiling.
        let requests = 1_490;
        let free = seconds_for(tokens, requests, ledger::FREE_TPM, ledger::FREE_RPM);
        assert!(
            free > 60.0 * 60.0 * 17.0 && free < 60.0 * 60.0 * 19.0,
            "a free-tier re-index of this library is ~18 hours, got {free}s"
        );
        assert!(free > requests as f64 / ledger::FREE_RPM * 60.0, "TPM must be the binding limit");
    }

    #[test]
    fn tier_one_turns_the_same_run_into_minutes() {
        let tokens = 2_980 * batch::tokens_for(2339, 1653);
        let paid = seconds_for(tokens, 34, ledger::TIER1_TPM, ledger::TIER1_RPM);
        assert!(paid < 10.0 * 60.0, "tier 1 is single-digit minutes, got {paid}s");
    }

    /// The banner's claim about money, checked against the constants.
    #[test]
    fn this_library_fits_inside_the_free_grant() {
        let pixels = 2_980u64 * batch::BILLED_PIXEL_CAP;
        assert!(pixels < ledger::FREE_PIXELS / 10, "the grant is not the constraint here");
    }
}
