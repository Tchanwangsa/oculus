use super::super::phrases;
use super::spoken;

#[test]
fn a_phrase_shows_just_before_its_first_word_and_hands_over_to_the_next() {
    // Two sentences 0.3 s apart: no gap on screen between them.
    let w = spoken("First idea. Second idea.", 1.0, 0.4, &[(1, 0.3)]);
    let p = phrases(&w, &[], 42, 2);
    assert_eq!(p.len(), 2);
    assert!((p[0].show - 0.85).abs() < 1e-9, "{p:?}");
    assert!((p[0].hide - p[1].show).abs() < 1e-9, "{p:?}");
    assert!((p[1].show - (w[2].start - 0.15)).abs() < 1e-9);
    assert!((p[1].hide - (w[3].end + 0.4)).abs() < 1e-9);
}

#[test]
fn a_long_pause_clears_the_caption() {
    let w = spoken("Before the pause. After it.", 0.0, 0.3, &[(2, 1.5)]);
    let p = phrases(&w, &[], 42, 2);
    // The first hides 0.4 s after its last word, well before the second.
    assert!((p[0].hide - (w[2].end + 0.4)).abs() < 1e-9, "{p:?}");
    assert!(p[0].hide < p[1].show);
}
