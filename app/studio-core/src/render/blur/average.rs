//! The average of straight-alpha RGBA8 sub-frames, taken premultiplied in
//! integers so it is exact and the same on every machine: colour is
//! `Σ c·a / Σ a` and alpha `Σ a / N`, each rounded half up.

use crate::render::Frame;

pub struct Accumulator {
    width: u32,
    height: u32,
    /// Per pixel: Σ r·a, Σ g·a, Σ b·a, Σ a.
    sums: Vec<u32>,
}

impl Accumulator {
    pub fn new(width: u32, height: u32) -> Accumulator {
        Accumulator {
            width,
            height,
            sums: vec![0; width as usize * height as usize * 4],
        }
    }

    pub fn add(&mut self, rgba: &[u8]) {
        for (s, px) in self
            .sums
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(rgba.as_chunks::<4>().0)
        {
            let a = u32::from(px[3]);
            s[0] += u32::from(px[0]) * a;
            s[1] += u32::from(px[1]) * a;
            s[2] += u32::from(px[2]) * a;
            s[3] += a;
        }
    }

    pub fn finish(self, logs: Vec<String>, n: u32) -> Frame {
        let mut rgba = vec![0u8; self.sums.len()];
        for (px, s) in rgba
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(self.sums.as_chunks::<4>().0)
        {
            let sa = s[3];
            if sa == 0 {
                continue;
            }
            for c in 0..3 {
                px[c] = ((s[c] + sa / 2) / sa).min(255) as u8;
            }
            px[3] = ((sa + n / 2) / n).min(255) as u8;
        }
        Frame {
            width: self.width,
            height: self.height,
            rgba,
            logs,
            samples: n,
        }
    }
}
