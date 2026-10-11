//! Digital silence: a TTS server that answers 200 with a clip of nothing
//! (Kokoro does for an unknown voice) has failed, not narrated a pause.

use std::fmt;

/// Whole-clip RMS below this (-80 dBFS) is silence. Speech sits near
/// -20 to -30 dBFS.
pub const SILENCE_RMS: f64 = 1e-4;

#[derive(Debug, Clone, PartialEq)]
pub enum Silence {
    /// The whole clip is below [`SILENCE_RMS`].
    Quiet { rms: f64 },
    /// At least a second of samples that are exactly zero, from `start`.
    ZeroRun { start: f64, seconds: f64 },
}

impl fmt::Display for Silence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Quiet { rms } => write!(f, "the clip is silent (RMS {rms:.1e})"),
            Self::ZeroRun { start, seconds } => write!(
                f,
                "the clip is digitally silent for {seconds:.1} s from {start:.1} s"
            ),
        }
    }
}

/// The first sign that mono `samples` at `rate` Hz are digitally silent:
/// the whole clip under [`SILENCE_RMS`], else the longest run of exact
/// zeros if it lasts a second or more. Spoken pauses are low but not zero.
pub fn silence(samples: &[f32], rate: u32) -> Option<Silence> {
    let rms = if samples.is_empty() {
        0.0
    } else {
        (samples.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
    };
    if rms < SILENCE_RMS {
        return Some(Silence::Quiet { rms });
    }
    let (mut best, mut best_start, mut run) = (0usize, 0usize, 0usize);
    for (i, &s) in samples.iter().enumerate() {
        run = if s == 0.0 { run + 1 } else { 0 };
        if run > best {
            best = run;
            best_start = i + 1 - run;
        }
    }
    (rate > 0 && best >= rate as usize).then(|| Silence::ZeroRun {
        start: best_start as f64 / rate as f64,
        seconds: best as f64 / rate as f64,
    })
}

pub fn is_silent(samples: &[f32], rate: u32) -> bool {
    silence(samples, rate).is_some()
}
