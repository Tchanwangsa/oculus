//! Apple's on-device recogniser (macOS 26 `SpeechAnalyzer`), through the
//! `apple-speech` helper (`app/src-tauri/speech/main.swift`). Free, offline,
//! any length in one call. Its results are whole utterances — up to half a
//! minute — so they are split into caption-sized cues on their word timings.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::transcribe::{Engine, EngineError, Segment, Settings};

const HELPER: &str = "apple-speech";

/// The helper's exit code for "this Mac cannot recognise speech on device".
const UNAVAILABLE: i32 = 3;

/// The longest cue a split leaves, and the shortest it aims not to.
const MAX_CUE: f64 = 7.0;
const MIN_CUE: f64 = 1.5;

pub(in crate::transcribe) struct Apple {
    helper: PathBuf,
    /// A locale identifier such as `en_AU`; `None` is the helper's default,
    /// English in the Mac's region.
    locale: Option<String>,
}

impl Apple {
    /// The engine as Settings has it, or why it cannot run here.
    pub(in crate::transcribe) fn configured(
        settings: &Settings,
        resource_dir: Option<PathBuf>,
    ) -> Result<Self, String> {
        if !cfg!(target_os = "macos") {
            return Err("on-device speech is macOS only".into());
        }
        if !settings.apple {
            return Err("on-device speech is turned off in Settings → Transcription".into());
        }
        let helper = helper(resource_dir)?;
        Ok(Self {
            helper,
            locale: settings.language.apple(),
        })
    }
}

fn helper(resource_dir: Option<PathBuf>) -> Result<PathBuf, String> {
    crate::runtime::bundled::find(HELPER, resource_dir).ok_or_else(|| {
        "the on-device speech helper is missing — run `bun run speech` in app/".into()
    })
}

impl Engine for Apple {
    fn name(&self) -> &'static str {
        "apple"
    }

    fn max_upload_bytes(&self) -> Option<u64> {
        None
    }

    fn transcribe(&self, audio: &Path) -> Result<Vec<Segment>, EngineError> {
        let mut command = Command::new(&self.helper);
        command.arg("transcribe");
        if let Some(locale) = &self.locale {
            command.arg("--locale").arg(locale);
        }
        // No timeout: an hour of audio takes the recogniser about a minute,
        // and a first run also downloads the language's model.
        let output = command.arg(audio).output().map_err(|e| {
            EngineError::Failed(format!("could not run the on-device speech helper: {e}"))
        })?;
        answer(
            output.status.code(),
            &String::from_utf8_lossy(&output.stdout),
            &String::from_utf8_lossy(&output.stderr),
        )
    }
}

