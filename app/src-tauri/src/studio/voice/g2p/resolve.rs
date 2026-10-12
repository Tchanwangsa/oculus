//! The unit loop: tag, split into sub-tokens, read every word from right
//! to left (a word's reading can depend on the next sound), then join.
//! misaki's `G2P.__call__` with the oracle's one change: a sub-token that
//! nothing reads is guessed on its own, so `PyTorch` guesses only `Py`.

use super::chars::has_letter;
use super::split::{
    merged_phonemes, merged_tag, merged_text, resolve_tokens, retokenize, Word, SUBTOKEN_JUNKS, UNK,
};
use super::tag::{preprocess, tokenize};
use super::token::{token_context, Ctx, Token};
use super::word::{guessable, word_phonemes};
use super::{Dictionary, Guess, Lexicon, Phonemized, Source, WordRecord};

/// Who answered for a word, for its record.
#[derive(Default)]
struct Events {
    guessed: Vec<String>,
    lexicon: bool,
}

struct Reader<'a> {
    dict: &'a Dictionary,
    lexicon: &'a Lexicon,
    guesser: &'a mut dyn Guess,
}

impl Reader<'_> {
    /// The lexicon, then the dictionary.
    fn read(&self, text: &str, tag: &str, ctx: &Ctx, ev: &mut Events) -> Option<String> {
        if let Some(ps) = self.lexicon.phonemes(text) {
            ev.lexicon = true;
            return Some(ps);
        }
        word_phonemes(self.dict, text, tag, ctx)
    }

    /// The guesser, for something with a letter in it; an empty guess is no
    /// reading.
    fn guess(&mut self, text: &str, ev: &mut Events) -> Result<Option<String>, String> {
        if !guessable(text) {
            return Ok(None);
        }
        let ps = self.guesser.guess(text)?;
        ev.guessed.push(text.to_string());
        Ok(Some(ps).filter(|p| !p.is_empty()))
    }

    /// A word's sub-tokens: the widest window the lexicon or dictionary
    /// reads, right to left, then each part left over on its own (junk
    /// reads as nothing, the rest is guessed).
    fn read_parts(
        &mut self,
        w: &mut [Token],
        mut ctx: Ctx,
        ev: &mut Events,
    ) -> Result<Ctx, String> {
        let (mut left, mut right) = (0, w.len());
        while left < right {
            let window = &w[left..right];
            let mut read = None;
            if window.iter().all(|tk| tk.phonemes.is_none()) {
                let text = merged_text(window);
                let tag = merged_tag(window);
                if let Some(ps) = self.read(&text, &tag, &ctx, ev) {
                    read = Some((ps, text, tag));
                }
            }
            if let Some((ps, text, tag)) = read {
                ctx = token_context(ctx, Some(&ps), &text, &tag);
                w[left].phonemes = Some(ps);
                for x in &mut w[left + 1..right] {
                    x.phonemes = Some(String::new());
                }
                right = left;
                left = 0;
            } else if left + 1 < right {
                left += 1;
            } else {
                right -= 1;
                if w[right].phonemes.is_none() {
                    let text = w[right].text.clone();
                    w[right].phonemes = if text.chars().all(|c| SUBTOKEN_JUNKS.contains(c)) {
                        Some(String::new())
                    } else {
                        self.guess(&text, ev)?
                    };
                }
                left = 0;
            }
        }
        Ok(ctx)
    }
}

pub fn phonemize(
    text: &str,
    dict: &Dictionary,
    lexicon: &Lexicon,
    guesser: &mut dyn Guess,
) -> Result<Phonemized, String> {
    let text = preprocess(text);
    let tagged = tokenize(&text, dict);
    let mut words = retokenize(tagged.tokens);
    let mut reader = Reader {
        dict,
        lexicon,
        guesser,
    };
    let mut events: Vec<Events> = words.iter().map(|_| Events::default()).collect();
    let mut ctx = Ctx::default();
    for (w, ev) in words.iter_mut().zip(events.iter_mut()).rev() {
        match w {
            Word::One(tk) => {
                if tk.phonemes.is_none() {
                    tk.phonemes = reader.read(&tk.text, &tk.tag, &ctx, ev);
                }
                if tk.phonemes.is_none() {
                    tk.phonemes = reader.guess(&tk.text, ev)?;
                }
                ctx = token_context(ctx, tk.phonemes.as_deref(), &tk.text, &tk.tag);
            }
            Word::Parts(parts) => {
                ctx = reader.read_parts(parts, ctx, ev)?;
                resolve_tokens(parts);
            }
        }
    }

    let mut phonemes = String::new();
    let mut records = Vec::new();
    for (w, ev) in words.into_iter().zip(events) {
        let (word, ps, whitespace) = match w {
            Word::One(tk) => (tk.text, tk.phonemes, tk.whitespace),
            Word::Parts(parts) => {
                let whitespace = parts
                    .last()
                    .map(|t| t.whitespace.clone())
                    .unwrap_or_default();
                (
                    merged_text(&parts),
                    Some(merged_phonemes(&parts)),
                    whitespace,
                )
            }
        };
        let ps = ps.map(|p| p.replace('ɾ', "T").replace('ʔ', "t"));
        phonemes.push_str(ps.as_deref().unwrap_or(""));
        phonemes.push_str(&whitespace);
        if !has_letter(&word) {
            continue;
        }
        let said = ps
            .as_deref()
            .map(|p| p.replace(UNK, ""))
            .unwrap_or_default();
        let said = said.trim();
        if said.is_empty() || ps.as_deref().is_some_and(|p| p.contains(UNK)) {
            return Err(format!("no phonemes for {word:?} in {text:?}"));
        }
        let source = if !ev.guessed.is_empty() {
            Source::Guess
        } else if ev.lexicon {
            Source::Lexicon
        } else if tagged.rules.contains(&word) {
            Source::Rule
        } else {
            Source::Dict
        };
        records.push(WordRecord {
            word,
            phonemes: said.to_string(),
            source,
            guessed: ev.guessed,
        });
    }
    Ok(Phonemized {
        phonemes: phonemes.replace(UNK, ""),
        words: records,
    })
}
