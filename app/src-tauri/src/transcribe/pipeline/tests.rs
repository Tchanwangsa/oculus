use super::*;
use crate::test_support::Scratch as Dir;
use crate::transcribe::engines::groq;
use crate::transcribe::vtt;

#[test]
fn a_second_claim_on_one_video_is_refused_until_the_first_drops() {
    let (a, b) = (
        Path::new("/claim-test/a.mp4"),
        Path::new("/claim-test/b.mp4"),
    );
    let first = Claim::take(a).unwrap();
    assert!(Claim::take(a).is_err());
    assert!(Claim::take(b).is_ok());
    drop(first);
    assert!(Claim::take(a).is_ok());
}

#[test]
fn the_scratch_directory_is_removed_on_drop() {
    let dir = {
        let scratch = Scratch::new().unwrap();
        std::fs::write(scratch.file("audio.ogg"), b"x").unwrap();
        scratch.0.clone()
    };
    assert!(!dir.exists());
}

#[test]
fn the_vtt_sits_beside_the_video_with_its_full_name() {
    assert_eq!(
        vtt_path(Path::new("/lib/courses/MAST/files/Matrices Part 1.mp4")),
        Path::new("/lib/courses/MAST/files/Matrices Part 1.mp4.vtt")
    );
}

#[test]
fn only_files_inside_the_library_resolve() {
    let library = Dir::new("transcribe-library");
    let outside = Dir::new("transcribe-outside");
    let files = library.join("courses").join("MAST").join("files");
    std::fs::create_dir_all(&files).unwrap();
    std::fs::write(files.join("Week 1.mp4"), b"x").unwrap();
    std::fs::write(outside.join("elsewhere.mp4"), b"x").unwrap();
    let inside = files.join("Week 1.mp4").canonicalize().unwrap();

    assert_eq!(
        resolve(&library, "courses/MAST/files/Week 1.mp4").unwrap(),
        inside
    );
    assert_eq!(resolve(&library, inside.to_str().unwrap()).unwrap(), inside);

    let escape = format!(
        "courses/../../{}/elsewhere.mp4",
        outside.file_name().unwrap().to_string_lossy()
    );
    assert!(resolve(&library, &escape)
        .unwrap_err()
        .contains("outside the library"));
    let absolute = outside.join("elsewhere.mp4");
    assert!(resolve(&library, absolute.to_str().unwrap())
        .unwrap_err()
        .contains("outside the library"));

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(outside.join("elsewhere.mp4"), files.join("link.mp4")).unwrap();
        assert!(resolve(&library, "courses/MAST/files/link.mp4")
            .unwrap_err()
            .contains("outside"));
    }

    assert!(resolve(&library, "courses/MAST/files/missing.mp4")
        .unwrap_err()
        .contains("not on disk"));
    assert!(resolve(&library, "courses/MAST/files")
        .unwrap_err()
        .contains("not a file"));
}

/// Answers each call from a script, in order, and records the paths.
struct Scripted {
    name: &'static str,
    cap: Option<u64>,
    replies: Mutex<Vec<Result<Vec<Segment>, EngineError>>>,
    heard: Mutex<Vec<PathBuf>>,
}

impl Scripted {
    fn new(
        name: &'static str,
        cap: Option<u64>,
        replies: Vec<Result<Vec<Segment>, EngineError>>,
    ) -> Box<dyn Engine> {
        Box::new(Self {
            name,
            cap,
            replies: Mutex::new(replies),
            heard: Mutex::new(vec![]),
        })
    }
}

impl Engine for Scripted {
    fn name(&self) -> &'static str {
        self.name
    }
    fn max_upload_bytes(&self) -> Option<u64> {
        self.cap
    }
    fn transcribe(&self, audio: &Path) -> Result<Vec<Segment>, EngineError> {
        self.heard.lock().unwrap().push(audio.to_path_buf());
        self.replies.lock().unwrap().remove(0)
    }
}

fn said(text: &str) -> Result<Vec<Segment>, EngineError> {
    Ok(vec![Segment {
        start: 1.0,
        end: 2.0,
        text: text.into(),
    }])
}

/// 30 MB over 90 s: three parts under a 20 MB-ish cap, one without.
const AUDIO: audio::Extracted = audio::Extracted {
    bytes: 30_000_000,
    duration: Some(90.0),
};

