//! librespot's warnings and errors, on stderr like the rest of the app, and
//! the ones that mean a track will not be heard into the snapshot's error.
//! librespot reports a missing audio key only as a warning ("Unable to load
//! key, continuing without decryption") and goes on reporting `Playing` over
//! silence, so a log line is the only signal there is.
//!
//! The app has no other `log` logger; this one is installed when the first
//! session starts and passes nothing but librespot's own lines.

use std::sync::Once;

use log::{Level, LevelFilter, Log, Metadata, Record};
use tauri::AppHandle;
use tokio::sync::mpsc;

use super::state;

struct Logger {
    failures: mpsc::UnboundedSender<String>,
}

impl Log for Logger {
    fn enabled(&self, meta: &Metadata) -> bool {
        meta.target().starts_with("librespot") && meta.level() <= Level::Warn
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let msg = record.args().to_string();
        eprintln!("[music] {} {}: {msg}", record.level(), record.target());
        if is_track_failure(record.level(), record.target(), &msg) {
            self.failures.send(msg).ok();
        }
    }

    fn flush(&self) {}
}

/// Lines that mean a track will not play: audio-key, decrypt, load and
/// availability failures, and Login5's INVALID_CREDENTIALS, plus any error
/// from the player or the audio fetcher.
pub(super) fn is_track_failure(level: Level, target: &str, msg: &str) -> bool {
    const PATTERNS: &[&str] = &[
        "audio key",
        "invalid_credentials",
        "unable to load",
        "decrypt",
        "unavailable",
        "unable to read audio file",
        "cannot handle load",
        "should be available",
        "opening subfile",
        "unable to get cdn",
    ];
    let lower = msg.to_ascii_lowercase();
    PATTERNS.iter().any(|p| lower.contains(p))
        || (level == Level::Error
            && (target.starts_with("librespot_playback") || target.starts_with("librespot_audio")))
}

/// Installs the logger once per process; a failure lands in the snapshot as
/// "Spotify could not play a track: …".
pub(super) fn install(app: &AppHandle) {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let (tx, mut rx) = mpsc::unbounded_channel();
        if log::set_boxed_logger(Box::new(Logger { failures: tx })).is_err() {
            eprintln!("[music] another logger is installed; track failures will not be reported");
            return;
        }
        log::set_max_level(LevelFilter::Warn);
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(msg) = rx.recv().await {
                state::update(&app, |s| {
                    s.error = Some(format!("Spotify could not play a track: {msg}"));
                });
            }
        });
    });
}
