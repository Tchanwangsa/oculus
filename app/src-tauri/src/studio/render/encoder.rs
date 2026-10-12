//! One segment's ffmpeg: frames written to its stdin in order, the mp4
//! written to `<out>.part` and renamed when ffmpeg exits cleanly. Dropped
//! unfinished (an error or a cancel), it kills ffmpeg and deletes the part.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

use super::error::RenderError;
use super::ffmpeg::{self, Tail, QUIET};
use super::quality::Settings;

pub(super) struct Encoder {
    child: Child,
    stdin: Option<ChildStdin>,
    tail: Option<Tail>,
    part: PathBuf,
    out: PathBuf,
    frame_bytes: usize,
    finished: bool,
}

impl Encoder {
    pub fn spawn(ffmpeg: &Path, settings: &Settings, out: &Path) -> Result<Self, RenderError> {
        let part = part_path(out);
        let mut child = ffmpeg::spawn(
            Command::new(ffmpeg)
                .args(QUIET)
                .args(settings.encoder_args(&part))
                .stdin(Stdio::piped()),
            "encoding",
        )?;
        let tail = Tail::spawn(child.stderr.take().expect("piped stderr"));
        Ok(Self {
            stdin: child.stdin.take(),
            child,
            tail: Some(tail),
            part,
            out: out.to_owned(),
            frame_bytes: settings.width as usize * settings.height as usize * 4,
            finished: false,
        })
    }

    /// One frame of straight RGBA. A frame of the wrong size, or ffmpeg
    /// gone (a closed pipe), fails with ffmpeg's own words when it has any.
    pub fn write(&mut self, rgba: &[u8]) -> Result<(), RenderError> {
        if rgba.len() != self.frame_bytes {
            return Err(RenderError::Render(format!(
                "a frame is {} bytes; the encoder expects {}",
                rgba.len(),
                self.frame_bytes
            )));
        }
        let stdin = self.stdin.as_mut().expect("stdin open until finish");
        if let Err(e) = stdin.write_all(rgba) {
            self.stdin = None;
            let status = match self.child.wait() {
                Ok(s) => s.to_string(),
                Err(_) => "did not exit".to_owned(),
            };
            let tail = self.tail.take().map(Tail::join).unwrap_or_default();
            return Err(RenderError::Ffmpeg(format!(
                "encoding: ffmpeg stopped reading frames ({e}); ffmpeg {status}:\n{tail}"
            )));
        }
        Ok(())
    }

    /// Close the input, wait for ffmpeg, and move the part into place.
    pub fn finish(mut self) -> Result<(), RenderError> {
        self.stdin = None;
        let status = self
            .child
            .wait()
            .map_err(|e| RenderError::Ffmpeg(format!("encoding: waiting for ffmpeg: {e}")))?;
        let tail = self.tail.take().map(Tail::join).unwrap_or_default();
        if !status.success() {
            return Err(ffmpeg::failed("encoding", &status.to_string(), &tail));
        }
        std::fs::rename(&self.part, &self.out).map_err(|e| RenderError::io(&self.out, e))?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        self.stdin = None;
        self.child.kill().ok();
        self.child.wait().ok();
        if let Some(tail) = self.tail.take() {
            tail.join();
        }
        std::fs::remove_file(&self.part).ok();
    }
}

/// `<out>.part`, beside it so the rename stays on one filesystem.
pub(super) fn part_path(out: &Path) -> PathBuf {
    let mut name = out.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}
