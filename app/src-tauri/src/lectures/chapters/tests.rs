use super::agent::{outline, parse_chapters, prompt, validate, Chapter, Job};
use super::decode::{mean_abs_diff, read_frame};
use super::frames::{pick_offset, spread, sweep_orphans, thumbnail_pick};
use super::score::candidates;
use super::transcript::{cue_gaps, TranscriptCue};
use super::{Candidate, FRAME_BYTES};
use std::path::PathBuf;

/// A frame of one flat grey value — a held slide, in miniature.
fn flat(value: u8) -> Vec<u8> {
    vec![value; FRAME_BYTES]
}

/// Feed a sequence of frames through the same read-and-diff loop
/// `sample_diffs` runs, without an ffmpeg between.
fn diffs_of(frames: &[Vec<u8>]) -> Vec<(u32, f32)> {
    let bytes: Vec<u8> = frames.concat();
    let mut source = std::io::Cursor::new(bytes);
    let mut previous = vec![0u8; FRAME_BYTES];
    let mut current = vec![0u8; FRAME_BYTES];
    let mut out = Vec::new();
    let mut index = 0u32;
    while read_frame(&mut source, &mut current).unwrap() {
        if index > 0 {
            out.push((index, mean_abs_diff(&previous, &current)));
        }
        std::mem::swap(&mut previous, &mut current);
        index += 1;
    }
    out
}

#[test]
fn a_held_slide_is_silent_and_a_cut_is_loud() {
    let frames = vec![flat(40), flat(40), flat(41), flat(200), flat(200)];
    let diffs = diffs_of(&frames);
    assert_eq!(diffs.len(), 4, "one diff per frame after the first");
    assert_eq!(diffs[0], (1, 0.0));
    assert_eq!(diffs[1], (2, 1.0), "a one-level drift is not a change");
    assert_eq!(diffs[2], (3, 159.0), "the cut");
    assert_eq!(diffs[3], (4, 0.0));

    let found = candidates(&diffs, &[], 10);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].seconds, 3);
    assert!(!found[0].pause);
}

#[test]
fn a_trailing_partial_frame_is_dropped() {
    let mut bytes = flat(10);
    bytes.extend(flat(200));
    bytes.extend(vec![0u8; 17]); // ffmpeg cut off mid-write
    let mut source = std::io::Cursor::new(bytes);
    let mut frame = vec![0u8; FRAME_BYTES];
    assert!(read_frame(&mut source, &mut frame).unwrap());
    assert!(read_frame(&mut source, &mut frame).unwrap());
    assert!(!read_frame(&mut source, &mut frame).unwrap());
}

#[test]
fn a_run_of_loud_frames_collapses_onto_its_first() {
    // A dissolve: four consecutive loud frames, peaking in the middle.
    let diffs = vec![
        (100, 20.0),
        (101, 60.0),
        (102, 30.0),
        (103, 8.0),
        // Well clear of the run, and of MIN_SPACING.
        (400, 25.0),
    ];
    let found = candidates(&diffs, &[], 600);
    assert_eq!(
        found.iter().map(|c| c.seconds).collect::<Vec<_>>(),
        vec![100, 400]
    );
    assert_eq!(found[0].diff, 60.0, "the run keeps its peak magnitude");
}

#[test]
fn frames_below_the_threshold_never_become_candidates() {
    let diffs = vec![(10, 0.008), (200, 5.9), (400, 6.1)];
    let found = candidates(&diffs, &[], 600);
    assert_eq!(
        found.iter().map(|c| c.seconds).collect::<Vec<_>>(),
        vec![400]
    );
}

#[test]
fn thinning_keeps_the_strongest_of_a_cluster() {
    // Three changes inside 90 s, the middle one strongest, plus one far
    // enough away to survive on its own.
    let diffs = vec![(30, 10.0), (60, 90.0), (100, 40.0), (300, 12.0)];
    let found = candidates(&diffs, &[], 600);
    assert_eq!(
        found.iter().map(|c| c.seconds).collect::<Vec<_>>(),
        vec![60, 300],
        "greedy strongest-first, then back into play order"
    );
}