/// The helper's exit, read: 0 is a transcript, 3 is "not on this Mac" (which
/// a later engine may answer), anything else fails with its last stderr line.
fn answer(code: Option<i32>, stdout: &str, stderr: &str) -> Result<Vec<Segment>, EngineError> {
    let said = stderr.lines().rev().map(str::trim).find(|l| !l.is_empty());
    match code {
        Some(0) => Ok(cues(parse_reply(stdout)?)),
        Some(UNAVAILABLE) => Err(EngineError::NotConfigured(
            said.unwrap_or("on-device speech is not available on this Mac")
                .to_string(),
        )),
        Some(code) => Err(EngineError::Failed(match said {
            Some(line) => line.to_string(),
            None => format!("the on-device speech helper exited with code {code}"),
        })),
        None => Err(EngineError::Failed(
            "the on-device speech helper was killed".into(),
        )),
    }
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub(in crate::transcribe) struct Word {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// One recogniser result: an utterance, with each word's timing.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub(in crate::transcribe) struct Utterance {
    pub start: f64,
    pub end: f64,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub words: Vec<Word>,
}

pub(in crate::transcribe) fn parse_reply(body: &str) -> Result<Vec<Utterance>, EngineError> {
    #[derive(serde::Deserialize)]
    struct Reply {
        #[serde(default)]
        segments: Vec<Utterance>,
    }
    serde_json::from_str::<Reply>(body)
        .map(|reply| reply.segments)
        .map_err(|e| {
            EngineError::Failed(format!(
                "on-device speech's reply was not a transcript: {e}"
            ))
        })
}

pub(in crate::transcribe) fn cues(utterances: Vec<Utterance>) -> Vec<Segment> {
    utterances.iter().flat_map(split).collect()
}

/// An utterance as cues of at most [`MAX_CUE`] seconds, cut between words:
/// after a sentence end if one fits, else after a clause break in the second
/// half of the window, else wherever leaves the pieces closest to even.
/// No piece under [`MIN_CUE`] unless the words allow nothing else.
fn split(utterance: &Utterance) -> Vec<Segment> {
    let words: Vec<&Word> = utterance
        .words
        .iter()
        .filter(|w| !w.text.trim().is_empty())
        .collect();
    if utterance.end - utterance.start <= MAX_CUE || words.len() < 2 {
        return vec![Segment {
            start: utterance.start,
            end: utterance.end,
            text: utterance.text.trim().to_string(),
        }];
    }
    let mut pieces = Vec::new();
    let mut from = 0;
    while from < words.len() {
        let to = from + cut_after(&words[from..]);
        pieces.push((from, to));
        from = to + 1;
    }
    let last = pieces.len() - 1;
    pieces
        .iter()
        .enumerate()
        .map(|(k, &(from, to))| Segment {
            start: if k == 0 {
                utterance.start.min(words[from].start)
            } else {
                words[from].start
            },
            end: if k == last {
                utterance.end.max(words[to].end)
            } else {
                words[to].end
            },
            text: words[from..=to]
                .iter()
                .map(|w| w.text.trim())
                .collect::<Vec<_>>()
                .join(" "),
        })
        .collect()
}

/// The index, within `words`, of the last word of the first piece.
fn cut_after(words: &[&Word]) -> usize {
    let last = words.len() - 1;
    let start = words[0].start;
    let total = words[last].end - start;
    if total <= MAX_CUE {
        return last;
    }
    let len = |j: usize| words[j].end - start;
    let fits = |j: usize| len(j) <= MAX_CUE;
    let comfortable =
        |j: usize| fits(j) && len(j) >= MIN_CUE && words[last].end - words[j + 1].start >= MIN_CUE;
    let latest = |ok: &dyn Fn(usize) -> bool| (0..last).rev().find(|&j| ok(j));

    if let Some(j) = latest(&|j| comfortable(j) && ends_sentence(&words[j].text)) {
        return j;
    }
    if let Some(j) =
        latest(&|j| comfortable(j) && len(j) >= MAX_CUE / 2.0 && ends_clause(&words[j].text))
    {
        return j;
    }
    let share = total / (total / MAX_CUE).ceil();
    if let Some(j) = (0..last)
        .filter(|&j| comfortable(j))
        .min_by(|&a, &b| (len(a) - share).abs().total_cmp(&(len(b) - share).abs()))
    {
        return j;
    }
    latest(&fits).unwrap_or(0)
}

/// The word's last character, past any closing quote or bracket.
fn final_mark(word: &str) -> Option<char> {
    word.trim()
        .chars()
        .rev()
        .find(|c| !matches!(c, '"' | '\'' | ')' | ']' | '”' | '’'))
}

fn ends_sentence(word: &str) -> bool {
    matches!(final_mark(word), Some('.' | '?' | '!'))
}

fn ends_clause(word: &str) -> bool {
    matches!(final_mark(word), Some(',' | ';' | ':'))
}

/// What `apple-speech locales` reports, passed to Settings as is.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub available: bool,
    pub reason: Option<String>,
    #[serde(default)]
    pub supported: Vec<String>,
    #[serde(default)]
    pub installed: Vec<String>,
    pub default_locale: Option<String>,
}

impl Status {
    fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            available: false,
            reason: Some(reason.into()),
            supported: Vec::new(),
            installed: Vec::new(),
            default_locale: None,
        }
    }
}

