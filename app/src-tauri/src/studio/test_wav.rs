//! WAV bytes for the narration tests.

use std::io::Cursor;

/// 16-bit mono at `rate` Hz, `seconds` long, each sample from `sample(i)`.
pub(crate) fn wav_of(rate: u32, seconds: f64, sample: impl Fn(usize) -> i16) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
        for i in 0..(rate as f64 * seconds) as usize {
            writer.write_sample(sample(i)).unwrap();
        }
        writer.finalize().unwrap();
    }
    cursor.into_inner()
}

/// A 24 kHz 220 Hz tone at about -12 dBFS, as Kokoro's rate and level.
pub(crate) fn tone(seconds: f64) -> Vec<u8> {
    wav_of(24_000, seconds, |i| {
        ((i as f64 * 220.0 * std::f64::consts::TAU / 24_000.0).sin() * 8_000.0) as i16
    })
}