#[test]
fn thinning_measures_from_what_it_kept_not_from_the_last_candidate() {
    // 0 is strongest and keeps 80 out; 150 is 150 s from 0 and stays.
    let diffs = vec![(10, 99.0), (80, 50.0), (160, 40.0)];
    let found = candidates(&diffs, &[], 600);
    assert_eq!(
        found.iter().map(|c| c.seconds).collect::<Vec<_>>(),
        vec![10, 160]
    );
}

#[test]
fn a_nearby_pause_is_a_bonus_and_never_a_gate() {
    // Two equal changes; only the second has a silence beside it.
    let diffs = vec![(100, 20.0), (300, 20.0)];
    let gaps = vec![(60, 0.4), (295, 3.2), (400, 0.1)];
    let found = candidates(&diffs, &gaps, 600);
    assert_eq!(found.len(), 2, "the unsupported change survives");
    assert!(!found[0].pause);
    assert!(found[1].pause);
    assert_eq!(found[1].score, 23.0);
    assert_eq!(
        found[1].diff, 20.0,
        "the bonus does not touch the magnitude"
    );

    // A pause on its own is not a boundary.
    assert!(candidates(&[], &gaps, 600).is_empty());
}

#[test]
fn a_pause_outside_the_window_does_not_count() {
    let diffs = vec![(300, 20.0)];
    assert!(!candidates(&diffs, &[(291, 5.0)], 600)[0].pause);
    assert!(candidates(&diffs, &[(292, 5.0)], 600)[0].pause);
    assert!(candidates(&diffs, &[(308, 5.0)], 600)[0].pause);
    assert!(!candidates(&diffs, &[(309, 5.0)], 600)[0].pause);
}

#[test]
fn candidates_past_the_end_are_dropped() {
    let diffs = vec![(100, 30.0), (2519, 30.0)];
    let found = candidates(&diffs, &[], 2519);
    assert_eq!(
        found.iter().map(|c| c.seconds).collect::<Vec<_>>(),
        vec![100]
    );
}

#[test]
fn spread_separates_a_blank_frame_from_a_busy_one() {
    assert_eq!(spread(&flat(0)), 0.0, "a black frame has no spread at all");
    assert_eq!(spread(&flat(255)), 0.0, "and neither does a white one");
    // Half black, half white — the letterboxed slide these captures are.
    let mut split = flat(0);
    split[FRAME_BYTES / 2..].fill(255);
    assert!((spread(&split) - 127.5).abs() < 0.01);
}

#[test]
fn a_frame_grab_steps_over_a_blank_and_over_a_splash_screen() {
    // The cut is black, +2 and +6 are the AV splash, the slide is back by
    // +12. Probes arrive in GRAB_OFFSETS order: 2, 6, 12, 0.
    let probed = [(1388, 81.4), (1392, 81.4), (1398, 102.2), (1386, 0.0)];
    assert_eq!(pick_offset(&probed), Some(1398));

    // +2 is the splash; the first frame within tolerance of the best wins.
    let probed = [(1726, 81.4), (1730, 102.5), (1736, 102.5), (1724, 102.6)];
    assert_eq!(pick_offset(&probed), Some(1730));
}

#[test]
fn a_run_sweeps_the_grabs_it_will_not_rewrite() {
    let dir = crate::test_support::Scratch::new("sweep");
    std::fs::create_dir_all(dir.join("live")).unwrap();
    for name in ["50.jpg", "313.jpg", "1767.jpg", "notes.txt", "keyframe.jpg"] {
        std::fs::write(dir.join(name), b"x").unwrap();
    }
    std::fs::write(dir.join("live").join("900.jpg"), b"x").unwrap();

    sweep_orphans(&dir, &[313, 1767, 2550]);

    let left = |name: &str| dir.join(name).exists();
    assert!(
        !left("50.jpg"),
        "an orphan from a previous candidate set goes"
    );
    assert!(
        left("313.jpg") && left("1767.jpg"),
        "a frame this run rewrites stays"
    );
    assert!(left("notes.txt"), "only JPEGs are swept");
    assert!(
        left("keyframe.jpg"),
        "a name that is not a second was not written here"
    );
    assert!(left("live/900.jpg"), "another job's subfolder is untouched");
}

