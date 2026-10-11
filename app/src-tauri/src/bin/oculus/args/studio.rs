//! Arguments of the `studio` commands, which render a video folder on disk:
//! no database, no network.

use crate::*;

#[derive(Subcommand)]
pub(crate) enum StudioAction {
    Frame(StudioFrameArgs),
    Strip(StudioStripArgs),
    Lint(StudioLintArgs),
}

/// Render one frame of a video to a PNG
///
/// A video is a folder: `video.json` (size, theme and the scenes in order,
/// each a component, its props and how many seconds it holds),
/// `components/<Name>.tsx` (one TSX component per file, run in a sandbox),
/// `assets/` (the images `<Image src>` names) and, for a narrated video,
/// `script.md` and `narration/manifest.json`, which make a narrated scene
/// last as long as its narration. Every frame is a pure
/// function of the folder and the time, so the same call always draws the
/// same pixels.
///
/// Time is either global, `--t` seconds from the start of the video, or
/// scene-relative, `--scene N --at S` (scenes count from 0). With neither it
/// draws the first frame. The PNG goes to `<DIR>/out/frame-<t>.png`, `t`
/// global, unless `-o` names another file. Anything a component passes to
/// `console.log` is printed after the path.
///
///     oculus studio frame lectures/week-4 --t 12.4
///
///     oculus studio frame lectures/week-4 --scene 2 --at 0.5 -o /tmp/check.png --scale 0.5
#[derive(Args)]
#[command(group(clap::ArgGroup::new("when").args(["t", "scene"]).multiple(false)))]
pub(crate) struct StudioFrameArgs {
    /// The video folder, the one holding `video.json`
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
    /// Seconds from the start of the video (default: 0)
    #[arg(long = "t", value_name = "SECONDS", allow_negative_numbers = true)]
    pub(crate) t: Option<f64>,
    /// Which scene, counting from 0; the time is then `--at` within it
    #[arg(long, value_name = "N")]
    pub(crate) scene: Option<usize>,
    /// Seconds from the start of `--scene` (default: 0)
    #[arg(
        long,
        value_name = "SECONDS",
        requires = "scene",
        allow_negative_numbers = true
    )]
    pub(crate) at: Option<f64>,
    /// Where to write the PNG (default: `<DIR>/out/frame-<t>.png`)
    #[arg(short, long, value_name = "OUT.png")]
    pub(crate) out: Option<PathBuf>,
    /// Pixel scale over the same layout: 0.5 draws a 1080p video at 540p
    #[arg(long, value_name = "F", default_value_t = 1.0)]
    pub(crate) scale: f64,
    /// Average sub-frames over a 30 fps, 180° shutter, as many as the
    /// motion needs (off unless given, whatever the theme's motionBlur says)
    #[arg(long)]
    pub(crate) motion_blur: bool,
}

/// Render frames across a stretch of a video as one contact-sheet PNG
///
/// Draws `--n` frames evenly spaced from `--from` to `--to` (both
/// included, global seconds), tiled `--cols` a row, each labelled under
/// the frame with its time and, in a video of several scenes, its scene and
/// the time within it. It is how to check a motion without a video: what
/// moves, in which order, and where it settles. `--scene N` covers that
/// scene alone. A `--to` at or past the end is the video's last moment.
///
/// Frames render in parallel, one renderer per thread (default half the
/// cores, capped to keep memory under 2 GB); the sheet is the same at any
/// thread count. A frame that fails fails the sheet, naming its time and
/// scene. The PNG goes to `<DIR>/out/strip-<from>-<to>.png` unless `-o`
/// names another file.
///
///     oculus studio strip lectures/week-4
///
///     oculus studio strip lectures/week-4 --scene 2 --n 8 --cols 8 --scale 0.2
///
///     oculus studio strip lectures/week-4 --from 3 --to 4 --n 6 -o /tmp/motion.png
#[derive(Args)]
pub(crate) struct StudioStripArgs {
    /// The video folder, the one holding `video.json`
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
    /// First frame, in seconds from the start of the video (default: 0)
    #[arg(long, value_name = "SECONDS", conflicts_with = "scene")]
    pub(crate) from: Option<f64>,
    /// Last frame, in seconds from the start of the video (default: the end)
    #[arg(long, value_name = "SECONDS", conflicts_with = "scene")]
    pub(crate) to: Option<f64>,
    /// Cover only this scene, counting from 0
    #[arg(long, value_name = "N")]
    pub(crate) scene: Option<usize>,
    /// How many frames
    #[arg(long, value_name = "K", default_value_t = 12)]
    pub(crate) n: usize,
    /// Frames per row
    #[arg(long, value_name = "C", default_value_t = 4)]
    pub(crate) cols: usize,
    /// Each frame's scale over the video's size: 0.25 makes 480x270 tiles of
    /// a 1080p video
    #[arg(long, value_name = "F", default_value_t = 0.25)]
    pub(crate) scale: f64,
    /// Render threads (default: half the cores, at most one per frame)
    #[arg(long, value_name = "T")]
    pub(crate) threads: Option<usize>,
    /// Blur each frame over a 30 fps, 180° shutter (see `frame --help`)
    #[arg(long)]
    pub(crate) motion_blur: bool,
    /// Where to write the PNG (default: `<DIR>/out/strip-<from>-<to>.png`)
    #[arg(short, long, value_name = "OUT.png")]
    pub(crate) out: Option<PathBuf>,
}

/// Check a whole video without writing anything
///
/// Loads every component the video uses, checks each scene's props and each
/// component's `examples` against its `props` schema, and renders every scene
/// at its first and last moment. For a narrated video it also checks
/// `script.md` (sections no scene matches, text not written as it is said)
/// and the narration (missing or stale clips, clips over 60 s, words the
/// aligner was unsure of, unscripted speech). Every problem is reported, not
/// just the first, each with its scene, component and, where there is one,
/// the TSX line. Exits 1 when there is any problem, so it can gate a render.
///
/// See `oculus studio frame --help` for what a video folder holds.
#[derive(Args)]
pub(crate) struct StudioLintArgs {
    /// The video folder, the one holding `video.json`
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
}

#[cfg(test)]
mod tests {
    use crate::Cli;
    use clap::Parser;

    fn parses(args: &[&str]) -> bool {
        let argv = ["oculus", "studio", "strip", "video"].iter().chain(args);
        Cli::try_parse_from(argv).is_ok()
    }

    #[test]
    fn strip_scene_excludes_from_and_to() {
        assert!(parses(&[]));
        assert!(parses(&["--from", "1", "--to", "2.5", "--n", "6"]));
        assert!(parses(&["--scene", "2", "--cols", "3", "--motion-blur"]));
        assert!(!parses(&["--scene", "2", "--from", "1"]));
        assert!(!parses(&["--scene", "2", "--to", "3"]));
    }
}
