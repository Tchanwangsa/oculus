//! Notes the track Oculus starts playing, and writes it into the row off
//! the runtime. A track is noted when Oculus, the device playing, plays a
//! `spotify:track:` it was not playing at the last change (a new track, a
//! play after a pause), unless the same account noted the same track under
//! `AGAIN_AFTER_MS` ago, so pausing and resuming writes nothing new.

use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use super::{encode, is_track, parse, record, History, Played, SETTING};
use crate::music::state::Snapshot;

/// How long after noting a track playing it again notes it again.
pub(in crate::music) const AGAIN_AFTER_MS: u64 = 60_000;

#[derive(Debug, Default)]
pub(in crate::music) struct Recorder {
    /// The account and track Oculus was playing at the last change.
    playing: Option<(String, String)>,
    /// The last track noted: its account, URI and time.
    noted: Option<(String, String, u64)>,
    /// Held across a write's read and write, so two writes never interleave.
    writing: Arc<Mutex<()>>,
}

impl Recorder {
    /// Follows the snapshot after a change, writing the row when a track is
    /// due (`due`).
    pub(in crate::music) fn follow(&mut self, s: &Snapshot, now_ms: u64) -> Option<JoinHandle<()>> {
        let (account, played) = self.due(s, now_ms)?;
        Some(self.save(account, played))
    }

    /// The account and track to note after this change, if any.
    pub(in crate::music) fn due(&mut self, s: &Snapshot, now_ms: u64) -> Option<(String, Played)> {
        let track = s.track.as_ref().map(|t| t.uri.as_str());
        let now = match (s.active && s.playing, &s.account, track) {
            (true, Some(account), Some(uri)) if is_track(uri) => {
                Some((account.clone(), uri.to_string()))
            }
            _ => None,
        };
        let before = std::mem::replace(&mut self.playing, now.clone());
        let (account, uri) = now?;
        if before.as_ref() == Some(&(account.clone(), uri.clone())) {
            return None;
        }
        if let Some((a, u, at)) = &self.noted {
            if *a == account && *u == uri && now_ms.saturating_sub(*at) < AGAIN_AFTER_MS {
                return None;
            }
        }
        self.noted = Some((account.clone(), uri.clone(), now_ms));
        Some((
            account,
            Played {
                uri,
                played_at_ms: now_ms,
            },
        ))
    }

    /// Reads the row, notes `played` in it and writes it back, on a thread
    /// of its own. A row for another account is replaced.
    fn save(&self, account: String, played: Played) -> JoinHandle<()> {
        let writing = self.writing.clone();
        std::thread::spawn(move || {
            let _held = writing.lock().unwrap_or_else(|e| e.into_inner());
            let raw = crate::db::store::setting_blocking(SETTING);
            let items = record(parse(raw.as_deref(), &account), played);
            let text = encode(&History { account, items });
            if let Err(why) = crate::db::store::set_setting_blocking(SETTING, &text) {
                elog!("[music] could not remember a played track: {why}");
            }
        })
    }
}