#[test]
fn a_frame_grab_prefers_the_earliest_good_frame() {
    let probed = [(114, 102.9), (118, 102.9), (124, 102.9), (112, 102.9)];
    assert_eq!(pick_offset(&probed), Some(114));

    // A sparse title slide is not passed over for a denser later one.
    let probed = [(22, 98.0), (26, 99.0), (32, 101.0), (20, 98.5)];
    assert_eq!(pick_offset(&probed), Some(22));
}

#[test]
fn a_frame_grab_with_nothing_to_go_on_still_picks_something() {
    // Every probe blank: the first offset still wins.
    let probed = [(1388, 0.0), (1392, 0.0), (1398, 0.0), (1386, 0.0)];
    assert_eq!(pick_offset(&probed), Some(1388));
    assert_eq!(pick_offset(&[]), None);
}

fn chapter(start: u32, title: &str) -> Chapter {
    Chapter {
        start_seconds: start,
        title: title.to_string(),
        summary: format!("What happens in {title}."),
    }
}

const REPLY: &str = r#"[
  {"start": 0, "title": "Qubits and superposition", "summary": "Sets up the state vector."},
  {"start": 742, "title": "Hadamard gates", "summary": "Builds the uniform superposition."}
]"#;

#[test]
fn a_bare_json_array_is_the_easy_case() {
    let parsed = parse_chapters(REPLY).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].start_seconds, 0);
    assert_eq!(parsed[1].title, "Hadamard gates");
}

#[test]
fn a_reply_wrapped_in_prose_still_parses() {
    let reply = format!(
        "I read the transcript around each candidate. Here are the chapters:\n\n{REPLY}\n\n\
         Let me know if you would like them merged differently."
    );
    let parsed = parse_chapters(&reply).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[1].start_seconds, 742);
}

#[test]
fn a_fenced_reply_still_parses() {
    let reply = format!("Done — six chapters.\n\n```json\n{REPLY}\n```\n");
    assert_eq!(parse_chapters(&reply).unwrap().len(), 2);
    // And an unlabelled fence, which is just as common.
    let reply = format!("```\n{REPLY}\n```");
    assert_eq!(parse_chapters(&reply).unwrap().len(), 2);
}

#[test]
fn an_object_around_the_array_is_accepted() {
    let reply = format!("{{\"chapters\": {REPLY}}}");
    assert_eq!(
        parse_chapters(&reply).unwrap()[0].title,
        "Qubits and superposition"
    );
}

#[test]
fn a_bracket_inside_a_summary_does_not_end_the_scan() {
    let reply = r#"Here you go:
    [{"start": 0, "title": "Resolution", "summary": "The rule [A ∨ B], [¬B ∨ C] ⊢ [A ∨ C]."}]
    That's it."#;
    let parsed = parse_chapters(reply).unwrap();
    assert_eq!(parsed.len(), 1);
    assert!(parsed[0].summary.ends_with("[A ∨ C]."));
}

