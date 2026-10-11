//! Arguments of the `studio` commands, which render a video folder on disk:
//! no database, no network.

use crate::*;

#[derive(Subcommand)]
pub(crate) enum StudioAction {
    Frame(StudioFrameArgs),
    Lint(StudioLintArgs),
}

/// Render one frame of a video to a PNG
///
/// A video is a folder: `video.json` (size, theme and the scenes in order,
/// each a component, its props and how many seconds it holds),
/// `components/<Name>.tsx` (one TSX component per file, run in a sandbox)
/// and `assets/` (the images `<Image src>` names). Every frame is a pure
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
}

/// Check a whole video without writing anything
///
/// Loads every component the video uses, checks each scene's props and each
/// component's `examples` against its `props` schema, and renders every scene
/// at its first and last moment. Every problem is reported, not just the
/// first, each with its scene, component and, where there is one, the TSX
/// line. Exits 1 when there is any problem, so it can gate a render.
///
/// See `oculus studio frame --help` for what a video folder holds.
#[derive(Args)]
pub(crate) struct StudioLintArgs {
    /// The video folder, the one holding `video.json`
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
}
