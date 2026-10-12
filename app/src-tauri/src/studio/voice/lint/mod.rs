//! The narration checks that need the voice engine. For Kokoro every
//! narrated section is read through the G2P and cut into Kokoro's inputs as
//! `narrate` will: a missing G2P pack, a lexicon entry that is not valid, a
//! word with no phonemes or a sentence too long for Kokoro is a lint issue,
//! and each guessed word is a note. Notes never fail a lint. The server
//! engine reads its own text, so there is nothing to check.

#[cfg(test)]
mod tests;

use std::path::Path;

use studio_core::{LintIssue, LintLevel, Project, ScriptSection, StudioError};

use super::g2p::{Phonemized, Source, WordRecord};
use super::review::Review;
use super::Voice;
use crate::studio::tts::kokoro::{chunks, LexiconFiles, Reader};

/// A guessed word: worth a listen, and a suggested reading if it is wrong.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Note {
    /// The scene, 0-based, and its id.
    pub scene: usize,
    pub id: String,
    /// The script.md line the word is on (1-based).
    pub line: usize,
    pub word: String,
    pub phonemes: String,
    /// The parts the guesser read (all of a word, or one sub-token).
    pub guessed: Vec<String>,
    /// The suggestion awaiting the student's review for this word, if any
    /// (`review::Store::annotate`).
    pub review: Option<Review>,
}

impl Note {
    pub fn message(&self) -> String {
        let head = format!(
            "check pronunciation: \"{}\" is guessed as /{}/",
            self.word, self.phonemes
        );
        match &self.review {
            Some(review) => format!("{head} {}", review.suffix()),
            None => format!(
                "{head}; listen to it, and if it is wrong suggest a reading (`oculus studio pronounce suggest`)"
            ),
        }
    }
}

#[derive(Debug, Default)]
pub struct VoiceLint {
    pub issues: Vec<LintIssue>,
    pub notes: Vec<Note>,
}

/// Checks `project`'s narration for `voice`; nothing for the server engine
/// or a video with no script.
pub fn lint(project: &Project, voice: &Voice, g2p: &Path, lexicon: &LexiconFiles) -> VoiceLint {
    let mut out = VoiceLint::default();
    if !matches!(voice, Voice::Kokoro(_)) || project.script.is_none() {
        return out;
    }
    let mut reader = match Reader::load(g2p, lexicon) {
        Ok(reader) => reader,
        Err(e) => {
            out.issues.push(LintIssue {
                level: LintLevel::Error,
                group: None,
                scene: None,
                id: None,
                component: None,
                example: None,
                error: StudioError::Lint(e),
            });
            return out;
        }
    };
    for (index, section) in sections(project) {
        let read = reader
            .read(&section.text)
            .and_then(|p| chunks(&p).map(|_| p));
        match read {
            Ok(p) => out.notes.extend(section_notes(index, section, &p)),
            Err(message) => out.issues.push(LintIssue {
                level: LintLevel::Error,
                group: None,
                scene: Some(index),
                id: Some(section.id.clone()),
                component: Some(project.video.scenes[index].component.clone()),
                example: None,
                error: StudioError::Script {
                    line: section.line,
                    message,
                },
            }),
        }
    }
    out
}

/// The notes for every narrated section `read` knows, in video order: for
/// `narrate`, whose speaker read the sections it keyed.
pub fn notes(project: &Project, read: impl Fn(&str) -> Option<Phonemized>) -> Vec<Note> {
    sections(project)
        .filter_map(|(index, section)| {
            read(&section.text).map(|p| section_notes(index, section, &p))
        })
        .flatten()
        .collect()
}

/// Each narrated scene's index and section.
pub(crate) fn sections(project: &Project) -> impl Iterator<Item = (usize, &ScriptSection)> {
    (0..project.video.scenes.len()).filter_map(|i| project.section(i).map(|s| (i, s)))
}

/// A note per guessed word in `section`.
fn section_notes(index: usize, section: &ScriptSection, read: &Phonemized) -> Vec<Note> {
    section_words(section, read)
        .filter(|(_, word)| word.source == Source::Guess)
        .map(|(line, word)| Note {
            scene: index,
            id: section.id.clone(),
            line,
            word: word.word.clone(),
            phonemes: word.phonemes.clone(),
            guessed: word.guessed.clone(),
            review: None,
        })
        .collect()
}

/// Each word of `read` with the script.md line of `section` it is found
/// on: words are searched for in order, so a repeated word finds its own
/// line.
pub(crate) fn section_words<'a>(
    section: &'a ScriptSection,
    read: &'a Phonemized,
) -> impl Iterator<Item = (usize, &'a WordRecord)> + 'a {
    let lines: Vec<&str> = section.raw.lines().collect();
    let (mut line, mut column) = (0, 0);
    read.words.iter().map(move |word| {
        if let Some((l, c)) = find(&lines, line, column, &word.word) {
            line = l;
            column = c + word.word.len();
        }
        (section.line + 1 + line, word)
    })
}

/// The first place `word` is at or after `(line, column)`.
fn find(lines: &[&str], line: usize, column: usize, word: &str) -> Option<(usize, usize)> {
    lines.iter().enumerate().skip(line).find_map(|(l, text)| {
        let from = if l == line { column } else { 0 };
        text.get(from..)?.find(word).map(|c| (l, from + c))
    })
}
