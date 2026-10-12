//! The mechanical checks a director would otherwise make by eye on a
//! strip, as warnings: something past the frame's edge (`off_frame`), dead
//! air (`still`), everything in one half (`half`), and a leftover fragment
//! at the end (`fragment`). Each names the time span and the words said in
//! it. Thresholds are in `limits`.

mod fragment;
mod half;
mod off_frame;
mod still;

use super::entities::{Entity, entities};
use super::sample::SceneLook;

/// One warning about a scene.
#[derive(Clone, Debug)]
pub struct Warning {
    /// `off_frame`, `still`, `half_frame` or `fragment`.
    pub kind: &'static str,
    /// What one is called in a grouped line: `off-frame node`.
    pub noun: &'static str,
    /// It in a few words, for a grouped line.
    pub short: String,
    pub message: String,
}

impl SceneLook {
    /// Every warning, in the order of the checks.
    pub fn warnings(&self) -> Vec<Warning> {
        let shown: Vec<Vec<Entity>> = self
            .samples
            .iter()
            .map(|s| entities(s, self.size, &self.component))
            .collect();
        let mut out = Vec::new();
        off_frame::check(self, &shown, &mut out);
        still::check(self, &shown, &mut out);
        half::check(self, &shown, &mut out);
        fragment::check(self, &shown, &mut out);
        out
    }
}

/// `12.3 s`
pub(crate) fn secs(t: f64) -> String {
    format!("{t:.1} s")
}

/// `12.3-16.0 s`
pub(crate) fn span(a: f64, b: f64) -> String {
    format!("{a:.1}-{b:.1} s")
}
