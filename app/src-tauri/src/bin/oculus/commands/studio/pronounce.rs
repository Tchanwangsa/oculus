//! `oculus studio pronounce`: an agent's suggested readings, stored for the
//! student's review in Settings -> Voice
//! (`app_lib::studio::voice::review`).

use crate::*;
use app_lib::studio::libraries;
use app_lib::studio::tts::kokoro::{self, LexiconFiles, Reader};
use app_lib::studio::voice::review::{self, Item, Status, Store, Suggestion};
use std::path::Path;

impl Ctx {
    pub(crate) fn studio_pronounce(&self, action: &StudioPronounceAction) -> Result<(), String> {
        match action {
            StudioPronounceAction::Suggest(a) => self.pronounce_suggest(a),
            StudioPronounceAction::List(a) => self.pronounce_list(a),
        }
    }

    fn pronounce_suggest(&self, args: &StudioPronounceSuggestArgs) -> Result<(), String> {
        let project =
            libraries::open_unnarrated(&args.dir, None, None).map_err(|e| e.to_string())?;
        if project.script.is_none() {
            return Err(format!(
                "{} has no script.md: there is no narration to find the word in",
                args.dir.display()
            ));
        }
        let subject = project.video.subject.clone();
        let mut reader = Reader::load(&kokoro::dir(), &LexiconFiles::app(subject.as_deref()))?;
        let reading = args.reading.trim().to_owned();
        let phonemes = review::check("the reading", &args.word, &reading, reader.dict())?;
        let located = review::locate(&project, |text| reader.read(text), &args.word)?;
        let item = Store::app().suggest(Suggestion {
            reading,
            subject,
            video: slug(&args.dir),
            located,
        })?;

        if self.json {
            let mut value = serde_json::to_value(&item).map_err(|e| e.to_string())?;
            value["phonemes"] = phonemes.into();
            return self.emit(&value);
        }
        let r = &item.record;
        let now = if r.guessed {
            format!("guessed as /{}/", r.original)
        } else {
            format!("not a guessed word: Kokoro reads it /{}/ now", r.original)
        };
        println!(
            "{} \"{}\" for \"{}\" (/{phonemes}/); {now}",
            paint("suggested", GREEN),
            r.reading,
            r.word
        );
        println!(
            "{}",
            paint(
                &format!(
                    "  scene `{}` script.md:{}: \"{}\"",
                    r.scene, r.line, r.sentence
                ),
                DIM
            )
        );
        println!(
            "{}",
            paint(
                "  the student reviews it in Settings -> Voice; a decision applies at the next narrate",
                DIM
            )
        );
        Ok(())
    }

    fn pronounce_list(&self, args: &StudioPronounceListArgs) -> Result<(), String> {
        let video = args.video.as_deref().map(slug);
        let items = Store::app().list(|r| video.as_ref().is_none_or(|v| &r.video == v))?;
        if self.json {
            return self.emit(&items);
        }
        if items.is_empty() {
            println!("{}", paint("no pronunciation suggestions", DIM));
            return Ok(());
        }
        for item in &items {
            print_item(item);
        }
        Ok(())
    }
}

/// `pending  Nesterov  "NESS-ter-off"  COMP30027 · momentum · scene `intro` script.md:9`,
/// then a wrong one's note.
fn print_item(item: &Item) {
    let r = &item.record;
    let (status, colour) = match r.status {
        Status::Pending => ("pending", DIM),
        Status::Wrong => ("wrong", YELLOW),
    };
    println!(
        "{} {} \"{}\"  {}",
        paint(&format!("{status:<8}"), colour),
        paint(&r.word, BOLD),
        r.reading,
        paint(
            &format!(
                "{} · {} · scene `{}` script.md:{}",
                r.subject.as_deref().unwrap_or("no subject"),
                r.video,
                r.scene,
                r.line
            ),
            DIM
        )
    );
    if r.status == Status::Wrong {
        let note = match &r.note {
            Some(note) => format!("  the student says: \"{note}\"; suggest again"),
            None => "  no note; suggest another reading".to_owned(),
        };
        println!("{note}");
    }
}

/// A video folder's name: its slug.
fn slug(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.display().to_string())
}
