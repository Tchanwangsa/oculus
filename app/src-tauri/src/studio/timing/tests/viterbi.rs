use super::super::viterbi::force_align;
use super::{emissions, frames, words};
use crate::studio::NarrationError;

fn spans(frame_str: &str, ws: &[&str]) -> Result<Vec<(usize, usize)>, NarrationError> {
    Ok(force_align(&emissions(&frames(frame_str)), &words(ws))?
        .into_iter()
        .map(|s| (s.start_frame, s.end_frame))
        .collect())
}

#[test]
fn the_path_follows_the_letters_and_spans_cover_held_frames() {
    assert_eq!(spans("_ C C A _ T T _", &["CAT"]).unwrap(), vec![(1, 6)]);
    assert_eq!(
        spans("_ C A T _ | _ D O G _", &["CAT", "DOG"]).unwrap(),
        vec![(1, 3), (7, 9)]
    );
}

#[test]
fn a_repeated_letter_needs_a_blank_between() {
    // S E _ E: the blank separates the two Es.
    assert_eq!(spans("S E _ E", &["SEE"]).unwrap(), vec![(0, 3)]);
    // S E E has frames for three labels but no room for the blank.
    assert!(matches!(
        spans("S E E", &["SEE"]),
        Err(NarrationError::Unalignable(_))
    ));
}

#[test]
fn different_labels_may_skip_the_blank_between_them() {
    // A | B in exactly three frames is only possible by skipping both blanks.
    assert_eq!(spans("A | B", &["A", "B"]).unwrap(), vec![(0, 0), (2, 2)]);
    // A L _ L: A skips to L, the Ls need their blank.
    assert_eq!(spans("A L _ L", &["ALL"]).unwrap(), vec![(0, 3)]);
}

#[test]
fn too_few_frames_for_the_letters_is_unalignable() {
    let error = spans("C A", &["CAT"]).unwrap_err();
    assert_eq!(
        error,
        NarrationError::Unalignable("2 frames of audio cannot hold 3 letters".into())
    );
}

#[test]
fn confidence_is_the_mean_log_prob_of_the_letter_frames() {
    // Every letter frame of CAT carries its letter at ln 0.6.
    let out = force_align(&emissions(&frames("_ C A T _")), &words(&["CAT"])).unwrap();
    assert!((out[0].mean_logp - 0.6f64.ln()).abs() < 1e-6);
    // Forcing DOG through frames that say CAT drives it to ln 0.001.
    let forced = force_align(&emissions(&frames("_ C A T _")), &words(&["DOG"])).unwrap();
    assert!((forced[0].mean_logp - 0.001f64.ln()).abs() < 1e-4);
}

#[test]
fn the_path_ends_on_the_last_letter_or_the_trailing_blank() {
    assert_eq!(spans("A T T", &["AT"]).unwrap(), vec![(0, 2)]);
    assert_eq!(spans("A T _ _", &["AT"]).unwrap(), vec![(0, 1)]);
}
