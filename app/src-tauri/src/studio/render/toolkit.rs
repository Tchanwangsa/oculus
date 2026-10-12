//! What the ffmpeg at hand can do, asked once per binary (`-encoders`,
//! `-decoders`, `-demuxers`), and the H.264 encoder each preset gets from
//! it. macOS's build has VideoToolbox and libx264; an LGPL Windows build has
//! Media Foundation's `h264_mf` and no libx264. A render that needs what the
//! binary lacks fails before its first frame, naming the missing part.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use super::error::RenderError;
use super::preset::Class;

/// The components a binary was built with, by name.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Toolkit {
    encoders: BTreeSet<String>,
    decoders: BTreeSet<String>,
    demuxers: BTreeSet<String>,
}

/// One H.264 encoder and its arguments, part of every segment's key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Codec {
    pub name: &'static str,
    pub args: Vec<String>,
}

/// Toolkits already probed, by binary.
static PROBED: Mutex<Option<HashMap<PathBuf, Arc<Toolkit>>>> = Mutex::new(None);

impl Toolkit {
    /// `ffmpeg`'s toolkit, asked on first use and kept for the process.
    pub fn of(ffmpeg: &Path) -> Result<Arc<Toolkit>, RenderError> {
        let mut probed = PROBED.lock().unwrap_or_else(PoisonError::into_inner);
        let probed = probed.get_or_insert_with(HashMap::new);
        if let Some(t) = probed.get(ffmpeg) {
            return Ok(t.clone());
        }
        let list = |what: &str| -> Result<String, RenderError> {
            let out = crate::runtime::platform::command(ffmpeg)
                .args(["-hide_banner", what])
                .output()
                .map_err(|e| RenderError::Ffmpeg(format!("could not run ffmpeg {what}: {e}")))?;
            if !out.status.success() {
                return Err(RenderError::Ffmpeg(format!(
                    "ffmpeg {what} exited {}: {}",
                    out.status,
                    String::from_utf8_lossy(&out.stderr).trim()
                )));
            }
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        };
        let toolkit = Arc::new(Self::parse(
            &list("-encoders")?,
            &list("-decoders")?,
            &list("-demuxers")?,
        ));
        probed.insert(ffmpeg.to_owned(), toolkit.clone());
        Ok(toolkit)
    }

    /// The three lists as ffmpeg prints them: a legend, a dashed line, then
    /// one component a line, flags first and the name (or comma-joined
    /// names, for demuxers) second.
    pub fn parse(encoders: &str, decoders: &str, demuxers: &str) -> Self {
        Self {
            encoders: names(encoders),
            decoders: names(decoders),
            demuxers: names(demuxers),
        }
    }

    pub fn has_encoder(&self, name: &str) -> bool {
        self.encoders.contains(name)
    }

    /// What reading raw frames and joining segments takes.
    pub fn check_frames(&self) -> Result<(), RenderError> {
        self.require(&[
            ("demuxer", "rawvideo", &self.demuxers),
            ("decoder", "rawvideo", &self.decoders),
            ("demuxer", "concat", &self.demuxers),
        ])
    }

    /// What encoding the narration track to AAC takes.
    pub fn check_audio(&self) -> Result<(), RenderError> {
        self.require(&[
            ("demuxer", "wav", &self.demuxers),
            ("decoder", "pcm_s16le", &self.decoders),
            ("encoder", "aac", &self.encoders),
        ])
    }

    fn require(&self, parts: &[(&str, &str, &BTreeSet<String>)]) -> Result<(), RenderError> {
        let missing: Vec<String> = parts
            .iter()
            .filter(|(_, name, set)| !set.contains(*name))
            .map(|(kind, name, _)| format!("the `{name}` {kind}"))
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        Err(RenderError::Render(format!(
            "this ffmpeg was built without {}, which Studio renders need; use an ffmpeg built with them",
            missing.join(" and ")
        )))
    }

    /// The encoder for `class` at `width` x `height` and `fps`. A fast
    /// render prefers VideoToolbox, then Media Foundation, then libx264's
    /// `veryfast`; a quality render prefers libx264 at its crf, then Media
    /// Foundation at a generous bitrate, then VideoToolbox at a high
    /// quality.
    pub fn codec(
        &self,
        class: Class,
        (width, height): (u32, u32),
        fps: u32,
    ) -> Result<Codec, RenderError> {
        let order: [&str; 3] = match class {
            Class::Fast => ["h264_videotoolbox", "h264_mf", "libx264"],
            Class::Quality { .. } => ["libx264", "h264_mf", "h264_videotoolbox"],
        };
        let name = order
            .into_iter()
            .find(|n| self.has_encoder(n))
            .ok_or_else(|| {
                RenderError::Render(format!(
                    "this ffmpeg has no H.264 encoder Studio can use ({}); use an ffmpeg built with one of them",
                    order.join(", ")
                ))
            })?;
        let args: Vec<String> = match (name, class) {
            // x264 gets a fixed 8 threads: its default (1.5 per core) held
            // 1.1 GB at 1080p on an 18-core Mac against 0.78 GB at 8, at
            // the same speed, and a fixed count makes its output the same
            // on every machine.
            ("libx264", Class::Fast) => {
                strings(&["-preset", "veryfast", "-crf", "23", "-threads", "8"])
            }
            ("libx264", Class::Quality { crf }) => {
                let crf = crf.to_string();
                strings(&["-preset", "medium", "-crf", &crf, "-threads", "8"])
            }
            // VideoToolbox takes a constant quality, 1 to 100.
            ("h264_videotoolbox", Class::Fast) => strings(&["-q:v", "65"]),
            ("h264_videotoolbox", Class::Quality { crf }) => {
                strings(&["-q:v", if crf <= 18 { "80" } else { "65" }])
            }
            // Media Foundation takes a bitrate: bits per pixel a frame.
            (_, class) => {
                let bpp = match class {
                    Class::Fast => 0.1,
                    Class::Quality { crf } if crf <= 18 => 0.3,
                    Class::Quality { .. } => 0.15,
                };
                let rate = f64::from(width) * f64::from(height) * f64::from(fps) * bpp;
                let kbps = format!("{}k", (rate / 1000.0).round() as u64);
                strings(&["-rate_control", "u_vbr", "-b:v", &kbps])
            }
        };
        let mut all = strings(&["-c:v", name]);
        all.extend(args);
        Ok(Codec { name, args: all })
    }
}

fn strings(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).to_owned()).collect()
}

/// The names after the dashed line of one `-encoders`-style list.
fn names(list: &str) -> BTreeSet<String> {
    list.lines()
        .skip_while(|l| !l.trim_start().starts_with("--"))
        .skip(1)
        .filter_map(|l| l.split_whitespace().nth(1))
        .flat_map(|n| n.split(','))
        .map(str::to_owned)
        .collect()
}
