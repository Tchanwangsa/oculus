use super::super::align::assemble;
use super::super::align_with;
use super::super::gaps::Heard;
use super::super::viterbi::Span;
use super::words;
use crate::studio::NarrationError;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn span(start_frame: usize, end_frame: usize, mean_logp: f64) -> Span {
    Span {
        start_frame,
        end_frame,
        mean_logp,
    }
}

fn heard(next: usize, start_frame: usize, end_frame: usize, letters: usize, text: &str) -> Heard {
    Heard {
        next,
        start_frame,
        end_frame,
        letter_frames: letters,
        heard: text.into(),
    }
}

#[test]
fn starts_take_the_bias_clamped_at_zero_and_ends_do_not() {
    let script = words(&["Hello,", "world!"]);
    let out = assemble(
        &[span(2, 10, -0.1), span(20, 30, -9.5)],
        &[],
        &[0, 1],
        &script,
    );
    let got: Vec<(usize, &str, f64, f64, f64)> = out
        .words
        .iter()
        .map(|w| (w.index, w.text.as_str(), w.start, w.end, w.confidence))
        .collect();
    let want = [
        (0, "Hello,", 0.0, 0.22, -0.1),
        (1, "world!", 0.4 - 0.065, 0.62, -9.5),
    ];
    assert_eq!(got.len(), want.len());
    for (g, w) in got.iter().zip(want) {
        assert_eq!((g.0, g.1), (w.0, w.1));
        assert!(
            close(g.2, w.2) && close(g.3, w.3) && close(g.4, w.4),
            "{g:?} vs {w:?}"
        );
    }
}

#[test]
fn words_without_letters_are_left_out_and_indices_point_into_the_script() {
    let script = words(&["—", "Hello", "—", "world"]);
    let out = assemble(
        &[span(5, 10, 0.0), span(20, 30, 0.0)],
        &[
            heard(0, 0, 5, 4, "UM"),
            heard(1, 11, 20, 7, "THICK"),
            heard(2, 31, 40, 3, "SO"),
        ],
        &[1, 3],
        &script,
    );
    let indices: Vec<(usize, &str)> = out
        .words
        .iter()
        .map(|w| (w.index, w.text.as_str()))
        .collect();
    assert_eq!(indices, vec![(1, "Hello"), (3, "world")]);
    // A gap's `after` indexes the aligned words, as the manifest's does.
    // Four or more letter frames is a gap; three is not.
    let gaps: Vec<(Option<usize>, f64, f64, &str)> = out
        .gaps
        .iter()
        .map(|g| (g.after, g.start, g.end, g.heard.as_str()))
        .collect();
    let want = [(None, 0.0, 0.1, "UM"), (Some(0), 0.22, 0.4, "THICK")];
    assert_eq!(gaps.len(), want.len());
    for (g, w) in gaps.iter().zip(want) {
        assert_eq!((g.0, g.3), (w.0, w.3));
        assert!(close(g.1, w.1) && close(g.2, w.2), "{g:?} vs {w:?}");
    }
}

#[test]
fn a_script_with_no_letters_is_refused_before_the_model_loads() {
    let error = align_with(
        std::path::Path::new("/nonexistent/model.onnx"),
        &[0.0; 16_000],
        &words(&["—", "42"]),
    )
    .unwrap_err();
    assert_eq!(error, NarrationError::EmptyScript);
}

#[test]
fn a_clip_over_sixty_seconds_is_refused_before_the_model_loads() {
    let pcm = vec![0.0; studio_core::MAX_NARRATION_SECS as usize * 16_000 + 1];
    let error = align_with(
        std::path::Path::new("/nonexistent/model.onnx"),
        &pcm,
        &words(&["hello"]),
    )
    .unwrap_err();
    assert!(matches!(error, NarrationError::TooLong { .. }));
    assert!(error.to_string().contains("split the paragraph"), "{error}");
}

#[test]
fn a_model_that_will_not_load_is_an_onnx_error() {
    let error = align_with(
        std::path::Path::new("/nonexistent/model.onnx"),
        &[0.1; 16_000],
        &words(&["hello"]),
    )
    .unwrap_err();
    assert!(matches!(error, NarrationError::Onnx(_)), "{error}");
}
