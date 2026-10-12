use super::super::chunk::Chunk;
use super::super::synth::{render, wav, Synth, SAMPLE_RATE};
use crate::studio::audio::decode_wav;

/// A quarter second of tone per input; silence for an input whose first
/// id is 0.
struct Fake {
    runs: usize,
}

impl Synth for Fake {
    fn run(&mut self, ids: &[i64]) -> Result<Vec<f32>, String> {
        self.runs += 1;
        let n = SAMPLE_RATE as usize / 4;
        if ids[0] == 0 {
            return Ok(vec![0.0; n]);
        }
        Ok((0..n).map(|i| (i as f32 * 0.05).sin() * 0.3).collect())
    }
}

fn chunk(first: i64, words: &str) -> Chunk {
    Chunk {
        ids: vec![first, 50, 51],
        words: words.to_owned(),
    }
}

#[test]
fn inputs_join_with_no_gap() {
    let mut fake = Fake { runs: 0 };
    let audio = render(&[chunk(43, "one"), chunk(44, "two")], &mut fake).unwrap();
    assert_eq!(fake.runs, 2);
    assert_eq!(audio.len(), SAMPLE_RATE as usize / 2);
}

#[test]
fn a_silent_input_fails_naming_its_words() {
    let mut fake = Fake { runs: 0 };
    let e = render(
        &[
            chunk(43, "we begin"),
            chunk(0, "then nothing"),
            chunk(44, "never"),
        ],
        &mut fake,
    )
    .unwrap_err();
    assert!(
        e.contains("part 2 of 3") && e.contains("\"then nothing\""),
        "{e}"
    );
    assert_eq!(fake.runs, 2, "stops at the silent one");
}

#[test]
fn the_clip_is_a_24_khz_16_bit_wav() {
    let samples: Vec<f32> = (0..2400).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
    let bytes = wav(&samples).unwrap();
    let d = decode_wav(&bytes).unwrap();
    assert_eq!(d.sample_rate, 24_000);
    assert_eq!(d.samples.len(), 2400);
    assert!((d.duration - 0.1).abs() < 1e-9);
    let worst = samples
        .iter()
        .zip(&d.samples)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(worst < 1e-4, "{worst}");
    let clipped = decode_wav(&wav(&[2.0, -2.0]).unwrap()).unwrap();
    assert!(clipped.samples[0] > 0.99 && clipped.samples[1] < -0.99);
}
