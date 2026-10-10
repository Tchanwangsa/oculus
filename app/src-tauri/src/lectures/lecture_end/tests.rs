use super::picture::{black_tail, Tail};
use super::prompt::{prompt, Prompt};
use super::reply::{ask, parse_reply, validate, Found, Reply};
use super::window::{clock, recording_length, transcript_lines, window, Line};
use crate::lectures::chapters::TranscriptCue;

fn cue(start: f32, end: f32, speaker: &str, text: &str) -> (TranscriptCue, Option<String>) {
    (
        TranscriptCue {
            start,
            end,
            text: text.into(),
        },
        Some(speaker.to_string()).filter(|s| !s.is_empty()),
    )
}

fn line(start: u32, end: u32, text: &str) -> Line {
    Line {
        start,
        end,
        speaker: Some("Speaker 0".into()),
        text: text.into(),
    }
}

fn reply(ends_at: Option<f64>, quote: Option<&str>) -> Reply {
    Reply {
        ends_at,
        quote: quote.map(str::to_string),
    }
}

/// A sign-off split over two cues, then a student's question.
fn sign_off() -> Vec<Line> {
    vec![
        line(1890, 1894, "So that is the proof of the theorem."),
        line(1895, 1899, "OK, that's it for today, thank you,"),
        line(1899, 1902, "and I'll see you all on Thursday."),
        line(1905, 1909, "Thanks! Can I ask about question three?"),
    ]
}

#[test]
fn the_window_is_the_last_fifteen_minutes_of_the_longer_length() {
    let cues = vec![
        cue(10.0, 12.0, "Speaker 0", "Hello."),
        cue(1199.5, 1201.0, "Speaker 0", "Before the window."),
        cue(1200.4, 1203.9, "Speaker 0", "First in the window."),
        cue(2095.2, 2101.7, "Speaker 1", "Last cue."),
    ];
    assert_eq!(
        recording_length(2000, &cues),
        2101,
        "the last cue outruns the row"
    );
    assert_eq!(recording_length(2400, &cues), 2400);
    assert_eq!(
        recording_length(0, &cues),
        2101,
        "an unknown duration takes the cues'"
    );
    let lines = window(&cues, 2100);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], line(1200, 1203, "First in the window."));
    assert_eq!(lines[1].speaker.as_deref(), Some("Speaker 1"));
    assert_eq!(
        window(&cues, 600).len(),
        4,
        "a short recording is read whole"
    );
}

#[test]
fn lines_print_both_columns_and_drop_a_lone_speaker() {
    assert_eq!(
        (clock(65), clock(3725)),
        ("01:05".to_string(), "1:02:05".to_string())
    );
    let mut lines = vec![
        line(1915, 1918, "See you tomorrow."),
        line(3725, 3727, "Bye."),
    ];
    assert_eq!(
        transcript_lines(&lines),
        "1915  31:55  See you tomorrow.\n3725  1:02:05  Bye."
    );
    lines[1].speaker = Some("Speaker 1".into());
    assert_eq!(
        transcript_lines(&lines),
        "1915  31:55  Speaker 0: See you tomorrow.\n3725  1:02:05  Speaker 1: Bye."
    );
    let mut again = line(3728, 3729, "Thanks.");
    again.speaker = Some("Speaker 1".into());
    lines.push(again);
    assert!(transcript_lines(&lines).ends_with("Speaker 1: Bye.\n3728  1:02:08  Thanks."));
}

#[test]
fn the_prompt_carries_the_length_the_hint_and_the_lines() {
    let lines = sign_off();
    let text = prompt(&Prompt {
        title: "Lecture 12",
        code: Some("COMP30026"),
        length: 2290,
        black_from: Some(1925),
        lines: &lines,
    });
    assert!(text
        .starts_with("Lecture: Lecture 12\nCourse: COMP30026\nRecording length: 38:10 (2290 s)\n"));
    assert!(
        text.contains("The projector goes black from 32:05 (1925) to the end of the recording.\n")
    );
    assert!(text.contains("1899  31:39  and I'll see you all on Thursday."));
    let bare = prompt(&Prompt {
        title: "T",
        code: None,
        length: 600,
        black_from: None,
        lines: &lines,
    });
    assert!(!bare.contains("Course:") && !bare.contains("projector"));
    assert!(bare.contains("The whole transcript:"));
}

