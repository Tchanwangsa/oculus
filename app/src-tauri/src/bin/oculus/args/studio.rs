//! Arguments of the `studio` commands. `frame`, `strip`, `lint` and
//! `render` read a video folder on disk (no database, no network); `narrate`
//! and `tts` call the TTS server, and `aligner` manages the word-timing
//! model.

use crate::*;

#[derive(Subcommand)]
pub(crate) enum StudioAction {
    Frame(StudioFrameArgs),
    Strip(StudioStripArgs),
    Lint(StudioLintArgs),
    Gallery(StudioGalleryArgs),
    Components {
        #[command(subcommand)]
        action: StudioComponentsAction,
    },
    Render(StudioRenderArgs),
    Narrate(StudioNarrateArgs),
    Tts(StudioTtsArgs),
    /// Download, check or delete the narration aligner model (378 MB)
    ///
    /// `narrate` times each clip's words by forced alignment with a
    /// wav2vec2 model run on the CPU. It is downloaded once, on request, to
    /// `<app data>/models/align/`, checked against a pinned size and sha256,
    /// and never bundled with the app.
    Aligner {
        #[command(subcommand)]
        action: StudioAlignerAction,
    },
}

#[derive(Subcommand)]
pub(crate) enum StudioAlignerAction {
    /// Whether the aligner model is downloaded, where, and its size
    Status,
    /// Download the aligner model, printing progress; does nothing when it
    /// is already there
    Download,
    /// Delete the aligner model and any partial download
    Delete,
}

/// Which TTS server, model, voice and speed to use. Each flag overrides the
/// video.json `voice` block, which overrides Settings -> Voice.
#[derive(Args)]
pub(crate) struct StudioVoiceArgs {
    /// An OpenAI-compatible TTS server, with or without a trailing `/v1`
    /// (e.g. http://127.0.0.1:8880). Keyless servers only
    #[arg(long, value_name = "URL")]
    pub(crate) server: Option<String>,
    /// The server's model id (e.g. `kokoro`, or `mlx-community/Kokoro-82M-bf16`
    /// on mlx-audio)
    #[arg(long, value_name = "MODEL")]
    pub(crate) model: Option<String>,
    /// The voice (e.g. `af_heart`)
    #[arg(long, value_name = "VOICE")]
    pub(crate) voice: Option<String>,
    /// Speaking rate; 1.0 is normal, 1.25 is a quarter faster
    #[arg(long, value_name = "S")]
    pub(crate) speed: Option<f64>,
}

/// Synthesise and time a video's narration, writing narration/manifest.json
///
/// Each scene with a `## <id>` section in `script.md` gets a clip: its
/// section text is sent to the TTS server (`POST <server>/v1/audio/speech`,
/// WAV), the clip is saved as `narration/<key>.wav`, and its words are timed
/// against the script with the aligner model (`oculus studio aligner
/// download` fetches it once). The manifest then gives each narrated scene its
/// length and its `word()` cues; `frame`, `strip` and `lint` read it.
///
/// A clip's key hashes its text, the server, model, voice, speed and aligner
/// model, so a run only requests scenes whose text or voice changed; the
/// rest print `cached`. Scenes run one at a time in video order and the
/// manifest is saved after each, so a failure keeps the scenes before it;
/// the command then exits 1 naming the failed scene. At the end, entries
/// for scenes no longer narrated and clips nothing names are deleted.
///
/// Per scene it prints the id, `narrated` or `cached`, the clip's length,
/// its word count, and as warnings the words the aligner was unsure of and
/// any speech the script lacks (a mangled or added word: reword it, or
/// listen). `lint` fails on the same words.
///
///     oculus studio narrate lectures/week-4
///
///     oculus studio narrate lectures/week-4 --scene steps --force --voice bf_emma
#[derive(Args)]
pub(crate) struct StudioNarrateArgs {
    /// The video folder, the one holding `video.json` and `script.md`
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
    /// Only this scene id
    #[arg(long, value_name = "ID")]
    pub(crate) scene: Option<String>,
    /// Make the clips again even when the cached ones match
    #[arg(long)]
    pub(crate) force: bool,
    #[command(flatten)]
    pub(crate) voice: StudioVoiceArgs,
}

