use super::super::{Cue, srt, vtt};

fn cues() -> Vec<Cue> {
    vec![
        Cue {
            start: 0.1,
            end: 2.5,
            lines: vec!["We follow the gradient".into(), "downhill,".into()],
        },
        Cue {
            start: 3661.0005,
            end: 3662.25,
            lines: vec!["one step.".into()],
        },
    ]
}

#[test]
fn vtt_and_srt_carry_the_same_cues() {
    assert_eq!(
        vtt(&cues()),
        "WEBVTT\n\n00:00:00.100 --> 00:00:02.500\nWe follow the gradient\ndownhill,\n\n01:01:01.001 --> 01:01:02.250\none step.\n"
    );
    assert_eq!(
        srt(&cues()),
        "1\n00:00:00,100 --> 00:00:02,500\nWe follow the gradient\ndownhill,\n\n2\n01:01:01,001 --> 01:01:02,250\none step.\n"
    );
}
