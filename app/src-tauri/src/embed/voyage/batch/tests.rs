use crate::embed::raster::{RasterError, RenderedPage};
use crate::embed::{EmbedError, EmbedPage};
use std::sync::Arc;

use super::*;
use crate::embed::EMBED_DIM;
use crate::providers::ratelimit::Permits;

/// A landscape A4 slide at `RENDER_DPI`.
const A4_LANDSCAPE: (u32, u32) = (2339, 1653);

#[test]
fn a_full_dpi_page_costs_what_was_measured() {
    let (width, height) = A4_LANDSCAPE;
    assert_eq!(u64::from(width) * u64::from(height), 3_866_367);
    // Capped at 2M billed pixels, rounded up.
    assert_eq!(tokens_for(width, height), 3_572);
    assert_eq!(BILLED_PIXEL_CAP.div_ceil(PIXELS_PER_TOKEN), 3_572);
    assert_eq!(tokens_for(842, 595), 895); // 72 dpi
    assert_eq!(tokens_for(1170, 827), 1_728); // 100 dpi
    assert_eq!(tokens_for(1754, 1240), 3_572); // 150 dpi is already at the cap
    assert_eq!(tokens_for(1, 1), 1);
    assert_eq!(tokens_for(560, 1), 1);
    assert_eq!(tokens_for(561, 1), 2);
    // The cap is on billing, not on the raster.
    let page = RenderedPage {
        page_no: 1,
        width,
        height,
        png: Vec::new(),
    };
    assert_eq!(raw_pixels(&page), 3_866_367);
    assert_eq!(billed_pixels(&page), BILLED_PIXEL_CAP);
}

#[test]
fn a_request_holds_eighty_nine_pages_not_three_hundred_and_twenty() {
    let costs = vec![tokens_for(A4_LANDSCAPE.0, A4_LANDSCAPE.1); 200];
    let requests = plan(&costs, MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST);
    assert_eq!(requests[0].len(), 89);
    assert!(89 * 3_572 <= MAX_TOKENS_PER_REQUEST as usize);
    assert!(90 * 3_572 > MAX_TOKENS_PER_REQUEST as usize);
    let flat: Vec<usize> = requests.iter().flatten().copied().collect();
    assert_eq!(flat, (0..200).collect::<Vec<_>>());
}

#[test]
fn the_batch_is_computed_from_real_pixels_never_a_page_count() {
    // A deck whose second half is cheaper.
    let mut costs = vec![tokens_for(2339, 1653); 100];
    costs.extend(vec![tokens_for(827, 1170); 100]);
    let requests = plan(&costs, MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST);
    assert!(requests.len() >= 2);
    assert!(
        requests.iter().all(
            |request| request.iter().map(|index| costs[*index]).sum::<u64>()
                <= MAX_TOKENS_PER_REQUEST
        ),
        "a request went over the token ceiling: {requests:?}"
    );
    assert_eq!(requests[0].len(), 89);
    assert!(requests.last().unwrap().len() > 89, "{:?}", requests.last());
}

#[test]
fn the_input_ceiling_binds_when_the_token_one_does_not() {
    let costs = vec![1u64; 2_500];
    let requests = plan(&costs, MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST);
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].len(), MAX_INPUTS_PER_REQUEST);
    assert_eq!(requests[2].len(), 500);
}

#[test]
fn one_page_larger_than_a_whole_request_still_travels_alone() {
    let costs = vec![5, MAX_TOKENS_PER_REQUEST + 1, 5];
    let requests = plan(&costs, MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST);
    assert_eq!(requests, vec![vec![0], vec![1], vec![2]]);
}

