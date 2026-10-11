use super::super::gaps::{unscripted, Heard};
use super::super::viterbi::force_align;
use super::{emissions, frames, words};

#[test]
fn a_spoken_word_missing_from_the_script_is_heard_in_the_gap() {
    // The clip says "A THICK B"; the script says "A B".
    let em = emissions(&frames("A _ T H I C K _ | _ B _"));
    let spans = force_align(&em, &words(&["A", "B"])).unwrap();
    assert_eq!(
        (
            spans[0].start_frame,
            spans[0].end_frame,
            spans[1].start_frame
        ),
        (0, 0, 10)
    );
    assert_eq!(
        unscripted(&em, &spans),
        vec![Heard {
            next: 1,
            start_frame: 1,
            end_frame: 10,
            letter_frames: 5,
            heard: "THICK".into(),
        }]
    );
}

#[test]
fn speech_before_the_first_and_after_the_last_word_counts_too() {
    let em = emissions(&frames("U M _ A _ | _ B _ S O"));
    let spans = force_align(&em, &words(&["A", "B"])).unwrap();
    let heard = unscripted(&em, &spans);
    assert_eq!(heard.len(), 2);
    assert_eq!((heard[0].next, heard[0].heard.as_str()), (0, "UM"));
    assert_eq!((heard[1].next, heard[1].heard.as_str()), (2, "SO"));
}

#[test]
fn separators_and_blanks_are_not_speech_and_repeats_collapse() {
    let em = emissions(&frames("A _ | _ O O _ O | X _ B"));
    let spans = force_align(&em, &words(&["A", "B"])).unwrap();
    let heard = unscripted(&em, &spans);
    assert_eq!(heard.len(), 1);
    assert_eq!(heard[0].letter_frames, 4);
    // O O collapses; the blank between lets the next O count again; `|`
    // splits words.
    assert_eq!(heard[0].heard, "OO X");
}

#[test]
fn a_clean_path_has_no_gaps() {
    let em = emissions(&frames("_ C A T _ | _ D O G _"));
    let spans = force_align(&em, &words(&["CAT", "DOG"])).unwrap();
    assert!(unscripted(&em, &spans).is_empty());
}
