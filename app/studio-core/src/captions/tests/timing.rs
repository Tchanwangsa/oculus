use super::super::{Moment, moment, phrases};
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
    let gap_middle = w[2].end + 0.75;
    assert_eq!(moment(&p, &w, gap_middle), None);
    assert!(p[0].hide < p[1].show);
}

#[test]
fn the_current_word_is_the_one_being_said_and_lingers_briefly() {
    let w = spoken("one two three", 0.0, 0.5, &[(0, 0.2)]);
    let p = phrases(&w, &[], 42, 2);
    let at = |t| moment(&p, &w, t).unwrap();
    assert_eq!(
        at(0.0),
        Moment {
            phrase: 0,
            current: Some(0),
            said: 1
        }
    );
    // In the 0.2 s gap after "one", it stays lit.
    assert_eq!(at(0.6).current, Some(0));
    assert_eq!(at(0.75).current, Some(1));
    // Shown before the first word: nothing lit, nothing said.
    let w = spoken("late start", 1.0, 0.5, &[]);
    let p = phrases(&w, &[], 42, 2);
    let m = moment(&p, &w, 0.9).unwrap();
    assert_eq!((m.current, m.said), (None, 0));
    // After the last word, past the linger: all said, none lit.
    let m = moment(&p, &w, 2.0 + 0.35).unwrap();
    assert_eq!((m.current, m.said), (None, 2));
}