#[test]
fn a_free_tier_ceiling_shrinks_the_plan_instead_of_repeating_it() {
    // Over the account's TPM the plan must get smaller, not slower.
    let costs = vec![tokens_for(A4_LANDSCAPE.0, A4_LANDSCAPE.1); 8];
    let free_ceiling = 10_000;

    let optimistic = plan(&costs, MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST);
    assert_eq!(optimistic.len(), 1, "8 pages fit in one tier-1 request");
    assert!(
        optimistic[0].iter().map(|i| costs[*i]).sum::<u64>() > free_ceiling,
        "this is the request that can never be accepted"
    );

    let shrunk = plan(&costs, MAX_INPUTS_PER_REQUEST, free_ceiling);
    assert_eq!(shrunk.len(), 4, "2 pages a request at 3,572 tokens each");
    assert!(
        shrunk
            .iter()
            .all(|r| r.iter().map(|i| costs[*i]).sum::<u64>() <= free_ceiling),
        "{shrunk:?}"
    );
    assert_eq!(shrunk.iter().flatten().count(), 8);
}

#[test]
fn one_page_always_fits_inside_the_smallest_programme_voyage_runs() {
    // Shrinking always terminates: the billing cap keeps any page under
    // the free tier's TPM.
    assert!((BILLED_PIXEL_CAP.div_ceil(PIXELS_PER_TOKEN) as f64) < super::super::ledger::FREE_TPM);
}

#[test]
fn an_empty_document_plans_no_requests() {
    assert!(plan(&[], MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST).is_empty());
}

#[test]
fn an_oversized_page_is_this_documents_problem_and_nobody_elses() {
    let page = RenderedPage {
        page_no: 7,
        width: 5_000,
        height: 5_000,
        png: vec![0; 16],
    };
    let error = refuse_oversized(&page).unwrap_err();
    assert_eq!(error.kind(), "document");
    assert!(!error.latching(), "one huge page must not condemn the run");
    assert!(format!("{error:?}").contains("p7"), "{error:?}");

    // Refused on bytes at a legal pixel count.
    let heavy = RenderedPage {
        page_no: 1,
        width: 100,
        height: 100,
        png: vec![0; MAX_BYTES_PER_IMAGE as usize + 1],
    };
    assert!(refuse_oversized(&heavy).is_err());

    let ordinary = RenderedPage {
        page_no: 1,
        width: A4_LANDSCAPE.0,
        height: A4_LANDSCAPE.1,
        png: vec![0; 16],
    };
    assert_eq!(refuse_oversized(&ordinary).unwrap(), 3_572);
}

#[test]
fn the_raster_vocabulary_reconciles_into_the_seams() {
    let library = EmbedError::from(RasterError::Library("no dylib".into()));
    assert_eq!(library.kind(), "not_ready");
    assert!(library.retryable());

    for (raster, code) in [
        (RasterError::Unreadable("junk".into()), "unreadable-pdf"),
        (RasterError::Encrypted, "encrypted-pdf"),
        (RasterError::Empty, "empty-pdf"),
    ] {
        let error = EmbedError::from(raster);
        assert_eq!(error.kind(), "document");
        assert!(format!("{error:?}").contains(code), "{error:?}");
        assert!(!error.latching());
    }

    let page = EmbedError::from(RasterError::Page {
        page_no: 12,
        message: "internal pdfium detail".into(),
    });
    assert!(!format!("{page:?}").contains("pdfium"), "{page:?}");
    assert!(format!("{page:?}").contains("12"), "{page:?}");
}

#[test]
fn permits_bound_what_runs_at_once() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let permits = Arc::new(Permits::new(2));
    let running = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let permits = permits.clone();
        let running = running.clone();
        let peak = peak.clone();
        handles.push(std::thread::spawn(move || {
            permits.acquire();
            let now = running.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(30));
            running.fetch_sub(1, Ordering::SeqCst);
            permits.release();
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
    assert!(
        peak.load(Ordering::SeqCst) <= 2,
        "{}",
        peak.load(Ordering::SeqCst)
    );
}

#[test]
fn a_vector_of_the_wrong_width_is_caught_before_it_reaches_a_record() {
    assert!(EmbedPage::new(1, &vec![0.5; EMBED_DIM]).is_ok());
    assert!(EmbedPage::new(1, &vec![0.5; EMBED_DIM - 1]).is_err());
}
