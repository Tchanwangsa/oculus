//! cpal's view of this Mac's output devices: the list the picker shows, a
//! lookup by name, and opening one as a rodio stream the way librespot's own
//! rodio backend does (stereo 44.1 kHz when offered, else the device's rate,
//! else its default config).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait};
use librespot_playback::{NUM_CHANNELS, SAMPLE_RATE};

use super::OutputDevice;

/// Every output device, the current default marked.
pub(in crate::music) fn list() -> Result<Vec<OutputDevice>, String> {
    let host = cpal::default_host();
    let default = host.default_output_device().and_then(|d| d.name().ok());
    let devices = host
        .output_devices()
        .map_err(|e| format!("Could not list the audio outputs ({e})"))?;
    Ok(devices
        .filter_map(|d| d.name().ok())
        .map(|name| OutputDevice {
            is_default: default.as_deref() == Some(name.as_str()),
            name,
        })
        .collect())
}

/// The names of the output devices present now.
pub(super) fn names() -> Vec<String> {
    cpal::default_host()
        .output_devices()
        .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

/// An open output: the rodio sink packets are appended to, and the stream
/// that plays it. `dead` turns true when cpal reports the device gone.
pub(super) struct Open {
    pub(super) sink: rodio::Sink,
    _stream: rodio::OutputStream,
    pub(super) dead: Arc<AtomicBool>,
}

/// Opens `name`, or the system default for `None`. The default is opened as
/// cpal's default device, which CoreAudio keeps on the Mac's default output.
pub(super) fn open(name: Option<&str>) -> Result<Open, String> {
    let host = cpal::default_host();
    let device = match name {
        Some(name) => host
            .output_devices()
            .map_err(|e| format!("Could not list the audio outputs ({e})"))?
            .find(|d| d.name().ok().as_deref() == Some(name))
            .ok_or_else(|| format!("{name} is not available"))?,
        None => host
            .default_output_device()
            .ok_or("This Mac has no audio output")?,
    };

    let dead = Arc::new(AtomicBool::new(false));
    let flag = dead.clone();
    let on_error = move |e: cpal::StreamError| {
        eprintln!("[music] audio output: {e}");
        if matches!(e, cpal::StreamError::DeviceNotAvailable) {
            flag.store(true, Ordering::Release);
        }
    };

    let fallback_err = |e: rodio::StreamError| format!("Could not open the audio output ({e})");
    let default_config = device
        .default_output_config()
        .map_err(|e| format!("Could not open the audio output ({e})"))?;
    let config = device
        .supported_output_configs()
        .ok()
        .and_then(|mut configs| configs.find(|c| c.channels() == NUM_CHANNELS as u16))
        .and_then(|c| {
            c.try_with_sample_rate(cpal::SampleRate(SAMPLE_RATE))
                .or_else(|| c.try_with_sample_rate(default_config.sample_rate()))
        })
        .unwrap_or(default_config);
    let mut stream = match rodio::OutputStreamBuilder::default()
        .with_device(device.clone())
        .with_config(&config.config())
        .with_sample_format(cpal::SampleFormat::F32)
        .with_error_callback(on_error.clone())
        .open_stream()
    {
        Ok(stream) => stream,
        Err(_) => rodio::OutputStreamBuilder::from_device(device)
            .map_err(fallback_err)?
            .with_error_callback(on_error)
            .open_stream_or_fallback()
            .map_err(fallback_err)?,
    };
    stream.log_on_drop(false);
    let sink = rodio::Sink::connect_new(stream.mixer());
    Ok(Open {
        sink,
        _stream: stream,
        dead,
    })
}