fn luma(picture: usize, black: usize, tail_picture: usize) -> Vec<f32> {
    let mut out = vec![120.0; picture];
    out.extend(std::iter::repeat(3.0).take(black));
    out.extend(std::iter::repeat(90.0).take(tail_picture));
    out
}

#[test]
fn a_black_run_counts_only_to_the_end_and_after_real_picture() {
    assert_eq!(black_tail(&luma(500, 400, 0)), Tail::Black(500));
    assert_eq!(
        black_tail(&luma(0, 900, 0)),
        Tail::Dead,
        "black from the window's start"
    );
    assert_eq!(
        black_tail(&luma(30, 870, 0)),
        Tail::Dead,
        "too little picture before it"
    );
    assert_eq!(
        black_tail(&luma(860, 40, 0)),
        Tail::Picture,
        "too short to be the projector off"
    );
    assert_eq!(
        black_tail(&luma(400, 300, 200)),
        Tail::Picture,
        "picture comes back"
    );
    assert_eq!(black_tail(&luma(900, 0, 0)), Tail::Picture);
    assert_eq!(black_tail(&[]), Tail::Dead);
}

#[test]
fn replies_parse_through_fences_prose_floats_and_nulls() {
    let fenced = "```json\n{\"ends_at\": 1895, \"quote\": \"that's it for today\"}\n```";
    assert_eq!(
        parse_reply(fenced).unwrap(),
        reply(Some(1895.0), Some("that's it for today"))
    );
    let prose = "Looking at the tail, the lecturer wraps up here:\n{\"ends_at\": 1895.0, \"quote\": \"thank you\"} — after that it is Q&A.";
    assert_eq!(
        parse_reply(prose).unwrap(),
        reply(Some(1895.0), Some("thank you"))
    );
    assert_eq!(
        parse_reply("{\"ends_at\": \"1899\", \"quote\": \"see you\"}")
            .unwrap()
            .ends_at,
        Some(1899.0)
    );
    assert_eq!(
        parse_reply("{\"ends_at\": \"1911 31:51\", \"quote\": \"see you\"}")
            .unwrap()
            .ends_at,
        Some(1911.0)
    );
    assert_eq!(
        parse_reply("{\"result\": {\"ends_at\": 12.5, \"quote\": \"bye\"}}").unwrap(),
        reply(Some(12.5), Some("bye"))
    );
    assert_eq!(
        parse_reply("{\"ends_at\": null, \"quote\": null}").unwrap(),
        reply(None, None)
    );
    assert_eq!(
        parse_reply("{\"ends_at\": null}").unwrap(),
        reply(None, None)
    );
    assert!(parse_reply("The lecture ends at 31:35.").is_err());
    assert!(parse_reply("{\"start\": 3}").is_err());
}

#[test]
fn validation_takes_the_end_of_the_cited_line() {
    let lines = sign_off();
    let found = validate(&reply(Some(1895.0), Some("That's it for today")), &lines).unwrap();
    assert_eq!(
        found,
        Some(Found {
            cue_start: 1895,
            end: 1899,
            quote: "That's it for today".into()
        })
    );
    let rounded = validate(&reply(Some(1895.4), Some("thank you")), &lines)
        .unwrap()
        .unwrap();
    assert_eq!(rounded.end, 1899);
    assert_eq!(validate(&reply(None, None), &lines).unwrap(), None);
}

