//! The guesser: phonemes for a word or sub-token that no lexicon entry,
//! dictionary entry or rule resolves, from a 3 MB BART (one encoder and one
//! decoder layer, d_model 128) run as two ONNX sessions on the CPU. The
//! encoder reads the word once; the decoder is rerun on the whole output so
//! far for each step (the export has no KV cache), and the highest logit is
//! taken until the stop id. The first id taken is always the start id again
//! (BART's targets begin with it): it goes back into the decoder but is no
//! phoneme. Matches the oracle's `BartG2P`
//! (data/evals/studio-g2p/routes/c-neural/bart_g2p.py).

mod model;
#[cfg(test)]
mod tests;
mod vocab;

use std::path::Path;

use ort::session::Session;
use ort::value::Tensor;

use super::Guess;
use crate::runtime::onnx;
pub use model::{dir, CONFIG_FILE, DECODER_FILE, ENCODER_FILE, GOLD_FILE, PACK, SILVER_FILE};
use vocab::{normalise, Vocab, START, STOP};

/// Positions the model has (`max_position_embeddings`): an encoded word
/// may take all of them, a decode one less, since the start id takes one.
const POSITIONS: usize = 64;
/// Most decoder steps one guess runs, the first being the start id.
pub const MAX_STEPS: usize = POSITIONS - 1;

pub struct Guesser {
    encoder: Session,
    decoder: Session,
    vocab: Vocab,
}

/// One decode step: the id taken, and how far its logit is above the
/// runner-up's (a near-tie could go the other way on another ORT build).
#[derive(Debug, Clone, Copy)]
struct Step {
    id: i64,
    /// Read by the golden test's report.
    #[cfg_attr(not(test), allow(dead_code))]
    margin: f32,
}

impl Guesser {
    /// The encoder, decoder and alphabet from a `g2p-en-us` pack folder.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let config = dir.join(CONFIG_FILE);
        let json =
            std::fs::read_to_string(&config).map_err(|e| format!("{}: {e}", config.display()))?;
        let vocab = Vocab::parse(&json).map_err(|e| format!("{}: {e}", config.display()))?;
        Ok(Self {
            encoder: onnx::session(&dir.join(ENCODER_FILE))?,
            decoder: onnx::session(&dir.join(DECODER_FILE))?,
            vocab,
        })
    }

    /// The greedy decode of encoder input `ids`: every step, the last one
    /// being the stop id unless [`MAX_STEPS`] ran out first.
    fn decode(&mut self, ids: &[i64]) -> Result<Vec<Step>, String> {
        let fail = |e: ort::Error| e.to_string();
        let s = ids.len();
        let mask = vec![1i64; s];
        let (width, hidden) = {
            let outputs = self
                .encoder
                .run(ort::inputs![
                    "input_ids" => Tensor::from_array(([1usize, s], ids.to_vec())).map_err(fail)?,
                    "attention_mask" => Tensor::from_array(([1usize, s], mask.clone())).map_err(fail)?,
                ])
                .map_err(fail)?;
            let (shape, data) = outputs["last_hidden_state"]
                .try_extract_tensor::<f32>()
                .map_err(fail)?;
            if shape.len() != 3 || shape[0] != 1 || shape[1] as usize != s {
                return Err(format!("encoder output has shape {shape:?}"));
            }
            (shape[2] as usize, data.to_vec())
        };
        let mut out = vec![START];
        let mut steps = Vec::new();
        while steps.len() < MAX_STEPS {
            let t = out.len();
            let outputs = self
                .decoder
                .run(ort::inputs![
                    "input_ids" => Tensor::from_array(([1usize, t], out.clone())).map_err(fail)?,
                    "encoder_hidden_states" => Tensor::from_array(([1usize, s, width], hidden.clone())).map_err(fail)?,
                    "encoder_attention_mask" => Tensor::from_array(([1usize, s], mask.clone())).map_err(fail)?,
                ])
                .map_err(fail)?;
            let (shape, logits) = outputs["logits"]
                .try_extract_tensor::<f32>()
                .map_err(fail)?;
            if shape.len() != 3 || shape[0] != 1 || shape[1] as usize != t || shape[2] < 2 {
                return Err(format!("decoder output has shape {shape:?}"));
            }
            let vocab = shape[2] as usize;
            let step = best(&logits[(t - 1) * vocab..t * vocab]);
            steps.push(step);
            if step.id == STOP {
                break;
            }
            out.push(step.id);
        }
        Ok(steps)
    }
}

impl Guess for Guesser {
    /// Phonemes for one word or sub-token. A guess that runs [`MAX_STEPS`]
    /// without stopping is kept as it is, like the oracle's.
    fn guess(&mut self, part: &str) -> Result<String, String> {
        let ids = self.vocab.encode(&normalise(part));
        if ids.len() > POSITIONS {
            return Err(format!(
                "{part:?} is too long to guess: the guesser takes at most {} characters",
                POSITIONS - 2
            ));
        }
        let steps = self.decode(&ids)?;
        let phonemes = steps
            .iter()
            .skip(usize::from(steps.first().is_some_and(|s| s.id == START)))
            .take_while(|s| s.id != STOP)
            .map(|s| {
                self.vocab.phoneme(s.id).ok_or_else(|| {
                    format!(
                        "the guesser answered id {} for {part:?}, which is not a phoneme",
                        s.id
                    )
                })
            })
            .collect::<Result<String, String>>()?;
        if phonemes.is_empty() {
            return Err(format!("the guesser has no phonemes for {part:?}"));
        }
        Ok(phonemes)
    }
}

/// The highest logit's id (the first on a tie, like numpy's `argmax`) and
/// its margin over the next highest.
fn best(logits: &[f32]) -> Step {
    let mut id = 0;
    for (i, &x) in logits.iter().enumerate() {
        if x > logits[id] {
            id = i;
        }
    }
    let runner_up = logits
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != id)
        .map(|(_, &x)| x)
        .fold(f32::NEG_INFINITY, f32::max);
    Step {
        id: id as i64,
        margin: logits[id] - runner_up,
    }
}
