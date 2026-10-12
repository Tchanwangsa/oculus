use std::io::Cursor;

use super::*;
use crate::studio::NarrationError;

fn wav(
    spec: hound::WavSpec,
    write: impl FnOnce(&mut hound::WavWriter<&mut Cursor<Vec<u8>>>),
) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
        write(&mut writer);
        writer.finalize().unwrap();
    }
    cursor.into_inner()
}

#[test]
fn an_i16_stereo_wav_decodes_to_mono_in_unit_range() {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 24_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let bytes = wav(spec, |w| {
        for _ in 0..24_000 {
            w.write_sample(16_384i16).unwrap();
            w.write_sample(-16_384i16 / 2).unwrap();
        }
    });
    let decoded = decode_wav(&bytes).unwrap();
    assert_eq!(decoded.sample_rate, 24_000);
    assert_eq!(decoded.samples.len(), 24_000);
    assert_eq!(decoded.duration, 1.0);
    assert!(
        (decoded.samples[0] - 0.125).abs() < 1e-6,
        "{}",
        decoded.samples[0]
    );
}

#[test]
fn an_i24_wav_scales_by_its_own_width() {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let bytes = wav(spec, |w| {
        w.write_sample(1i32 << 22).unwrap();
        w.write_sample(-(1i32 << 23)).unwrap();
    });
    let decoded = decode_wav(&bytes).unwrap();
    assert_eq!(decoded.samples, vec![0.5, -1.0]);
}

#[test]
fn an_f32_wav_keeps_its_samples() {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 22_050,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let bytes = wav(spec, |w| {
        for s in [0.25f32, -0.5, 0.75] {
            w.write_sample(s).unwrap();
        }
    });
    let decoded = decode_wav(&bytes).unwrap();
    assert_eq!(decoded.samples, vec![0.25, -0.5, 0.75]);
    assert_eq!(decoded.sample_rate, 22_050);
    assert!((decoded.duration - 3.0 / 22_050.0).abs() < 1e-12);
}

#[test]
fn bytes_that_are_not_a_wav_are_a_decode_error() {
    assert!(matches!(decode_wav(b""), Err(NarrationError::Decode(_))));
    assert!(matches!(
        decode_wav(b"ID3\x04\x00 an mp3"),
        Err(NarrationError::Decode(_))
    ));
}

#[test]
fn resampling_to_16k_keeps_the_duration() {
    let tone = |rate: u32, seconds: f64| -> Vec<f32> {
        (0..(rate as f64 * seconds) as usize)
            .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.3)
            .collect()
    };
    for (rate, seconds) in [(24_000, 1.5), (44_100, 2.0), (22_050, 0.7)] {
        let out = resample_to_16k(&tone(rate, seconds), rate).unwrap();
        let expected = (seconds * ALIGN_RATE as f64).round() as usize;
        assert!(
            out.len().abs_diff(expected) <= 1,
            "{rate} Hz, {seconds} s: {} samples, expected {expected}",
            out.len()
        );
    }
    let same = tone(16_000, 0.5);
    assert_eq!(resample_to_16k(&same, 16_000).unwrap(), same);
    assert!(resample_to_16k(&[], 24_000).unwrap().is_empty());
}

#[test]
fn silence_is_a_quiet_clip_or_a_second_of_exact_zeros() {
    let rate = 16_000;
    let speech: Vec<f32> = (0..rate * 2)
        .map(|i| ((i as f32) * 0.05).sin() * 0.2)
        .collect();
    assert_eq!(silence(&speech, rate), None);
    assert!(!is_silent(&speech, rate));

    assert!(matches!(
        silence(&vec![0.0; rate as usize], rate),
        Some(Silence::Quiet { .. })
    ));
    assert!(matches!(silence(&[], rate), Some(Silence::Quiet { .. })));
    assert!(matches!(
        silence(&vec![1e-6; rate as usize], rate),
        Some(Silence::Quiet { .. })
    ));

    // A held second of zeros inside speech.
    let mut gapped = speech.clone();
    gapped.splice(8_000..8_000, vec![0.0; rate as usize]);
    assert_eq!(
        silence(&gapped, rate),
        Some(Silence::ZeroRun {
            start: 0.5,
            seconds: 1.0
        })
    );
    // Just under a second of zeros is a pause.
    let mut paused = speech.clone();
    paused.splice(8_000..8_000, vec![0.0; rate as usize - 1]);
    assert_eq!(silence(&paused, rate), None);
}

#[test]
fn an_encoded_clip_decodes_to_its_16_bit_samples() {
    let samples: Vec<f32> = [-32768i16, -1, 0, 1, 16_384, 32_767]
        .iter()
        .map(|&v| f32::from(v) / 32768.0)
        .collect();
    let back = decode_wav(&encode_wav(&samples, 24_000).unwrap()).unwrap();
    assert_eq!(back.sample_rate, 24_000);
    assert_eq!(back.samples, samples);
    let loud = decode_wav(&encode_wav(&[2.0, -2.0], 24_000).unwrap()).unwrap();
    assert_eq!(loud.samples, [32_767.0 / 32768.0, -1.0], "clamped");
}
