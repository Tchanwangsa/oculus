use std::ops::Range;

use super::super::windows::{frames, samples, Windows, FRAME_SAMPLES};

/// Speech-loud audio, `frames` frames long, every frame's energy a little
/// different so no two tie.
fn loud(frames: usize) -> Vec<f32> {
    let n = (frames - 1) * FRAME_SAMPLES + 400;
    (0..n)
        .map(|i| (0.5 + 0.3 * (i as f32 / 997.0).sin()) * (i as f32 * 0.37).sin())
        .collect()
}

fn hush(pcm: &mut [f32], range: Range<usize>) {
    pcm[range].iter_mut().for_each(|s| *s *= 0.001);
}

const SMALL: Windows = Windows {
    single_pass: 300,
    target: 200,
    search: 40,
    context: 25,
};

/// Every frame is in exactly one window, in order, none empty.
fn assert_covers(plan: &[Range<usize>], total: usize) {
    assert_eq!(plan.first().unwrap().start, 0);
    assert_eq!(plan.last().unwrap().end, total);
    for pair in plan.windows(2) {
        assert_eq!(pair[0].end, pair[1].start, "{plan:?}");
    }
    assert!(plan.iter().all(|w| w.start < w.end), "{plan:?}");
}

#[test]
fn the_model_emits_a_frame_per_stride_whose_receptive_field_fits() {
    assert_eq!(frames(0), 0);
    assert_eq!(frames(399), 0);
    assert_eq!(frames(400), 1);
    assert_eq!(frames(719), 1);
    assert_eq!(frames(720), 2);
    assert_eq!(frames(16_000), 49);
    // A run's samples hold exactly its frames, and start on its first.
    for run in [0..1, 0..49, 7..300, 1234..5678] {
        let s = samples(&run);
        assert_eq!(s.start, run.start * FRAME_SAMPLES);
        assert_eq!(frames(s.len()), run.len(), "{run:?}");
    }
}

#[test]
fn a_clip_that_fits_runs_whole() {
    let pcm = loud(300);
    assert_eq!(SMALL.plan(&pcm), vec![0..300]);
    assert_eq!(SMALL.plan(&[]), vec![0..0]);
}

#[test]
fn a_cut_lands_on_the_quietest_frame_near_its_target() {
    // 500 frames: three windows, cuts aimed at 166 and 333.
    let mut pcm = loud(500);
    // A quiet frame at 180 (within 40 of 166), and a louder half-hushed
    // stretch straddling 150-151 that is not frame-aligned.
    hush(&mut pcm, 180 * FRAME_SAMPLES..181 * FRAME_SAMPLES);
    let mut straddle = pcm.clone();
    hush(
        &mut straddle,
        150 * FRAME_SAMPLES + 160..151 * FRAME_SAMPLES + 160,
    );
    let plan = SMALL.plan(&straddle);
    assert_eq!(plan[0], 0..180, "{plan:?}");
    assert_covers(&plan, 500);
    // A quiet frame out of reach (at 100, 66 from the target) is not taken.
    let mut far = loud(500);
    hush(&mut far, 100 * FRAME_SAMPLES..101 * FRAME_SAMPLES);
    let cut = SMALL.plan(&far)[0].end;
    assert!((126..=206).contains(&cut), "{cut}");
}

#[test]
fn ties_go_to_the_frame_nearest_the_target() {
    let plan = SMALL.plan(&vec![0.0; (999 * FRAME_SAMPLES) + 400]);
    // 1000 frames in five even windows.
    assert_eq!(plan, vec![0..200, 200..400, 400..600, 600..800, 800..1000]);
}

#[test]
fn no_window_is_short_and_every_frame_is_covered_once() {
    for total in (301..2000).step_by(37) {
        let mut pcm = loud(total);
        // Quiet frames right where a short window would be tempting: near
        // the start and the end.
        hush(&mut pcm, 3 * FRAME_SAMPLES..4 * FRAME_SAMPLES);
        let end = (total - 4) * FRAME_SAMPLES;
        hush(&mut pcm, end..end + FRAME_SAMPLES);
        let plan = SMALL.plan(&pcm);
        assert_covers(&plan, total);
        assert_eq!(plan.len(), total.div_ceil(SMALL.target));
        for w in &plan {
            assert!(w.len() >= SMALL.target / 2, "{total}: {plan:?}");
            assert!(
                w.len() <= SMALL.target + 2 * SMALL.search,
                "{total}: {plan:?}"
            );
        }
    }
}

#[test]
fn a_clip_just_over_one_pass_splits_evenly_instead_of_leaving_a_remainder() {
    let plan = SMALL.plan(&vec![0.0; 300 * FRAME_SAMPLES + 400]);
    assert_eq!(plan, vec![0..150, 150..301]);
}

#[test]
fn context_is_clamped_to_the_clip() {
    assert_eq!(SMALL.run(&(0..200), 1000), 0..225);
    assert_eq!(SMALL.run(&(200..400), 1000), 175..425);
    assert_eq!(SMALL.run(&(800..1000), 1000), 775..1000);
}

#[test]
fn the_default_never_runs_the_model_on_more_than_one_pass() {
    let w = Windows::DEFAULT;
    assert!(w.target + 2 * w.search + 2 * w.context <= w.single_pass);
}
