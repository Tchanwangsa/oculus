mod assemble;
mod gaps;
mod model;
mod parity;
mod targets;
mod viterbi;

use super::targets::{label, BLANK, SEPARATOR, VOCAB};

/// A frame whose best label is `best`.
#[derive(Clone, Copy)]
pub(super) enum F {
    Blank,
    Sep,
    Ch(char),
}

/// Log-prob emissions, one row per frame: the frame's label at ln 0.6, the
/// blank at ln 0.3 when it is not the label, everything else at ln 0.001.
/// A forced path parks on blanks over frames it has no letter for.
pub(super) fn emissions(frames: &[F]) -> Vec<f32> {
    let mut em = Vec::with_capacity(frames.len() * VOCAB);
    for f in frames {
        let best = match *f {
            F::Blank => BLANK,
            F::Sep => SEPARATOR,
            F::Ch(c) => label(c).unwrap(),
        };
        em.extend((0..VOCAB).map(|k| match k {
            _ if k == best => 0.6f32.ln(),
            BLANK => 0.3f32.ln(),
            _ => 0.001f32.ln(),
        }));
    }
    em
}

/// `"C A T"`-style frame strings: `_` blank, `|` separator, a letter itself.
pub(super) fn frames(s: &str) -> Vec<F> {
    s.split_whitespace()
        .map(|t| match t {
            "_" => F::Blank,
            "|" => F::Sep,
            _ => F::Ch(t.chars().next().unwrap()),
        })
        .collect()
}

pub(super) fn words(ws: &[&str]) -> Vec<String> {
    ws.iter().map(|w| w.to_string()).collect()
}