fn hear(engines: &[Box<dyn Engine>]) -> (Result<Heard, String>, Vec<String>) {
    let steps = Mutex::new(Vec::new());
    let result = transcribe_audio(
        engines,
        &AUDIO,
        Path::new("/scratch/audio.ogg"),
        |engine, index, _| Ok(PathBuf::from(format!("/scratch/part-{engine}-{index}.ogg"))),
        &|step| {
            if let Step::Transcribing {
                engine,
                chunk,
                chunks,
            } = step
            {
                steps
                    .lock()
                    .unwrap()
                    .push(format!("{engine} {chunk}/{chunks}"));
            }
        },
    );
    (result, steps.into_inner().unwrap())
}

#[test]
fn a_rate_limit_mid_run_hands_the_whole_audio_to_the_next_engine() {
    let engines = [
        Scripted::new(
            "groq",
            Some(12_000_000),
            vec![
                said("from groq"),
                Err(EngineError::RateLimited("Groq's limit".into())),
            ],
        ),
        Scripted::new("apple", None, vec![said("on device")]),
    ];
    let (result, steps) = hear(&engines);
    let heard = result.unwrap();
    assert_eq!(heard.engine, "apple");
    assert_eq!(heard.chunks, 1);
    // Groq's first part is dropped, and the whole file is reused unsplit.
    assert_eq!(
        heard.segments,
        vec![Segment {
            start: 1.0,
            end: 2.0,
            text: "on device".into()
        }]
    );
    assert_eq!(steps, vec!["groq 1/3", "groq 2/3", "apple 1/1"]);
}

#[test]
fn a_failure_surfaces_and_never_falls_through() {
    let engines = [
        Scripted::new(
            "groq",
            None,
            vec![Err(EngineError::Failed(
                "Groq rejected the saved key".into(),
            ))],
        ),
        Scripted::new("apple", None, vec![said("unreached")]),
    ];
    let (result, steps) = hear(&engines);
    assert_eq!(result.err().unwrap(), "Groq rejected the saved key");
    assert_eq!(steps, vec!["groq 1/1"]);
}

#[test]
fn when_every_engine_declines_each_reason_is_reported() {
    let engines = [
        Scripted::new(
            "groq",
            None,
            vec![Err(EngineError::RateLimited(
                "Groq: try again in 5 minutes".into(),
            ))],
        ),
        Scripted::new(
            "apple",
            None,
            vec![Err(EngineError::NotConfigured("needs macOS 26".into()))],
        ),
    ];
    let message = hear(&engines).0.err().unwrap();
    assert!(
        message.contains("try again in 5 minutes") && message.contains("needs macOS 26"),
        "{message}"
    );

    let alone = [Scripted::new(
        "apple",
        None,
        vec![Err(EngineError::NotConfigured("needs macOS 26".into()))],
    )];
    assert_eq!(hear(&alone).0.err().unwrap(), "needs macOS 26");
}

#[test]
fn an_engine_that_answers_first_is_the_only_one_called() {
    let engines = [
        Scripted::new(
            "groq",
            Some(12_000_000),
            vec![said("a"), said("b"), said("c")],
        ),
        Scripted::new("apple", None, vec![]),
    ];
    let heard = hear(&engines).0.unwrap();
    assert_eq!((heard.engine, heard.chunks), ("groq", 3));
    // Each part is on the video's clock: 30 s apart.
    let starts: Vec<f64> = heard.segments.iter().map(|s| s.start).collect();
    assert_eq!(starts, vec![1.0, 31.0, 61.0]);
}

#[test]
fn chunk_replies_become_one_vtt_on_the_videos_clock() {
    // Two chunks of a long recording, the second starting past the hour.
    let first =
        groq::parse_segments(r#"{"segments":[{"start":0.0,"end":3.25,"text":" Welcome back."}]}"#)
            .unwrap();
    let second = groq::parse_segments(
        r#"{"segments":[
            {"start":0.5,"end":4.0,"text":" Now the proof."},
            {"start":4.0,"end":5.0,"text":" "}
        ]}"#,
    )
    .unwrap();
    let segments: Vec<Segment> = shift(first, 0.0).chain(shift(second, 3725.0)).collect();
    let (body, cues) = vtt::render(&segments);
    assert_eq!(cues, 2);
    assert_eq!(
        body,
        "WEBVTT\n\
         \n00:00:00.000 --> 00:00:03.250\nWelcome back.\n\
         \n01:02:05.500 --> 01:02:09.000\nNow the proof.\n"
    );
}