/// Synthesise one clip of text to a WAV, to hear a voice
///
/// Sends `TEXT` to the TTS server as `narrate` would and writes the WAV it
/// returns, then prints its path and length. An empty, non-WAV or silent
/// reply is an error (Kokoro answers an unknown voice with silence). Uses
/// Settings -> Voice unless the flags say otherwise; no video is involved.
///
///     oculus studio tts "We follow the gradient downhill." --voice bf_emma -o /tmp/emma.wav
#[derive(Args)]
pub(crate) struct StudioTtsArgs {
    /// What to say, written as it is said
    #[arg(value_name = "TEXT")]
    pub(crate) text: String,
    /// Where to write the WAV
    #[arg(short, long, value_name = "OUT.wav", default_value = "tts.wav")]
    pub(crate) out: PathBuf,
    #[command(flatten)]
    pub(crate) voice: StudioVoiceArgs,
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
    /// Average sub-frames over a 180° shutter at the video's fps, as many as
    /// the motion needs (off unless given, whatever the theme's motionBlur
    /// says)
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
    /// Blur each frame over a 180° shutter at the video's fps (see `frame
    /// --help`)
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
/// and the narration (missing or stale clips, words the aligner was unsure
/// of, unscripted speech). Every problem is reported, not just the first,
/// each with its scene, component and, where there is one, the TSX line.
/// Exits 1 when there is any problem, so it can gate a render.
///
/// See `oculus studio frame --help` for what a video folder holds.
#[derive(Args)]
pub(crate) struct StudioLintArgs {
    /// The video folder, the one holding `video.json`
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
}

/// Render every example of a component as a still
///
/// Each entry of the component's `examples` is drawn as one PNG, at one
/// moment: `--t` seconds from the start of the component's scene, by default
/// the middle of the first scene that uses it (or 1 s of a 2 s preview when
/// none does). Examples render in parallel and the PNGs are the same at any
/// thread count. The component is read through the cascade (the video's
/// `components/`, then the subject's library, then the global one), and the
/// theme the same way. `--sheet` also writes all the stills as one contact
/// sheet, `--cols` a row. Stills are named `<Component>-<example>.png`.
///
///     oculus studio gallery lectures/week-4 Steps
///
///     oculus studio gallery lectures/week-4 Steps --subject COMP2011 --sheet --cols 2
#[derive(Args)]
pub(crate) struct StudioGalleryArgs {
    /// The video folder, the one holding `video.json`
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
    /// The component's name (PascalCase), as its file is named
    #[arg(value_name = "COMPONENT")]
    pub(crate) component: String,
    /// The course whose library is read for this load, over video.json's
    #[arg(long, value_name = "CODE")]
    pub(crate) subject: Option<String>,
    /// The theme (a bundled or library name, or a `.json` path), over
    /// video.json's
    #[arg(long, value_name = "THEME")]
    pub(crate) theme: Option<String>,
    /// Seconds from the start of the component's scene (default: the middle
    /// of its scene, or 1 s)
    #[arg(long = "t", value_name = "SECONDS", allow_negative_numbers = true)]
    pub(crate) t: Option<f64>,
    /// The folder for the PNGs (default: `<DIR>/out/gallery/`)
    #[arg(long, value_name = "DIR")]
    pub(crate) out: Option<PathBuf>,
    /// Also write every still as one contact sheet, `<Component>.png`
    #[arg(long)]
    pub(crate) sheet: bool,
    /// Stills per row in the contact sheet
    #[arg(long, value_name = "N", default_value_t = 4)]
    pub(crate) cols: usize,
    /// Render threads (default: half the cores, at most one per example)
    #[arg(long, value_name = "T")]
    pub(crate) threads: Option<usize>,
}

/// List, show or seed a video's components
///
/// `list` and `show` read the components a video can use through the
/// cascade and say which level each comes from: `project` (its own
/// `components/`), `subject <CODE>` (the course's `studio/components/`) or
/// `global` (`<app data>/studio/library/components/`). `seed` copies the
/// starter set into the global library; it never replaces an edit, and
/// `--reset` writes the starter text over the library's, keeping each old
/// text in the library's `.history/` first.
#[derive(Subcommand)]
pub(crate) enum StudioComponentsAction {
    /// Each component name a video can use, and the level it comes from
    List(StudioComponentsListArgs),
    /// A component's level and its TSX source
    Show(StudioComponentsShowArgs),
    /// Copy the starter components and themes into the global library
    Seed {
        /// Write the starter text over every file that differs, keeping the
        /// old text in `.history/`
        #[arg(long)]
        reset: bool,
    },
}

#[derive(Args)]
pub(crate) struct StudioComponentsListArgs {
    /// The video folder, the one holding `video.json`
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
    /// The course whose library is read for this load, over video.json's
    #[arg(long, value_name = "CODE")]
    pub(crate) subject: Option<String>,
}

#[derive(Args)]
pub(crate) struct StudioComponentsShowArgs {
    /// The video folder, the one holding `video.json`
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
    /// The component's name (PascalCase)
    #[arg(value_name = "COMPONENT")]
    pub(crate) component: String,
    /// The course whose library is read for this load, over video.json's
    #[arg(long, value_name = "CODE")]
    pub(crate) subject: Option<String>,
}

/// Render a video to an mp4
///
/// Each scene is rendered frame by frame (in parallel, one renderer per
/// thread) and encoded by ffmpeg as its own segment, which is kept in
/// `<DIR>/.cache/segments/` under a hash of everything its frames depend
/// on: the scene and the one before it, their narration, the theme, the
/// `components/` and `assets/` folders and the render settings. A second
/// render only renders the scenes an edit touched and prints `cached` for
/// the rest. The segments are joined without re-encoding, beside the
/// narration clips mixed into one track (AAC). Segments neither quality's
/// last render used are deleted afterwards.
///
/// `--draft` (the default) is a 540 px-high proxy on the hardware encoder,
/// without motion blur unless the theme's `motionBlur` says `always`;
/// `--final` is full size on libx264 (crf 18), blurred unless `motionBlur`
/// is `off`. Both are H.264, yuv420p, BT.709. The mp4 goes to
/// `<DIR>/renders/draft.mp4` or `final.mp4` unless `-o` names another file.
///
/// Render never narrates: run `oculus studio narrate` first, or it stops
/// with the stale narration's scene. A frame that fails stops the render,
/// naming its time and scene; an ffmpeg failure quotes ffmpeg.
///
///     oculus studio render lectures/week-4
///
///     oculus studio render lectures/week-4 --final -o ~/Desktop/week-4.mp4
#[derive(Args)]
#[command(group(clap::ArgGroup::new("quality").args(["draft", "final_"]).multiple(false)))]
pub(crate) struct StudioRenderArgs {
    /// The video folder, the one holding `video.json`
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
    /// A 540 px-high proxy on the hardware encoder (the default)
    #[arg(long)]
    pub(crate) draft: bool,
    /// Full size on libx264, with the theme's motion blur
    #[arg(long = "final")]
    pub(crate) final_: bool,
    /// Where to write the mp4 (default: `<DIR>/renders/<quality>.mp4`)
    #[arg(short, long, value_name = "OUT.mp4")]
    pub(crate) out: Option<PathBuf>,
    /// Render threads (default: half the cores, capped to keep memory
    /// under 2 GB)
    #[arg(long, value_name = "T")]
    pub(crate) threads: Option<usize>,
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
    fn narrate_and_tts_take_the_voice_flags() {
        let ok = |args: &[&str]| Cli::try_parse_from(args).is_ok();
        assert!(ok(&[
            "oculus", "studio", "narrate", "v", "--scene", "a", "--force"
        ]));
        assert!(ok(&[
            "oculus", "studio", "narrate", "v", "--server", "http://x", "--model", "m", "--voice",
            "af_heart", "--speed", "1.2"
        ]));
        assert!(ok(&[
            "oculus", "studio", "tts", "Hello.", "-o", "x.wav", "--voice", "v"
        ]));
        assert!(ok(&["oculus", "studio", "aligner", "status"]));
        assert!(!ok(&["oculus", "studio", "aligner"]));
        assert!(!ok(&["oculus", "studio", "narrate"]));
    }

