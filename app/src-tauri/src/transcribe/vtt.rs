//! Segments to WebVTT, in the shape `parseVtt` (`app/src/lib/lectures/media/vtt.ts`)
//! reads: no cue identifiers (a bare number line would join the cue's text),
//! one line of text per cue.

use super::Segment;

/// The file body and how many cues it holds. Segments with no words are
/// skipped, so zero cues means the engine heard nothing.
pub(super) fn render(segments: &[Segment]) -> (String, usize) {
    let mut body = String::from("WEBVTT\n");
    let mut cues = 0;
    for segment in segments {
        let text = clean(&segment.text);
        if text.is_empty() {
            continue;
        }
        let start = seconds(segment.start);
        let end = seconds(segment.end).max(start);
        body.push_str(&format!("\n{} --> {}\n{text}\n", stamp(start), stamp(end)));
        cues += 1;
    }
    (body, cues)
}

/// One line, and never the cue-timing arrow.
fn clean(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("-->", "->")
}

fn seconds(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

/// `HH:MM:SS.mmm`; the hours field is always written.
fn stamp(seconds: f64) -> String {
    let millis = (seconds * 1000.0).round() as u64;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        millis / 3_600_000,
        millis / 60_000 % 60,
        millis / 1000 % 60,
        millis % 1000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(start: f64, end: f64, text: &str) -> Segment {
        Segment {
            start,
            end,
            text: text.into(),
        }
    }

    #[test]
    fn stamps_carry_hours_and_milliseconds() {
        assert_eq!(stamp(0.0), "00:00:00.000");
        assert_eq!(stamp(5.2), "00:00:05.200");
        assert_eq!(stamp(59.9996), "00:01:00.000");
        assert_eq!(stamp(3725.042), "01:02:05.042");
        assert_eq!(stamp(7384.5), "02:03:04.500");
    }

    #[test]
    fn cues_are_trimmed_single_lines_and_empty_ones_are_skipped() {
        let (body, cues) = render(&[
            seg(0.0, 2.5, " Good morning, everyone."),
            seg(2.5, 3.0, "   "),
            seg(3.0, 6.0, " Today:\n  eigenvalues --> eigenvectors "),
        ]);
        assert_eq!(cues, 2);
        assert_eq!(
            body,
            "WEBVTT\n\
             \n00:00:00.000 --> 00:00:02.500\nGood morning, everyone.\n\
             \n00:00:03.000 --> 00:00:06.000\nToday: eigenvalues -> eigenvectors\n"
        );
    }

    #[test]
    fn an_end_before_its_start_is_clamped_and_negatives_are_zero() {
        let (body, _) = render(&[seg(-0.4, 1.0, "a"), seg(10.0, 9.0, "b")]);
        assert!(body.contains("00:00:00.000 --> 00:00:01.000\na"));
        assert!(body.contains("00:00:10.000 --> 00:00:10.000\nb"));
    }

    #[test]
    fn nothing_spoken_is_zero_cues() {
        let (body, cues) = render(&[seg(0.0, 30.0, " "), seg(30.0, 60.0, "")]);
        assert_eq!(cues, 0);
        assert_eq!(body, "WEBVTT\n");
    }
}
