//! Arguments of the `studio` commands. `new` and `list` make and list the
//! videos under `<app data>/studio/videos/`; every command that takes a DIR
//! also takes a bare slug naming one of them. `frame`, `strip`, `lint`,
//! `brief`, `beats` and `render` read a video folder on disk (no database,
//! no network); `slide` and `blocks` read a library PDF; `narrate`
//! and `tts` run Kokoro or call the TTS server, `g2p` shows how Kokoro reads
//! text, `pronounce` puts a reading up for the student's review, and
//! `voices` and `aligner` manage the downloaded models.

use crate::*;

#[derive(Subcommand)]
pub(crate) enum StudioAction {
    New(StudioNewArgs),
    List(StudioListArgs),
    Frame(StudioFrameArgs),
    Strip(StudioStripArgs),
    Lint(StudioLintArgs),
    Brief(StudioBriefArgs),
    Beats(StudioBeatsArgs),
    Gallery(StudioGalleryArgs),
    Slide(StudioSlideArgs),
    Blocks(StudioBlocksArgs),
    Components {
        #[command(subcommand)]
        action: StudioComponentsAction,
    },
    /// Promote a video's own theme file into a library
    Themes {
        #[command(subcommand)]
        action: StudioThemesAction,
    },
    Render(StudioRenderArgs),
    Narrate(StudioNarrateArgs),
    Tts(StudioTtsArgs),
    G2p(StudioG2pArgs),
    /// Suggest how Kokoro should say a word, for the student to review
    ///
    /// The lexicon (`lexicon.json`) is written only by the app: an agent
    /// that doubts a guessed word suggests a reading with `suggest`, and the
    /// student hears the word's sentence both ways in Settings -> Voice and
    /// accepts the suggestion, keeps the guess, or marks it wrong with a
    /// note. An accepted reading or a kept guess goes into the subject's
    /// lexicon (the global library's for a video with no subject) and
    /// applies at the next `narrate`. `list` shows what is still waiting
    /// and what was marked wrong.
    Pronounce {
        #[command(subcommand)]
        action: StudioPronounceAction,
    },
    /// List Kokoro's voices, or download or delete its packs (103 MB)
    ///
    /// The Kokoro engine narrates inside the app from two packs, both kept
    /// in `<app data>/models/voice/`, downloaded on request, checked against
    /// a pinned size and sha256 and never bundled: `voice-kokoro` (the int8
    /// model and the voices `af_heart` and `am_michael`, 93 MB) and
    /// `g2p-en-us` (misaki's US dictionary and the guesser that reads the
    /// words it lacks, 9 MB). With no action it lists each pack's status
    /// and each voice, marking the one Settings -> Voice names.
    Voices {
        #[command(subcommand)]
        action: Option<StudioVoicesAction>,
    },
    Agents(StudioAgentsArgs),
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

#[derive(Subcommand)]
pub(crate) enum StudioPronounceAction {
    Suggest(StudioPronounceSuggestArgs),
    List(StudioPronounceListArgs),
}

/// Put a reading of a word up for the student's review
///
/// Checks `READING` as a lexicon value: a respelling in dictionary words
/// joined by `-` or spaces, an all-caps part taking the main stress
/// (`NESS-ter-off`), or raw phonemes in Kokoro's alphabet in slashes
/// (`/nˈɛstəɹˌɔf/`). A part the dictionary lacks or a symbol Kokoro lacks
/// fails, naming it. Then reads the video's narration with Kokoro's G2P to
/// find `WORD` (any case; `Word's` counts): its first guessed place, else
/// its first place, recording how Kokoro reads it now, its sentence, scene
/// and script.md line. A word the narration never says fails. The
/// suggestion is stored as pending under the video's subject, replacing an
/// earlier one for the same word; it prints the reading's phonemes. The
/// student reviews it in Settings -> Voice. Needs the `g2p-en-us` pack.
///
///     oculus studio pronounce suggest momentum Nesterov NESS-ter-off
#[derive(Args)]
pub(crate) struct StudioPronounceSuggestArgs {
    /// The video folder (or its slug) whose narration says the word
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
    /// The word, as the script spells it
    #[arg(value_name = "WORD")]
    pub(crate) word: String,
    /// How to say it: `RE-spell-ing` or `/phonemes/`
    #[arg(value_name = "READING")]
    pub(crate) reading: String,
}

/// List the suggestions waiting for the student, and the ones marked wrong
///
/// Newest first: the status (`pending`, or `wrong` with the student's note
/// on how the word is said), the word, the suggested reading, the subject,
/// video, scene and line. A wrong one wants another suggestion that follows
/// the note. Decided suggestions are gone: their reading is in the lexicon.
///
///     oculus studio pronounce list --video momentum
#[derive(Args)]
pub(crate) struct StudioPronounceListArgs {
    /// Only this video's (a folder or slug)
    #[arg(long, value_name = "DIR")]
    pub(crate) video: Option<PathBuf>,
}

#[derive(Subcommand)]
pub(crate) enum StudioVoicesAction {
    /// Each pack's status and size, and each voice and whether it is
    /// downloaded (the default)
    List,
    /// Download both packs, printing progress; files already there are
    /// kept
    Download,
    /// Delete both packs and any partial download
    Delete,
}

impl StudioAction {
    /// The DIR this command reads, if it takes one, for slug resolution.
    pub(crate) fn dir_mut(&mut self) -> Option<&mut PathBuf> {
        match self {
            StudioAction::Frame(a) => Some(&mut a.dir),
            StudioAction::Strip(a) => Some(&mut a.dir),
            StudioAction::Lint(a) => Some(&mut a.dir),
            StudioAction::Brief(a) => Some(&mut a.dir),
            StudioAction::Beats(a) => Some(&mut a.dir),
            StudioAction::Gallery(a) => Some(&mut a.dir),
            StudioAction::Slide(a) => Some(&mut a.dir),
            StudioAction::Render(a) => Some(&mut a.dir),
            StudioAction::Narrate(a) => Some(&mut a.dir),
            StudioAction::G2p(a) => a.video.as_mut(),
            StudioAction::Pronounce { action } => match action {
                StudioPronounceAction::Suggest(a) => Some(&mut a.dir),
                StudioPronounceAction::List(a) => a.video.as_mut(),
            },
            StudioAction::Components { action } => match action {
                StudioComponentsAction::List(a) => Some(&mut a.dir),
                StudioComponentsAction::Show(a) => Some(&mut a.dir),
                StudioComponentsAction::Promote(a) => Some(&mut a.dir),
                StudioComponentsAction::Seed { .. } => None,
            },
            StudioAction::Themes { action } => match action {
                StudioThemesAction::Promote(a) => Some(&mut a.dir),
            },
            StudioAction::New(_)
            | StudioAction::List(_)
            | StudioAction::Tts(_)
            | StudioAction::Blocks(_)
            | StudioAction::Voices { .. }
            | StudioAction::Agents(_)
            | StudioAction::Aligner { .. } => None,
        }
    }
}

/// Make a new video under `<app data>/studio/videos/<SLUG>/`
///
/// Writes `video.json` (1920x1080, or 1080x1920 with `--portrait`, 30 fps,
/// theme `default`, the subject, and one silent `TitleCard` scene holding
/// `--title`), an empty `script.md`, and empty `components/` and `assets/`,
/// then prints the folder. A subject's library can replace the `default`
/// theme with its own `themes/default.json`. Every other `oculus studio`
/// command takes the slug in place of the folder. Refuses a slug that
/// exists.
///
///     oculus studio new gradient-descent --subject COMP30027 --title "Gradient descent"
#[derive(Args)]
pub(crate) struct StudioNewArgs {
    /// The video's name: lowercase letters, digits and `-`
    #[arg(value_name = "SLUG")]
    pub(crate) slug: String,
    /// The course the video belongs to; its library is read before the
    /// global one
    #[arg(long, value_name = "CODE")]
    pub(crate) subject: Option<String>,
    /// The title card's text (default: the slug, spaced)
    #[arg(long, value_name = "TITLE")]
    pub(crate) title: Option<String>,
    /// 1080x1920 instead of 1920x1080
    #[arg(long)]
    pub(crate) portrait: bool,
}

/// Which models a chat agent's Studio subagents run on, and how to start one
///
/// For each agent, or only `--agent`'s: the `studio-research` and
/// `studio-scene` roles, the model and effort Settings -> Jobs gives that
/// agent (its first entry in "Studio research" and "Studio scenes"), and the
/// exact call that agent makes to start one. A role with no entry for the
/// agent is off: the agent does that work itself. agy's subagents take no
/// model, so agy always does it itself. Reads only the settings.
///
///     oculus studio agents --agent codex
#[derive(Args)]
pub(crate) struct StudioAgentsArgs {
    /// Only this agent: claude, codex, opencode or antigravity
    #[arg(long, value_name = "AGENT")]
    pub(crate) agent: Option<String>,
}

/// List the videos under `<app data>/studio/videos/`
///
/// One line per video: its slug, subject, scene count, whether it has
/// narration (`narrated`; `lint` says whether it is current), the renders
/// in its `renders/` and when it last changed. A folder whose video.json
/// does not load is listed with the reason.
#[derive(Args)]
pub(crate) struct StudioListArgs {
    /// Only this course's videos
    #[arg(long, value_name = "CODE")]
    pub(crate) subject: Option<String>,
}

/// Which engine, voice and speed to use, and the server and model for the
/// server engine. Each flag overrides the video.json `voice` block, which
/// overrides Settings -> Voice.
#[derive(Args)]
pub(crate) struct StudioVoiceArgs {
    /// `kokoro` runs Kokoro inside the app (`oculus studio voices download`
    /// fetches it); `server` asks an OpenAI-compatible TTS server
    #[arg(long, value_enum, value_name = "ENGINE")]
    pub(crate) engine: Option<StudioEngineArg>,
    /// An OpenAI-compatible TTS server, with or without a trailing `/v1`
    /// (e.g. http://127.0.0.1:8880). Keyless servers only
    #[arg(long, value_name = "URL")]
    pub(crate) server: Option<String>,
    /// The server's model id (e.g. `kokoro`, or `mlx-community/Kokoro-82M-bf16`
    /// on mlx-audio)
    #[arg(long, value_name = "MODEL")]
    pub(crate) model: Option<String>,
    /// The voice (e.g. `af_heart`; Kokoro's are listed by `oculus studio
    /// voices`)
    #[arg(long, value_name = "VOICE")]
    pub(crate) voice: Option<String>,
    /// Speaking rate; 1.0 is the voice's own pace, 1.25 is a quarter
    /// faster (unset anywhere: 0.9)
    #[arg(long, value_name = "S")]
    pub(crate) speed: Option<f64>,
}

/// A narration engine
#[derive(Clone, Copy, clap::ValueEnum)]
pub(crate) enum StudioEngineArg {
    /// Kokoro, run inside the app
    Kokoro,
    /// An OpenAI-compatible TTS server
    Server,
}

/// Synthesise and time a video's narration, writing narration/manifest.json
///
/// Each scene with a `## <id>` section in `script.md` gets a clip: its
/// section text is spoken by Kokoro inside the app (`--engine kokoro`; read
/// to phonemes through the lexicon, the dictionary and the guesser) or sent
/// to a TTS server (`--engine server`, `POST <server>/v1/audio/speech`,
/// WAV), the clip is saved as `narration/<key>.wav`, and its words are timed
/// against the script with the aligner model (`oculus studio aligner
/// download` fetches it once). The manifest then gives each narrated scene its
/// length and its `word()` cues; `frame`, `strip` and `lint` read it. A
/// section is said and timed whole, then each `[pause]` is spliced in as
/// exact silence between the words either side of it.
///
/// A clip's key hashes its text with its pauses, the engine, the voice and
/// speed, the server and model or Kokoro's model and the text's phonemes,
/// and the aligner model, so a run only makes scenes whose text, voice or reading
/// changed (a lexicon edit re-narrates the scenes it changes); the rest
/// print `cached`. Scenes run one at a time in video order and the
/// manifest is saved after each, so a failure keeps the scenes before it;
/// the command then exits 1 naming the failed scene. At the end, entries
/// for scenes no longer narrated and clips nothing names are deleted.
///
/// Per scene it prints the id, `narrated` or `cached`, the clip's length,
/// its word count and pauses, and as warnings the words the aligner was unsure of and
/// any speech the script lacks (a mangled or added word: reword it, or
/// listen). `lint` notes the unsure words and fails on the added speech.
/// With Kokoro, every word the guesser read is then a `note: check
/// pronunciation` with its scene and script.md line, and the state of any
/// suggestion for it: listen to it, and suggest a reading for a doubtful
/// one with `oculus studio pronounce suggest`. Notes never change the exit
/// code.
///
///     oculus studio narrate lectures/week-4
///
///     oculus studio narrate lectures/week-4 --scene steps --force --voice bf_emma
#[derive(Args)]
pub(crate) struct StudioNarrateArgs {
    /// The video folder, the one holding `video.json` (or its slug) and `script.md`
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
/// Speaks `TEXT` as `narrate` would, with Kokoro or the TTS server, and
/// writes the WAV, then prints its path and length. An empty, non-WAV or
/// silent clip is an error (a Kokoro server answers an unknown voice with
/// silence). Uses Settings -> Voice unless the flags say otherwise; no
/// video is involved, so Kokoro reads only the global library's lexicon.
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

/// Show how Kokoro reads text: each word's phonemes and who read it
///
/// Reads `TEXT` as `narrate --engine kokoro` would, without synthesising:
/// each word with its phonemes and its source, `lexicon` (an entry in a
/// lexicon.json), `dict` (misaki's US dictionary and its rules), `rule` (a
/// heteronym's reading picked by its neighbours) or `guess` (the guesser,
/// worth a listen), then the phoneme string Kokoro is given. The lexicon is
/// the global library's (`<app data>/studio/library/lexicon.json`) with the
/// subject's over it (`<app data>/studio/subjects/<CODE>/lexicon.json`):
/// the subject is `--subject`, else the video's (`--video`, or the current
/// folder when it holds a `video.json`). Needs the `g2p-en-us` pack
/// (`oculus studio voices download`). A lexicon entry that is not valid
/// fails, naming its file and key.
///
///     oculus studio g2p "Nesterov and RMSProp adapt the step size."
///
///     oculus studio g2p "Cauchy" --subject MAST20004
#[derive(Args)]
pub(crate) struct StudioG2pArgs {
    /// The text, written as it is said
    #[arg(value_name = "TEXT")]
    pub(crate) text: String,
    /// A video folder (or slug) whose subject's lexicon to read
    #[arg(long, value_name = "DIR")]
    pub(crate) video: Option<PathBuf>,
    /// The subject whose lexicon to read, over the video's
    #[arg(long, value_name = "CODE")]
    pub(crate) subject: Option<String>,
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
    /// The video folder, the one holding `video.json` (or its slug)
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
    /// The video folder, the one holding `video.json` (or its slug)
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

/// Check a whole video, or one scene, without writing to its folder
///
/// Loads every component the video uses, checks each scene's props and each
/// component's `examples` against its `props` schema, and renders every scene
/// at its first and last moment. For a narrated video it also checks
/// `script.md` (sections no scene matches, text not written as it is said)
/// and the narration (missing or stale clips, unscripted speech). When the
/// video narrates with Kokoro it also reads every section as `narrate`
/// will: a missing `g2p-en-us` pack, a lexicon entry that is not valid, a
/// word with no phonemes or a sentence too long for Kokoro is an error.
///
/// Each scene that renders is then laid out every 0.2 s and at each cue
/// word (never painted) for warnings: a node past the frame's edge
/// (`off_frame`), narration over a frame where nothing changes for more
/// than 2.5 s or an empty opening (`still`), everything kept small in one
/// half of the frame for more than 3 s (`half_frame`), and a Math down to
/// a piece or two, or a tiny mark alone, at the end (`fragment`). Each
/// names its time span and the words said in it.
///
/// Three levels: `error` (exits 1, so it can gate a render), `warning`
/// and `note` (a word the aligner was unsure of, a word Kokoro's guesser
/// read); warnings and notes never change the exit code. Errors print one
/// a line in full; repeats of a warning or note in one scene print as one
/// line with a count and the first few. `--json` gives one entry per
/// issue with its `level`.
///
/// See `oculus studio frame --help` for what a video folder holds.
///
///     oculus studio lint shor --scene mod
#[derive(Args)]
pub(crate) struct StudioLintArgs {
    /// The video folder, the one holding `video.json` (or its slug)
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
    /// Only this scene (its id in video.json, or its index counting from
    /// 0), with its component and the video settings it renders with
    #[arg(long, value_name = "ID|N")]
    pub(crate) scene: Option<String>,
}

/// Print what changes on screen in one scene, as a timeline
///
/// Lays the scene out every 0.2 s and at each cue word (never painted)
/// and lists, per key (or per Math, or per TSX line for nodes with no key
/// near them), when it appears, vanishes, moves (centre more than 8 px),
/// resizes, recolours or is rewritten, each with the words said then, and
/// each `[pause]` with its length.
/// Numbered keys doing the same thing at the same moment share a line
/// (`d12-d14`). A builder checks a beat table against it as text instead
/// of reading a strip. At most 80 lines print; `--json` gives every beat
/// as `{index, id, component, length, beats: [{t, end, event, who,
/// detail, words}]}`.
///
///     oculus studio beats shor --scene squares
#[derive(Args)]
pub(crate) struct StudioBeatsArgs {
    /// The video folder, the one holding `video.json` (or its slug)
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
    /// The scene: its id in video.json, or its index counting from 0
    #[arg(long, value_name = "ID|N")]
    pub(crate) scene: String,
}

/// Print one scene builder's whole input
///
/// For one scene: its index, component and props; the scenes before and
/// after it, whether their components exist yet, and what their
/// storyboard entries say under `Carries in:` and `Hands on:`; its own
/// `storyboard.md` entry verbatim (from its `### <id>` heading to the next
/// heading or `---`); the `notes/` files the entry names, else every file
/// in `notes/`; its narration, one line per sentence, each word with the
/// time `word()` returns for it (scene seconds, the theme's lead included)
/// and each pause after its word at the time `pause()` returns; and the
/// exact `strip`, `frame` and `gallery` calls that check it, `-o` paths
/// absolute. Stale or missing narration prints its error and the
/// script text instead. Reads only the folder.
///
///     oculus studio brief shor --scene squares
///
///     oculus studio brief lectures/week-4 --scene 3 --json
#[derive(Args)]
pub(crate) struct StudioBriefArgs {
    /// The video folder, the one holding `video.json` (or its slug)
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
    /// The scene: its id in video.json, or its index counting from 0
    #[arg(long, value_name = "ID|N")]
    pub(crate) scene: String,
}

/// Render every example of a component as a still
///
/// Each entry of the component's `examples` is drawn as one PNG, at one
/// moment: `--t` seconds from the start of the component's scene, by default
/// its middle. That is the middle of the first scene that uses it when the
/// scene is silent with a `hold`, else of a 6 s preview (a narrated scene's
/// length needs its narration, which a gallery never reads, so it runs
/// before `narrate`; `word()` throws in a gallery). Examples render in
/// parallel and the PNGs are the same at any thread count. The component is read through the cascade (the video's
/// `components/`, then the subject's library, then the global one), and the
/// theme the same way. `--sheet` also writes all the stills as one contact
/// sheet, `--cols` a row. Stills are named `<Component>-<example>.png`.
///
///     oculus studio gallery lectures/week-4 Bars
///
///     oculus studio gallery lectures/week-4 Bars --subject COMP2011 --sheet --cols 2
#[derive(Args)]
pub(crate) struct StudioGalleryArgs {
    /// The video folder, the one holding `video.json` (or its slug)
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
    /// of its silent scene's hold, or 3 s of the 6 s preview)
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

/// Crop a region of a library PDF page into a video's assets
///
/// Renders the region with the app's PDF renderer and writes
/// `<DIR>/assets/slides/<NAME>.png` (sharp at up to 2x the video's frame),
/// `<NAME>.page.png` (the whole page fitted to the frame, for a zoom in from
/// the page) and `<NAME>.json`: the PDF's library path and sha256, the page,
/// the crop as fractions of the page, how it was chosen, the blocks inside it
/// as fractions of the crop (kind and text, for highlight boxes), and the
/// props for the `Slide` starter, which it also prints. FILE resolves as for
/// `oculus read`. Name the region one way: `--find` text in a block (a
/// figure, chart or table that holds it wins over running text; a caption
/// that is its own block brings the figure beside it; a tie or no match
/// lists the candidates), `--block` indices from `oculus studio blocks`, or
/// `--bbox`. Blocks come from the parse record, so `--find` and `--block`
/// need a parsed PDF. The same inputs write the same bytes; lint warns when
/// the PDF changes after.
///
///     oculus studio slide momentum lecture-04.pdf --find "Figure 3" --name descent
///
///     oculus studio slide momentum lecture-04.pdf --page 12 --block 3,4
///
///     oculus studio slide momentum lecture-04.pdf --page 12 --bbox 0.5,0.2,0.95,0.8
#[derive(Args)]
pub(crate) struct StudioSlideArgs {
    /// The video folder, the one holding `video.json` (or its slug)
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
    /// Library path, filename, or a fragment of either
    #[arg(value_name = "FILE")]
    pub(crate) file: String,
    /// The page, from 1 (needed by --block and --bbox; --find without it
    /// searches every page)
    #[arg(short = 'p', long, value_name = "N")]
    pub(crate) page: Option<u32>,
    /// Text in the block to crop, any case, e.g. "Figure 3"
    #[arg(long, value_name = "TEXT")]
    pub(crate) find: Option<String>,
    /// Block indices on the page; several are cropped as one region round
    /// them all
    #[arg(long, value_name = "I[,J…]", value_delimiter = ',')]
    pub(crate) block: Vec<usize>,
    /// A box as fractions of the page from its top left
    #[arg(long, value_name = "X0,Y0,X1,Y1")]
    pub(crate) bbox: Option<String>,
    /// The files' name under assets/slides/ (default: from the page and
    /// what was asked, e.g. `p12-figure-3`)
    #[arg(long, value_name = "NAME")]
    pub(crate) name: Option<String>,
    /// Margin round the region, as a fraction of the page's shorter side
    #[arg(long, value_name = "FRACTION", default_value_t = 0.02)]
    pub(crate) pad: f64,
    /// The crop's width in pixels (default: 2x what fills the frame, at
    /// most 6 megapixels)
    #[arg(long, value_name = "PX")]
    pub(crate) width: Option<u32>,
    /// Overwrite a slide of the same name cropped from another PDF
    #[arg(long)]
    pub(crate) replace: bool,
    /// Disambiguate FILE by subject; prefix codes are fine
    #[arg(short = 's', long, value_name = "SUBJECT_CODE")]
    pub(crate) subject: Option<String>,
}

/// List a parsed PDF's blocks: what `slide` can crop
///
/// For each page (or only `--page`): its size in points, then each block of
/// its parse record: index, kind (`text`, `image`, `chart`, `table`,
/// `equation`, …), box as fractions of the page from its top left, and its
/// text on one line (an image's caption). `--png` also writes that page with
/// every block's box and index drawn on it (figures in indigo, the rest in
/// grey), to look at before picking.
///
///     oculus studio blocks lecture-04.pdf --page 12
///
///     oculus studio blocks lecture-04.pdf --page 12 --png out/p12.png
#[derive(Args)]
pub(crate) struct StudioBlocksArgs {
    /// Library path, filename, or a fragment of either
    #[arg(value_name = "FILE")]
    pub(crate) file: String,
    /// Only this page, from 1
    #[arg(short = 'p', long, value_name = "N")]
    pub(crate) page: Option<u32>,
    /// With --page: write the page, its blocks drawn on, as this PNG
    #[arg(long, value_name = "OUT", requires = "page")]
    pub(crate) png: Option<PathBuf>,
    /// Disambiguate FILE by subject; prefix codes are fine
    #[arg(short = 's', long, value_name = "SUBJECT_CODE")]
    pub(crate) subject: Option<String>,
}

/// List, show, promote or seed a video's components
///
/// `list` and `show` read the components a video can use through the
/// cascade and say which level each comes from: `project` (its own
/// `components/`), `subject <CODE>` (`<app data>/studio/subjects/<CODE>/
/// components/`) or `global` (`<app data>/studio/library/components/`).
/// `promote` copies one of the video's own components into a library.
/// `seed` copies the starter set into the global library; it never replaces
/// an edit, and `--reset` writes the starter text over the library's. Every
/// library write keeps the old text in the library's `.history/` first.
#[derive(Subcommand)]
pub(crate) enum StudioComponentsAction {
    /// Each component name a video can use, and the level it comes from
    List(StudioComponentsListArgs),
    /// A component's level and its TSX source
    Show(StudioComponentsShowArgs),
    Promote(StudioComponentsPromoteArgs),
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
    /// The video folder, the one holding `video.json` (or its slug)
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
    /// The course whose library is read for this load, over video.json's
    #[arg(long, value_name = "CODE")]
    pub(crate) subject: Option<String>,
}

#[derive(Args)]
pub(crate) struct StudioComponentsShowArgs {
    /// The video folder, the one holding `video.json` (or its slug)
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
    /// The component's name (PascalCase)
    #[arg(value_name = "COMPONENT")]
    pub(crate) component: String,
    /// The course whose library is read for this load, over video.json's
    #[arg(long, value_name = "CODE")]
    pub(crate) subject: Option<String>,
}

/// Which library a promote writes to
#[derive(Clone, Copy, clap::ValueEnum)]
pub(crate) enum StudioLibraryArg {
    /// `<app data>/studio/library/`, read by every video
    Global,
    /// `<app data>/studio/subjects/<CODE>/`, for the subject in the video's
    /// video.json
    Subject,
}

/// Copy a video's own component into a library
///
/// Copies `<DIR>/components/<COMPONENT>.tsx` into the global library or the
/// video's subject's, so other videos can use it. A library file it
/// replaces is kept in that library's `.history/` first; when the library
/// already holds the same text nothing is written. The video keeps its own
/// copy, which still wins for it. Check the component with `gallery` (and
/// its scenes with `strip`) before promoting it.
///
///     oculus studio components promote gradient-descent Descent --to subject
#[derive(Args)]
pub(crate) struct StudioComponentsPromoteArgs {
    /// The video folder (or its slug)
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
    /// The component's name (PascalCase), as its file is named
    #[arg(value_name = "COMPONENT")]
    pub(crate) component: String,
    /// The library to copy it into
    #[arg(long, value_enum, value_name = "LIBRARY")]
    pub(crate) to: StudioLibraryArg,
}

#[derive(Subcommand)]
pub(crate) enum StudioThemesAction {
    Promote(StudioThemesPromoteArgs),
}

/// Copy a video's own theme file into a library
///
/// Checks that `FILE` (a `.json` path inside the video folder, such as
/// `themes/chalk.json`) loads as a theme, then copies it into the global
/// library or the video's subject's as `themes/<stem>.json`, so any video
/// can name it by its stem (`"theme": "chalk"`). A library file it replaces
/// is kept in that library's `.history/` first; when the library already
/// holds the same text nothing is written.
///
///     oculus studio themes promote gradient-descent themes/chalk.json --to global
#[derive(Args)]
pub(crate) struct StudioThemesPromoteArgs {
    /// The video folder (or its slug)
    #[arg(value_name = "DIR")]
    pub(crate) dir: PathBuf,
    /// The theme file, relative to the video folder
    #[arg(value_name = "FILE")]
    pub(crate) file: PathBuf,
    /// The library to copy it into
    #[arg(long, value_enum, value_name = "LIBRARY")]
    pub(crate) to: StudioLibraryArg,
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
    /// The video folder, the one holding `video.json` (or its slug)
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
    /// Frame-rendering threads (default: half the cores, capped to keep
    /// their memory under 2 GB). The encoder runs its own: `--final`'s
    /// libx264 always uses 8, so its output is the same at any `--threads`
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
        assert!(ok(&[
            "oculus",
            "studio",
            "pronounce",
            "suggest",
            "v",
            "Nesterov",
            "NESS-ter-off"
        ]));
        assert!(ok(&[
            "oculus",
            "studio",
            "pronounce",
            "suggest",
            "v",
            "Cauchy",
            "/kˈOʃi/"
        ]));
        assert!(!ok(&[
            "oculus",
            "studio",
            "pronounce",
            "suggest",
            "v",
            "Nesterov"
        ]));
        assert!(ok(&["oculus", "studio", "pronounce", "list"]));
        assert!(ok(&[
            "oculus",
            "studio",
            "pronounce",
            "list",
            "--video",
            "v"
        ]));
        assert!(!ok(&["oculus", "studio", "pronounce"]));
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
    fn new_list_and_promote_take_their_flags() {
        let ok = |args: &[&str]| Cli::try_parse_from(args).is_ok();
        assert!(ok(&["oculus", "studio", "new", "week-4"]));
        assert!(ok(&[
            "oculus",
            "studio",
            "new",
            "week-4",
            "--subject",
            "COMP2011",
            "--title",
            "Week 4",
            "--portrait"
        ]));
        assert!(!ok(&["oculus", "studio", "new"]));
        assert!(ok(&["oculus", "studio", "list"]));
        assert!(ok(&["oculus", "studio", "list", "--subject", "COMP2011"]));
        for to in ["global", "subject"] {
            assert!(ok(&[
                "oculus",
                "studio",
                "components",
                "promote",
                "week-4",
                "Steps",
                "--to",
                to
            ]));
            assert!(ok(&[
                "oculus",
                "studio",
                "themes",
                "promote",
                "week-4",
                "themes/chalk.json",
                "--to",
                to
            ]));
        }
        assert!(!ok(&[
            "oculus",
            "studio",
            "components",
            "promote",
            "week-4",
            "Steps"
        ]));
        assert!(!ok(&[
            "oculus",
            "studio",
            "components",
            "promote",
            "week-4",
            "Steps",
            "--to",
            "project"
        ]));
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
