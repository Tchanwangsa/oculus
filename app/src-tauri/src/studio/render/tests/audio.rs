use std::path::PathBuf;

use super::super::audio::{self, Clip, RATE};
use crate::studio::test_wav::wav_of;
use crate::test_support::Scratch;

#[test]
fn offsets_and_lengths_are_whole_samples_at_48k() {
    assert_eq!(audio::offset(0.0), 0);
    assert_eq!(audio::offset(0.25), 12_000);
    assert_eq!(audio::offset(1.0 / 3.0), 16_000);
    assert_eq!(audio::length(90, 30), 144_000);
    assert_eq!(audio::length(25, 24), 50_000);
}

#[test]
fn placing_adds_from_the_offset_and_cuts_at_the_end() {
    let mut track = vec![0.0f32; 10];
    audio::place(&mut track, &[1.0, 2.0, 3.0], 2);
    audio::place(&mut track, &[5.0, 5.0, 5.0], 8);
    audio::place(&mut track, &[9.0], 12);
    assert_eq!(track, [0.0, 0.0, 1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 5.0, 5.0]);
}

/// Where the first sample louder than `level` is.
fn onset(track: &[f32], level: f32) -> usize {
    track.iter().position(|s| s.abs() > level).unwrap()
}

#[test]
fn a_24k_clip_lands_on_its_sample_after_resampling() {
    let dir = Scratch::new("render-mix");
    // Silence, then a full-scale square wave from 0.1 s.
    let wav = wav_of(24_000, 0.5, |i| {
        if i < 2_400 {
            0
        } else if (i / 20) % 2 == 0 {
            16_000
        } else {
            -16_000
        }
    });
    let path = dir.join("clip.wav");
    std::fs::write(&path, wav).unwrap();
    let clips = [
        Clip {
            path: path.clone(),
            at: 0.25,
        },
        Clip { path, at: 1.0 },
    ];
    let track = audio::mix(&clips, RATE as usize * 2).unwrap();
    assert_eq!(track.len(), 96_000);
    // 0.25 s + 0.1 s = 16,800; the resampler's ringing starts a few
    // samples before the edge.
    let first = onset(&track, 0.05);
    assert!((16_790..=16_802).contains(&first), "{first}");
    let second = onset(&track[48_000..], 0.05) + 48_000;
    assert_eq!(second - first, 48_000 - 12_000);
    assert!(track[..16_700].iter().all(|s| s.abs() < 1e-3));
}

#[test]
fn the_key_names_the_clips_offsets_and_length() {
    let clip = |name: &str, at| Clip {
        path: PathBuf::from(format!("/n/{name}")),
        at,
    };
    let base = audio::key(&[clip("a.wav", 1.0)], 100);
    assert_eq!(base, audio::key(&[clip("a.wav", 1.0)], 100));
    assert_ne!(base, audio::key(&[clip("b.wav", 1.0)], 100));
    assert_ne!(base, audio::key(&[clip("a.wav", 1.1)], 100));
    assert_ne!(base, audio::key(&[clip("a.wav", 1.0)], 101));
}

#[test]
fn the_written_track_reads_back_as_16_bit_48k_mono() {
    let dir = Scratch::new("render-wav");
    let path = dir.join("t.wav");
    audio::write(&path, &[0.0, 0.5, -1.5, 1.0]).unwrap();
    let reader = hound::WavReader::open(&path).unwrap();
    let spec = reader.spec();
    assert_eq!(
        (spec.channels, spec.sample_rate, spec.bits_per_sample),
        (1, 48_000, 16)
    );
    let samples: Vec<i16> = reader.into_samples().map(Result::unwrap).collect();
    assert_eq!(samples, [0, 16_384, -32_767, 32_767]);
    assert!(!dir.join("t.wav.part").exists());
}