#[test]
fn a_quote_running_into_the_next_line_ends_with_that_line() {
    let lines = sign_off();
    let found = validate(
        &reply(Some(1895.0), Some("thank you, and I'll see you all")),
        &lines,
    )
    .unwrap()
    .unwrap();
    assert_eq!((found.cue_start, found.end), (1895, 1902));
    // Two cues share second 1899: one ends there, one starts there.
    let shared = validate(
        &reply(Some(1899.0), Some("see you all on Thursday")),
        &lines,
    )
    .unwrap()
    .unwrap();
    assert_eq!((shared.cue_start, shared.end), (1899, 1902));
}

#[test]
fn validation_rejects_what_the_transcript_does_not_say() {
    let mut lines = sign_off();
    // A second beside the quoted line still finds it: the quote is the evidence.
    let beside = validate(&reply(Some(1890.0), Some("that's it for today")), &lines)
        .unwrap()
        .unwrap();
    assert_eq!((beside.cue_start, beside.end), (1895, 1899));
    let wrong = validate(&reply(Some(1895.0), Some("see you next week")), &lines).unwrap_err();
    assert!(wrong.contains("not in the transcript near 1895"), "{wrong}");
    lines.push(line(1960, 1963, "See you next week."));
    let far = validate(&reply(Some(1895.0), Some("see you next week")), &lines).unwrap_err();
    assert!(far.contains("it is in the line at 1960"), "{far}");
    assert!(validate(&reply(Some(5000.0), Some("thank you")), &lines).is_err());
    assert!(validate(&reply(Some(1895.0), None), &lines).is_err());
    assert!(validate(&reply(None, Some("thank you")), &lines).is_err());
    assert!(validate(&reply(Some(1895.0), Some("...")), &lines).is_err());
    assert!(validate(&reply(Some(-3.0), Some("thank you")), &lines).is_err());
    // Words, not substrings.
    assert!(validate(&reply(Some(1890.0), Some("theorem s")), &lines).is_err());
}

#[test]
fn a_rejected_reply_is_asked_again_once_with_the_reason() {
    let lines = sign_off();
    let mut seen: Vec<String> = Vec::new();
    let replies = [
        "{\"ends_at\": 1895, \"quote\": \"see you next week\"}",
        "{\"ends_at\": 1895, \"quote\": \"thank you\"}",
    ];
    let found = ask(
        |text| {
            seen.push(text.to_string());
            Ok(replies[seen.len() - 1].to_string())
        },
        "PROMPT",
        &lines,
    )
    .unwrap();
    assert_eq!(found.map(|f| f.end), Some(1899));
    assert_eq!(seen.len(), 2);
    assert!(
        seen[1].starts_with(
            "PROMPT\nYour previous reply was rejected: the quote \"see you next week\" is not"
        ),
        "{}",
        seen[1]
    );

    let twice = ask(|_| Ok("no idea".to_string()), "PROMPT", &lines).unwrap_err();
    assert!(twice.starts_with("the reply was rejected twice"), "{twice}");
    let mut calls = 0;
    let down = ask(
        |_| {
            calls += 1;
            Err("not signed in".to_string())
        },
        "PROMPT",
        &lines,
    );
    assert_eq!(
        (down, calls),
        (Err("not signed in".to_string()), 1),
        "a provider failure is not retried"
    );
}

#[test]
fn voice_tags_name_the_speaker_and_leave_the_text_plain() {
    let vtt = "WEBVTT\n\n00:31:35.000 --> 00:31:38.500\n<v Speaker 0>That's it for today.\n\n\
               NOTE CONF {\"raw\":[99]}\n\n31:40.000 --> 31:42.000\n<v.loud Speaker 1>Thanks!\n\n\
               31:43.000 --> 31:44.000\nNo tag here.\n";
    let cues = crate::lectures::chapters::parse_transcript_voiced(vtt);
    assert_eq!(cues.len(), 3);
    assert_eq!(cues[0].0.text, "That's it for today.");
    assert_eq!(cues[0].1.as_deref(), Some("Speaker 0"));
    assert_eq!(cues[1].1.as_deref(), Some("Speaker 1"));
    assert_eq!(cues[2].1, None);
    assert_eq!(crate::lectures::chapters::parse_transcript(vtt).len(), 3);
}
