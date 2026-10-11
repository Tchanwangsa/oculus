//! Scoring and thinning the loud frames into candidate boundaries.

use super::{
    Candidate, COLLAPSE_SECS, DIFF_THRESHOLD, MIN_SPACING, PAUSE_BONUS, PAUSE_SECS, PAUSE_WINDOW,
};

/// Raw frame diffs and transcript gaps to a thinned, time-ordered set of
/// boundary candidates: keep loud frames, collapse runs onto their first
/// second, score with a pause bonus, then thin to [`MIN_SPACING`] strongest
/// first (so the important one of two close boundaries survives).
pub fn candidates(diffs: &[(u32, f32)], gaps: &[(u32, f32)], duration_secs: u32) -> Vec<Candidate> {
    // A collapsed run keeps its peak magnitude.
    let mut collapsed: Vec<(u32, f32)> = Vec::new();
    for &(second, diff) in diffs {
        if diff < DIFF_THRESHOLD {
            continue;
        }
        if duration_secs > 0 && second >= duration_secs {
            continue;
        }
        match collapsed.last_mut() {
            Some(last) if second - last.0 <= COLLAPSE_SECS => {
                if diff > last.1 {
                    last.1 = diff;
                }
            }
            _ => collapsed.push((second, diff)),
        }
    }

    let pauses: Vec<u32> = gaps
        .iter()
        .filter(|(_, gap)| *gap >= PAUSE_SECS)
        .map(|(start, _)| *start)
        .collect();

    let mut scored: Vec<Candidate> = collapsed
        .into_iter()
        .map(|(seconds, diff)| {
            let pause = pauses.iter().any(|p| p.abs_diff(seconds) <= PAUSE_WINDOW);
            Candidate {
                seconds,
                score: diff + if pause { PAUSE_BONUS } else { 0.0 },
                diff,
                pause,
            }
        })
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.seconds.cmp(&b.seconds))
    });
    let mut kept: Vec<Candidate> = Vec::new();
    for candidate in scored {
        if kept
            .iter()
            .all(|k| k.seconds.abs_diff(candidate.seconds) >= MIN_SPACING)
        {
            kept.push(candidate);
        }
    }
    kept.sort_by_key(|c| c.seconds);
    kept
}
