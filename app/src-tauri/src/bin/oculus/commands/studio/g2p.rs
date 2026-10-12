//! `oculus studio g2p`: text to Kokoro's phonemes, word by word, with the
//! lexicon a video would read (`app_lib::studio::tts::kokoro::Reader`).

use crate::*;
use app_lib::studio::libraries;
use app_lib::studio::tts::kokoro::{self, LexiconFiles, Reader};
use app_lib::studio::voice::g2p::Source;

impl Ctx {
    pub(crate) fn studio_g2p(&self, args: &StudioG2pArgs) -> Result<(), String> {
        let video = match &args.video {
            Some(dir) => Some(dir.clone()),
            None => Some(PathBuf::from(".")).filter(|d| d.join("video.json").is_file()),
        };
        let subject = match (&args.subject, &video) {
            (Some(code), _) => Some(code.clone()),
            (None, Some(dir)) => libraries::open_unnarrated(dir, None, None)
                .map_err(|e| e.to_string())?
                .video
                .subject
                .clone(),
            (None, None) => None,
        };
        let lexicon = LexiconFiles::app(subject.as_deref());
        let mut reader = Reader::load(&kokoro::dir(), &lexicon)?;
        let read = reader.read(&args.text)?;

        if self.json {
            let files: Vec<_> = [&lexicon.library, &lexicon.subject]
                .into_iter()
                .flatten()
                .map(|p| p.to_string_lossy())
                .collect();
            return self.emit(&serde_json::json!({
                "text": args.text,
                "phonemes": read.phonemes,
                "subject": subject,
                "lexicon": files,
                "words": read.words.iter().map(|w| serde_json::json!({
                    "word": w.word,
                    "phonemes": w.phonemes,
                    "source": w.source.as_str(),
                    "guessed": w.guessed,
                })).collect::<Vec<_>>(),
            }));
        }
        let width = read
            .words
            .iter()
            .map(|w| w.word.chars().count())
            .max()
            .unwrap_or(0);
        for w in &read.words {
            let pad = width - w.word.chars().count();
            let source = match w.source {
                Source::Guess => {
                    let parts = if w.guessed.len() == 1 && w.guessed[0] == w.word {
                        String::new()
                    } else {
                        format!(" ({})", w.guessed.join(", "))
                    };
                    paint(&format!("guess{parts}: check pronunciation"), YELLOW)
                }
                other => paint(other.as_str(), DIM),
            };
            println!(
                "{}{}  /{}/  {source}",
                paint(&w.word, BOLD),
                " ".repeat(pad),
                w.phonemes
            );
        }
        println!("{}", paint(&format!("  {}", read.phonemes.trim()), DIM));
        let guessed = read
            .words
            .iter()
            .filter(|w| w.source == Source::Guess)
            .count();
        if guessed > 0 {
            println!(
                "{}",
                paint(
                    &format!(
                        "  {guessed} guessed; a lexicon.json entry fixes a wrong one: \"Word\": \"/phonemes/\" or \"Word\": \"RE-spell-ing\""
                    ),
                    DIM
                )
            );
        }
        Ok(())
    }
}