    #[test]
    fn render_takes_one_quality() {
        let ok = |args: &[&str]| {
            let argv = ["oculus", "studio", "render", "video"].iter().chain(args);
            Cli::try_parse_from(argv).is_ok()
        };
        assert!(ok(&[]));
        assert!(ok(&["--draft", "-o", "x.mp4", "--threads", "3"]));
        assert!(ok(&["--final"]));
        assert!(!ok(&["--draft", "--final"]));
    }

    #[test]
    fn gallery_and_components_take_their_flags() {
        let ok = |args: &[&str]| Cli::try_parse_from(args).is_ok();
        assert!(ok(&["oculus", "studio", "gallery", "v", "Steps"]));
        assert!(ok(&[
            "oculus",
            "studio",
            "gallery",
            "v",
            "Steps",
            "--subject",
            "COMP2011",
            "--theme",
            "warm",
            "--t",
            "1.5",
            "--out",
            "x",
            "--sheet",
            "--cols",
            "2",
            "--threads",
            "1"
        ]));
        assert!(!ok(&["oculus", "studio", "gallery", "v"]));
        assert!(ok(&[
            "oculus",
            "studio",
            "components",
            "list",
            "v",
            "--subject",
            "S"
        ]));
        assert!(ok(&[
            "oculus",
            "studio",
            "components",
            "show",
            "v",
            "Steps"
        ]));
        assert!(ok(&["oculus", "studio", "components", "seed", "--reset"]));
        assert!(!ok(&["oculus", "studio", "components"]));
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
