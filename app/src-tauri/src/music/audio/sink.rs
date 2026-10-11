//! The player's sink: librespot's rodio sink, rebuilt here so it can move to
//! another output mid-track. librespot builds its sink once, when the player
//! starts, and has no way to reopen it, so this one looks at the output
//! choice on every start, stop and write and reopens itself when the choice
//! changed, the device went away, or (while a chosen device is missing) every
//! two seconds to see whether it is back.
//!
//! It never returns an error from `stop`, which librespot answers with
//! `exit(1)`, and its wait for rodio to drain is bounded: a stream whose
//! device has gone stops pulling samples, and librespot's own sink then waits
//! forever.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use librespot_playback::audio_backend::{Sink, SinkError, SinkResult};
use librespot_playback::convert::Converter;
use librespot_playback::decoder::AudioPacket;
use librespot_playback::{NUM_CHANNELS, SAMPLE_RATE};

use super::device::{self, Open};
use super::{fallback_message, resolve, Choice};

/// rodio's queue is kept to about half a second, as librespot's sink keeps it.
const AHEAD: usize = 26;
const POLL: Duration = Duration::from_millis(10);
/// How often a missing device, or a failed open, is tried again.
const RECHECK: Duration = Duration::from_secs(2);
/// The longest a stop waits for queued audio to play out.
const DRAIN: Duration = Duration::from_secs(1);

/// What the sink tells the snapshot: a message to show, or one of its own to
/// take back.
pub(in crate::music) enum Note {
    Error(String),
    Cleared(String),
}

pub(in crate::music) type Report = Arc<dyn Fn(Note) + Send + Sync>;

pub(in crate::music) struct Switching {
    choice: Arc<Choice>,
    report: Report,
    open: Option<Open>,
    /// The device `open` plays to; `None` is the system default.
    bound: Option<String>,
    /// The choice generation last looked at.
    seen: Option<u64>,
    /// A chosen device is missing (or would not open); on the default meanwhile.
    missing: bool,
    next_try: Instant,
    running: bool,
    /// The message this sink last put in the snapshot.
    shown: Option<String>,
}

impl Switching {
    /// Built on the player's thread, when librespot calls the sink builder.
    pub(in crate::music) fn new(choice: Arc<Choice>, report: Report) -> Self {
        let mut sink = Self {
            choice,
            report,
            open: None,
            bound: None,
            seen: None,
            missing: false,
            next_try: Instant::now(),
            running: false,
            shown: None,
        };
        sink.refresh();
        sink
    }

    fn alive(&self) -> bool {
        self.open
            .as_ref()
            .is_some_and(|o| !o.dead.load(Ordering::Acquire))
    }

    /// Whether the output needs looking at: a new choice or a device gone at
    /// once; no output or a missing device every `RECHECK`.
    fn stale(&self) -> bool {
        if self.seen != Some(self.choice.gen()) {
            return true;
        }
        match &self.open {
            Some(open) if open.dead.load(Ordering::Acquire) => true,
            Some(_) if !self.missing => false,
            _ => Instant::now() >= self.next_try,
        }
    }

    fn refresh(&mut self) {
        if !self.stale() {
            return;
        }
        let (chosen, gen) = self.choice.get();
        self.seen = Some(gen);
        self.next_try = Instant::now() + RECHECK;
        let present = if chosen.is_some() {
            device::names()
        } else {
            Vec::new()
        };
        let (target, mut missing) = resolve(chosen.as_deref(), &present);

        let mut failure = None;
        if !(self.alive() && self.bound == target) {
            if let Err(why) = self.reopen(target.clone()) {
                if target.is_some() {
                    eprintln!("[music] {why}; playing on the system default");
                    missing = true;
                    if !(self.alive() && self.bound.is_none()) {
                        failure = self.reopen(None).err();
                    }
                } else {
                    failure = Some(why);
                }
            }
        }
        if !self.alive() {
            self.open = None;
            self.bound = None;
        }
        self.missing = missing;
        self.show(match (failure, missing) {
            (Some(why), _) => Some(format!("No audio output for Spotify: {why}")),
            (None, true) => chosen.as_deref().map(fallback_message),
            (None, false) => None,
        });
    }

    /// Opens `target` and swaps it in, so a failed open leaves the old
    /// stream playing.
    fn reopen(&mut self, target: Option<String>) -> Result<(), String> {
        let open = device::open(target.as_deref())?;
        if self.running {
            open.sink.play();
        } else {
            open.sink.pause();
        }
        eprintln!(
            "[music] audio output: {}",
            target.as_deref().unwrap_or("the system default")
        );
        self.open = Some(open);
        self.bound = target;
        Ok(())
    }

    fn show(&mut self, message: Option<String>) {
        if self.shown == message {
            return;
        }
        if let Some(old) = self.shown.take() {
            (self.report)(Note::Cleared(old));
        }
        if let Some(new) = &message {
            (self.report)(Note::Error(new.clone()));
        }
        self.shown = message;
    }

    fn no_output(&self) -> SinkError {
        SinkError::NotConnected(
            self.shown
                .clone()
                .unwrap_or_else(|| "No audio output for Spotify".into()),
        )
    }
}

impl Sink for Switching {
    fn start(&mut self) -> SinkResult<()> {
        self.running = true;
        self.refresh();
        match &self.open {
            Some(open) => {
                open.sink.play();
                Ok(())
            }
            None => {
                self.running = false;
                Err(self.no_output())
            }
        }
    }

    fn stop(&mut self) -> SinkResult<()> {
        self.running = false;
        if let Some(open) = &self.open {
            let until = Instant::now() + DRAIN;
            while !open.sink.empty() && !open.dead.load(Ordering::Acquire) && Instant::now() < until
            {
                thread::sleep(POLL);
            }
            open.sink.pause();
        }
        Ok(())
    }

    fn write(&mut self, packet: AudioPacket, converter: &mut Converter) -> SinkResult<()> {
        self.refresh();
        let samples = packet
            .samples()
            .map_err(|e| SinkError::OnWrite(e.to_string()))?;
        let Some(open) = &self.open else {
            return Err(self.no_output());
        };
        open.sink.append(rodio::buffer::SamplesBuffer::new(
            NUM_CHANNELS as u16,
            SAMPLE_RATE,
            converter.f64_to_f32(samples),
        ));
        // A reopened sink starts empty, which ends the wait.
        while self.open.as_ref().is_some_and(|o| o.sink.len() > AHEAD) {
            if self.stale() {
                self.refresh();
            } else {
                thread::sleep(POLL);
            }
        }
        Ok(())
    }
}
