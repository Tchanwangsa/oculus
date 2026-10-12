//! Mono f32 samples back to WAV bytes, for a clip `narrate` spliced from
//! parts.

use std::io::Cursor;

/// `samples` as a 16-bit mono WAV at `rate` Hz. Scaled by 32768, the
/// inverse of `decode_wav`, so 16-bit input comes back sample for sample.
pub fn encode_wav(samples: &[f32], rate: u32) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let fail = |e: hound::Error| format!("writing the clip: {e}");
    let mut cursor = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut cursor, spec).map_err(fail)?;
    for &s in samples {
        let v = (f64::from(s) * 32768.0).round().clamp(-32768.0, 32767.0) as i16;
        writer.write_sample(v).map_err(fail)?;
    }
    writer.finalize().map_err(fail)?;
    Ok(cursor.into_inner())
}
