//! What the outstanding index run will cost, before it is started.
//!
//! An index run on a Voyage account with no payment method is **most of a day**
//! for this library, and until this module existed the settings page said only
//! "this can take hours". That is the difference between a number somebody can
//! plan around and a warning they learn to ignore — so the estimate is
//! computed, from the same arithmetic the run itself bills against, rather than
//! phrased.
//!
//! Three facts make an exact answer possible without sending anything:
//!
//! * **Voyage bills a page image by its pixels**, $0.60 per billion past the
//!   free grant, capped at 2,000,000 pixels an image
//!   (`ledger::USD_PER_BILLION_PIXELS`, `batch::BILLED_PIXEL_CAP`). Pixels come
//!   from the page box, which pdfium reads without rasterising anything
//!   (`raster::page_sizes`).
//! * **The packing is deterministic.** `batch::plan` is the same function the
//!   run uses, at the same ceiling, so the request count here is the request
//!   count that will happen.
//! * **The pace is the tier**, which the ledger has already learned. Tokens
//!   over TPM and requests over RPM, whichever is slower — on the free
//!   programme that is TPM by a factor of two, and on tier 1 it is TPM by a
//!   factor of a hundred.
//!
//! What this module must **never** do is quietly turn into a limiter. It
//! predicts; `ledger` decides. The one number they share is the spend guard,
//! and this module only reports where the run would land against it.

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
    /// Files in the backlog pdfium could not open to measure. They are still in
    /// `files`; the run will fail them one at a time, and an estimate that
    /// silently dropped them would under-count the work.
    pub unreadable: u32,
    /// Billed pixels and the tokens they pace against — `billed`, so a page
    /// over the 2M cap counts as 2M, exactly as Voyage bills it.
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
    /// Seconds at the tier the ledger has learned, and at tier 1 — the second
    /// number is what a payment method buys, and it is the only reason to
    /// mention a payment method at all.
    pub seconds: f64,
    pub seconds_tier1: f64,
    pub tier_rpm: f64,
    pub tier_tpm: f64,
    pub tier_free: bool,
    /// `assumed` / `observed` / `stated` — how much the two numbers above are
    /// worth. An `assumed` tier has never been tested against the API.
    pub tier_source: &'static str,
}

/// Measure the backlog.
///
/// `base` is the app data dir; paths in `files.relative_path` hang off it.
/// Blocking: it opens every outstanding PDF with pdfium. That is page boxes
/// only — no rasterising, no decompression of content streams — but it is
/// still file I/O over a whole library, so callers run it off the UI thread.
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
    // Every page's token cost, kept per document. The library is read from disk
    // **once**: the tier-1 comparison re-plans these same numbers at the other
    // ceiling rather than opening 166 PDFs a second time.
    let mut per_file: Vec<Vec<u64>> = Vec::with_capacity(backlog.len());
    // Pixels spent, page by page, so the spend guard's cut-off can be reported
    // as a page number rather than as a fraction.
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

            // Billed, not raw: Voyage downscales anything over the cap before
            // it charges, so a 200-DPI page costs the cap however big it is.
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
    // Tier 1 packs far more pages into one request, so its request count is its
    // own — reusing the current tier's would flatten the comparison that is the
    // only reason to mention a payment method at all.
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

/// Wall clock, from whichever ceiling binds.
///
/// Both, not either: TPM governs at every tier Voyage runs today, but a library
/// of tiny pages on the free programme would run out of *requests* first, and a
/// model that only knew about tokens would promise a run four times faster than
/// it is.
fn seconds_for(tokens: u64, requests: u32, tpm: f64, rpm: f64) -> f64 {
    let by_tokens = tokens as f64 / tpm.max(1.0) * 60.0;
    let by_requests = requests as f64 / rpm.max(1.0) * 60.0;
    by_tokens.max(by_requests)
}

// ── The backlog ──────────────────────────────────────────────────────────────

struct Outstanding {
    /// Relative to the app data dir, and already resolved to the PDF sibling an
    /// Office document gets.
    pdf: PathBuf,
    /// The extension as the library recorded it, lower-cased — what the
    /// breakdown groups by.
    kind: String,
}

/// Parsed, PDF-backed files with no usable vectors in the current space.
///
/// **The same predicate as `getUnembeddedPdfs` in
/// `app/src/lib/retrieval.ts`**, and it has to stay that way: an estimate over
/// a different set of files than the run walks is worse than no estimate. It is
/// duplicated rather than shared because one side is SQL over the WebView's
/// pool and the other is SQL over Rust's, and neither can call the other.
/// Both count *current-space page vectors* rather than reading
/// `files.embed_status`, for the reason recorded there: that column is a sticky
/// flag with no memory of which space set it.
async fn backlog(db_file: &Path) -> Result<Vec<Outstanding>, String> {
    let db = crate::store::pool(db_file).await?;
    let rows = sqlx::query(
        r#"SELECT f.relative_path, lower(f.file_type) AS kind FROM files f
           WHERE lower(f.file_type) IN ('pdf', 'pptx', 'docx', 'ppt', 'doc')
             AND f.parse_status = 'quality'
             AND (SELECT COUNT(*) FROM pages p
                   WHERE p.file_id = f.id AND p.embedding IS NOT NULL
                     AND p.embed_model = ?1 AND p.embed_dim = ?2)
                 < max((SELECT COUNT(*) FROM pages p2 WHERE p2.file_id = f.id), 1)
           ORDER BY f.relative_path ASC"#,
    )
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

    /// The whole reason the banner can promise a number: the free programme's
    /// 10,000 TPM is what makes this library a day's work, and it is TPM that
    /// binds, not RPM.
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

    /// And the number that justifies telling anyone about a payment method.
    #[test]
    fn tier_one_turns_the_same_run_into_minutes() {
        let tokens = 2_980 * batch::tokens_for(2339, 1653);
        let paid = seconds_for(tokens, 34, ledger::TIER1_TPM, ledger::TIER1_RPM);
        assert!(paid < 10.0 * 60.0, "tier 1 is single-digit minutes, got {paid}s");
    }

    /// The claim the banner makes about money, checked against the constants
    /// rather than against a sentence someone wrote once.
    #[test]
    fn this_library_fits_inside_the_free_grant() {
        let pixels = 2_980u64 * batch::BILLED_PIXEL_CAP;
        assert!(pixels < ledger::FREE_PIXELS / 10, "the grant is not the constraint here");
    }
}