#[test]
fn a_reply_with_no_chapters_in_it_is_refused() {
    let error = parse_chapters("I could not open the frames, sorry.").unwrap_err();
    assert!(error.contains("no chapter list"), "{error}");
    // Valid JSON that is not a chapter list is no better.
    assert!(parse_chapters("[]").is_err());
    assert!(parse_chapters(r#"{"ok": true}"#).is_err());
    // A truncated reply: the array never closes.
    assert!(parse_chapters(r#"[{"start": 0, "title": "Qubits","#).is_err());
}

#[test]
fn a_bracket_in_the_prose_does_not_hide_the_answer() {
    // An empty list, then a citation, then the real array.
    let reply = format!("I found no splash frames [] — see slide [3].\n\n{REPLY}");
    assert_eq!(parse_chapters(&reply).unwrap().len(), 2);
}

#[test]
fn a_float_second_and_a_synonym_for_start_are_tolerated() {
    let reply = r#"[{"start_seconds": 0.0, "title": "Opening", "summary": "Sets up."}]"#;
    assert_eq!(parse_chapters(reply).unwrap()[0].start_seconds, 0);
}

const BOUNDS: [u32; 5] = [0, 300, 700, 1200, 2000];

#[test]
fn a_well_formed_set_validates() {
    let set = vec![
        chapter(0, "Opening"),
        chapter(700, "Middle"),
        chapter(2000, "End"),
    ];
    assert!(validate(&set, &BOUNDS, 2400).is_ok());
}

#[test]
fn a_boundary_the_outline_never_printed_rejects_the_whole_set() {
    let set = vec![
        chapter(0, "Opening"),
        chapter(701, "Middle"),
        chapter(2000, "End"),
    ];
    let error = validate(&set, &BOUNDS, 2400).unwrap_err();
    assert_eq!(
        error,
        "chapter 2 (\"Middle\"): 701 is not one of the seconds in the outline"
    );
}

#[test]
fn a_transcript_cue_start_is_a_boundary_too() {
    let mut bounds = BOUNDS.to_vec();
    bounds.push(701);
    let set = vec![
        chapter(0, "Opening"),
        chapter(701, "Middle"),
        chapter(2000, "End"),
    ];
    assert!(validate(&set, &bounds, 2400).is_ok());
}

#[test]
fn chapters_out_of_order_reject_the_whole_set() {
    let set = vec![
        chapter(0, "Opening"),
        chapter(1200, "Middle"),
        chapter(700, "End"),
    ];
    let error = validate(&set, &BOUNDS, 2400).unwrap_err();
    assert!(
        error.starts_with("chapter 3 (\"End\"): starts at 700, which is not after"),
        "{error}"
    );
    // A repeat would make a zero-length chapter.
    let set = vec![chapter(0, "Opening"), chapter(700, "A"), chapter(700, "B")];
    assert!(validate(&set, &BOUNDS, 2400).is_err());
}

#[test]
fn a_first_chapter_that_does_not_start_at_zero_rejects_the_whole_set() {
    let set = vec![chapter(300, "Opening"), chapter(700, "Middle")];
    let error = validate(&set, &BOUNDS, 2400).unwrap_err();
    assert_eq!(
        error,
        "chapter 1 (\"Opening\"): the first chapter must start at 0, not 300"
    );
}

#[test]
fn thirteen_chapters_reject_the_whole_set() {
    let bounds: Vec<u32> = (0..13).map(|n| n * 300).collect();
    let set: Vec<Chapter> = bounds.iter().map(|s| chapter(*s, "Topic")).collect();
    assert_eq!(set.len(), 13);
    let error = validate(&set, &bounds, 9000).unwrap_err();
    assert!(
        error.starts_with("13 chapters is more than the 12 allowed"),
        "{error}"
    );
    // Twelve is the ceiling, not one short of it.
    assert!(validate(&set[..12], &bounds, 9000).is_ok());
}

#[test]
fn an_empty_or_gutted_set_is_refused() {
    assert!(validate(&[], &BOUNDS, 2400).is_err());
    let mut set = vec![chapter(0, "Opening")];
    set[0].summary.clear();
    assert!(validate(&set, &BOUNDS, 2400)
        .unwrap_err()
        .contains("needs a summary"));
    set[0].summary = "Sets up.".into();
    set[0].title.clear();
    assert!(validate(&set, &BOUNDS, 2400)
        .unwrap_err()
        .contains("needs a title"));
}

#[test]
fn a_chapter_past_the_end_of_the_recording_is_refused() {
    let set = vec![chapter(0, "Opening"), chapter(2000, "End")];
    assert!(validate(&set, &BOUNDS, 2000)
        .unwrap_err()
        .contains("past the end"));
    assert!(validate(&set, &BOUNDS, 2001).is_ok());
}

#[test]
fn the_prompt_names_the_outline_and_the_two_paths() {
    let job = Job {
        title: "Lecture 7: Grover",
        duration_secs: 2534,
        lecture_dir: "../lectures/abc-123",
        course_dir: Some("../courses/MULT20015_2026_SM2"),
        detected: 17,
        has_transcript: true,
    };
    let text = prompt(&job);
    assert!(text.contains("Lecture 7: Grover"));
    assert!(text.contains("00:42:14"), "the duration as a clock");
    assert!(
        text.contains("\n  ../lectures/abc-123/outline.md"),
        "indented under the folder"
    );
    assert!(
        text.contains("\n  ../lectures/abc-123/frames/<second>.jpg"),
        "indented under the folder"
    );
    assert!(text.contains("../courses/MULT20015_2026_SM2/"));
    assert!(text.contains("17 slide changes were found"));
    assert!(
        !text.contains("transcript.vtt"),
        "the outline replaced the raw VTT, it did not join it"
    );
    assert!(text.contains("never more than 12"));
    assert!(
        text.contains("connect your laptop"),
        "the AV splash warning"
    );
    assert!(!text.contains("no transcript"), "it has one");

    let silent = prompt(&Job {
        has_transcript: false,
        ..job
    });
    assert!(
        silent.contains("no transcript, so the outline is those markers"),
        "a lecture with no transcript should not be sent looking for words"
    );
}

#[test]
fn the_outline_merges_the_slide_changes_into_the_transcript_in_play_order() {
    let cues = vec![
        TranscriptCue {
            start: 1.5,
            end: 4.0,
            text: "Good morning.".into(),
        },
        TranscriptCue {
            start: 725.0,
            end: 728.0,
            text: "An equal superposition.".into(),
        },
    ];
    let changes = vec![
        Candidate {
            seconds: 723,
            score: 42.1,
            diff: 39.1,
            pause: true,
        },
        // Past the last spoken word: it must still reach the file.
        Candidate {
            seconds: 2400,
            score: 12.0,
            diff: 12.0,
            pause: false,
        },
    ];
    let text = outline("Lecture 7: Grover", &cues, &changes);
    let lines: Vec<&str> = text.lines().filter(|l| l.contains("00:")).collect();
    assert_eq!(
        lines,
        vec![
            "      1  00:00:01  Good morning.",
            "    723  00:12:03  --- slide change · score 42.1 · pause ---",
            "    725  00:12:05  An equal superposition.",
            "   2400  00:40:00  --- slide change · score 12.0 ---",
        ]
    );
    assert!(text.starts_with("# Lecture 7: Grover"));
}

#[test]
fn an_outline_with_no_transcript_is_still_the_slide_changes() {
    let changes = vec![Candidate {
        seconds: 30,
        score: 9.0,
        diff: 9.0,
        pause: false,
    }];
    let text = outline("Silent", &[], &changes);
    assert!(
        text.contains("     30  00:00:30  --- slide change · score 9.0 ---"),
        "{text}"
    );
}

#[test]
fn cue_gaps_reads_both_timestamp_shapes() {
    let vtt = include_str!("../../../fixtures/chapters/sample.vtt");
    let gaps = cue_gaps(vtt);
    assert_eq!(
        gaps,
        vec![
            // First cue: the silence is measured from the start of the file.
            (0, 0.5),
            (4, 0.0),
            // MM:SS.mmm, and a real pause before it.
            (66, 3.0),
            (70, 0.5),
            // HH:MM:SS.mmm past the hour.
            (3675, 3603.0),
        ]
    );
}

#[test]
fn cue_gaps_ignores_headers_notes_and_blank_blocks() {
    let vtt = include_str!("../../../fixtures/chapters/sample.vtt");
    assert_eq!(cue_gaps(vtt).len(), 5);
    assert!(cue_gaps("WEBVTT\n\nnot a cue at all\n").is_empty());
}

#[test]
fn cue_gaps_survives_crlf_and_a_bad_end_time() {
    let vtt =
        "WEBVTT\r\n\r\n00:00.000 --> broken\r\nhello\r\n\r\n00:10.000 --> 00:12.000\r\nworld\r\n";
    // The broken end falls back to its own start.
    assert_eq!(cue_gaps(vtt), vec![(0, 0.0), (10, 10.0)]);
}

#[test]
fn thumbnail_takes_source_one_a_quarter_in() {
    let one = PathBuf::from("/l/source1.mp4");
    let two = PathBuf::from("/l/source2.mp4");
    let both = [(2, two.clone()), (1, one.clone())];
    assert_eq!(thumbnail_pick(&both, 4000), Some((one, 1000)));
    // Only the camera is on disk: it stands in.
    assert_eq!(thumbnail_pick(&[(2, two.clone())], 3001), Some((two, 750)));
    assert_eq!(thumbnail_pick(&[], 4000), None);
}