/// Whether this Mac can transcribe on device, and in which languages. Reads
/// the OS's lists only: never downloads or transcribes anything.
pub(in crate::transcribe) fn status(resource_dir: Option<PathBuf>) -> Status {
    if !cfg!(target_os = "macos") {
        return Status::unavailable("On-device speech is macOS only");
    }
    let helper = match helper(resource_dir) {
        Ok(helper) => helper,
        Err(reason) => return Status::unavailable(reason),
    };
    let output = match Command::new(&helper).arg("locales").output() {
        Ok(output) => output,
        Err(e) => {
            return Status::unavailable(format!("could not run the on-device speech helper: {e}"))
        }
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let said = stderr.lines().rev().map(str::trim).find(|l| !l.is_empty());
        return Status::unavailable(said.unwrap_or("the on-device speech helper failed"));
    }
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        Status::unavailable(format!(
            "the on-device speech helper's reply was unreadable: {e}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(start: f64, end: f64, text: &str) -> Word {
        Word {
            start,
            end,
            text: text.into(),
        }
    }

    /// Words of `step` seconds each, back to back from `start`.
    fn utterance(start: f64, step: f64, text: &str) -> Utterance {
        let words: Vec<Word> = text
            .split(' ')
            .enumerate()
            .map(|(i, w)| word(start + step * i as f64, start + step * (i + 1) as f64, w))
            .collect();
        Utterance {
            start,
            end: words.last().unwrap().end,
            text: text.into(),
            words,
        }
    }

    fn texts(segments: &[Segment]) -> Vec<&str> {
        segments.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn the_helpers_reply_is_read_and_extra_fields_ignored() {
        let utterances = parse_reply(
            r#"{"segments":[{"end":2.5,"start":0,"text":"Good morning.","confidence":0.9,
                "words":[{"end":1.2,"start":0,"text":"Good"},{"end":2.5,"start":1.2,"text":"morning."}]}],
               "locale":"en_AU"}"#,
        )
        .unwrap();
        assert_eq!(
            utterances,
            vec![Utterance {
                start: 0.0,
                end: 2.5,
                text: "Good morning.".into(),
                words: vec![word(0.0, 1.2, "Good"), word(1.2, 2.5, "morning.")],
            }]
        );
        assert!(parse_reply(r#"{"segments":[]}"#).unwrap().is_empty());
        assert!(parse_reply("{}").unwrap().is_empty());
        assert!(matches!(
            parse_reply("downloading"),
            Err(EngineError::Failed(_))
        ));
    }

    #[test]
    fn a_short_utterance_is_one_cue_with_its_own_times() {
        let u = Utterance {
            start: 3.0,
            end: 9.5,
            text: " Hello there. ".into(),
            words: vec![],
        };
        assert_eq!(
            cues(vec![u]),
            vec![Segment {
                start: 3.0,
                end: 9.5,
                text: "Hello there.".into()
            }]
        );
        // Long, but with no word timings to cut on.
        let u = Utterance {
            start: 0.0,
            end: 30.0,
            text: "a long one".into(),
            words: vec![],
        };
        assert_eq!(texts(&cues(vec![u])), vec!["a long one"]);
    }

    #[test]
    fn a_long_utterance_is_cut_after_a_sentence_end_that_fits() {
        // 0.5 s words, 8.5 s in all: "two." ends 2 s in, and what follows fits one cue.
        let u = utterance(10.0, 0.5, "one more one two. three more four, five six seven eight nine ten eleven twelve thirteen fourteen");
        let pieces = cues(vec![u]);
        assert_eq!(pieces[0].text, "one more one two.");
        assert_eq!((pieces[0].start, pieces[0].end), (10.0, 12.0));
        assert_eq!(pieces[1].start, 12.0);
        assert!(pieces.iter().all(|p| p.end - p.start <= MAX_CUE + 1e-9));
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[1].end, 18.5);
    }

    #[test]
    fn without_a_sentence_end_a_late_clause_break_is_taken() {
        // 1 s words: "e," ends at 5 s, past half the window; "b," at 2 s is too early.
        let u = utterance(0.0, 1.0, "a b, c d e, f g h i j");
        let pieces = cues(vec![u]);
        assert_eq!(texts(&pieces), vec!["a b, c d e,", "f g h i j"]);
    }

    #[test]
    fn unpunctuated_speech_is_cut_into_even_pieces() {
        // 9 s of words: two pieces near 4.5 s, not 7 s and 2 s.
        let u = utterance(0.0, 0.5, "w w w w w w w w w w w w w w w w w w");
        let pieces = cues(vec![u]);
        assert_eq!(pieces.len(), 2);
        assert_eq!((pieces[0].start, pieces[0].end), (0.0, 4.5));
        assert_eq!((pieces[1].start, pieces[1].end), (4.5, 9.0));
        // 30 s: five pieces, each at most 7 s and none under 1.5 s.
        let long = utterance(0.0, 0.5, &["w"; 60].join(" "));
        let pieces = cues(vec![long]);
        assert_eq!(pieces.len(), 5);
        assert!(pieces
            .iter()
            .all(|p| (MIN_CUE..=MAX_CUE).contains(&(p.end - p.start))));
    }

    #[test]
    fn a_sentence_end_that_would_leave_a_scrap_is_passed_over() {
        // "done." at 6.5 s would leave a 1 s tail; the cut moves earlier.
        let u = utterance(0.0, 0.5, "a b c d e f g h i j k l done. m n");
        let pieces = cues(vec![u]);
        assert!(
            pieces.iter().all(|p| p.end - p.start >= MIN_CUE),
            "{pieces:?}"
        );
        assert!(pieces.iter().all(|p| p.end - p.start <= MAX_CUE));
    }

    #[test]
    fn a_word_longer_than_the_window_stands_alone() {
        let u = Utterance {
            start: 0.0,
            end: 12.0,
            text: "uh well".into(),
            words: vec![word(0.0, 9.0, "uh"), word(9.0, 12.0, "well")],
        };
        assert_eq!(texts(&cues(vec![u])), vec!["uh", "well"]);
    }

    #[test]
    fn closing_quotes_do_not_hide_the_punctuation() {
        assert!(ends_sentence("\"done.\""));
        assert!(ends_sentence("really?)"));
        assert!(ends_clause("first,"));
        assert!(!ends_sentence("Mr"));
    }

    #[test]
    fn the_helpers_exit_code_decides_the_answer() {
        assert!(matches!(
            answer(Some(3), "", "On-device speech needs macOS 26 or later\n"),
            Err(EngineError::NotConfigured(m)) if m == "On-device speech needs macOS 26 or later"
        ));
        assert!(matches!(
            answer(Some(1), "", "downloading the speech model for en_US\nno such file: /x.ogg\n"),
            Err(EngineError::Failed(m)) if m == "no such file: /x.ogg"
        ));
        assert!(
            matches!(answer(Some(1), "", ""), Err(EngineError::Failed(m)) if m.contains("code 1"))
        );
        assert!(matches!(answer(None, "", ""), Err(EngineError::Failed(_))));
        let ok = answer(
            Some(0),
            r#"{"segments":[{"start":1,"end":2,"text":"Hi.","words":[]}]}"#,
            "",
        )
        .unwrap();
        assert_eq!(
            ok,
            vec![Segment {
                start: 1.0,
                end: 2.0,
                text: "Hi.".into()
            }]
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_engine_runs_the_helper_with_the_locale_and_the_audio() {
        use std::os::unix::fs::PermissionsExt;
        let dir = crate::test_support::Scratch::new("apple-helper");
        let script = dir.join("apple-speech");
        // Echoes its arguments back as the one utterance's text.
        std::fs::write(
            &script,
            "#!/bin/sh\nprintf '{\"segments\":[{\"start\":0,\"end\":1,\"text\":\"%s\"}]}' \"$*\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let apple = Apple {
            helper: script,
            locale: Some("en_AU".into()),
        };
        let found = apple.transcribe(Path::new("/tmp/audio.ogg")).unwrap();
        assert_eq!(found[0].text, "transcribe --locale en_AU /tmp/audio.ogg");
        assert_eq!(apple.max_upload_bytes(), None);
    }
}
