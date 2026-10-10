//! Arguments of the `lecture` commands, which decode a recording already on
//! disk: no database, no network.

use crate::*;

#[derive(Subcommand)]
pub(crate) enum LectureAction {
    Candidates(LectureCandidatesArgs),
    Chapters(LectureChaptersArgs),
    End(LectureEndArgs),
}

/// Find where a recording plausibly changes topic
///
/// Samples the video one frame a second and reports the moments the picture
/// changes hard enough to be a new slide, thinned so no two are within 90
/// seconds. A transcript silence near a change nudges its score up; it never
/// creates a boundary on its own.
///
/// Detection is a single ffmpeg decode — a few seconds for an hour of video
/// — so nothing is stored and re-running always reflects the file on disk.
/// This is the raw candidate set: no titles and no summaries, which are a
/// later stage's job.
#[derive(Args)]
pub(crate) struct LectureCandidatesArgs {
    /// Lecture id, as `oculus list -l` prints it; a unique prefix is enough
    #[arg(value_name = "LECTURE_ID")]
    pub(crate) id: String,
    /// Also write one JPEG per candidate into the lecture's `frames/` folder,
    /// so the boundaries can be checked by eye
    #[arg(long)]
    pub(crate) frames: bool,
    /// Which captured stream to read — 1 or 2. Default: source 1, unless it
    /// turns out to be dead, in which case source 2 if it is downloaded
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u8).range(1..=2))]
    pub(crate) source: Option<u8>,
}

/// Name a recording's chapters with a CLI agent, and store them
///
/// Detects the boundary candidates, grabs a frame for each, then hands the
/// list, the transcript and the frames folder to a coding agent and asks it
/// which of them are real topic changes. The agent replies with JSON; this
/// command validates it against the candidate set and writes the rows. The
/// agent never touches the database.
///
/// One bad chapter rejects the whole set: a chapter list is a shape, and a
/// missing chapter is not a gap but twenty minutes silently attributed to the
/// chapter before it.
#[derive(Args)]
pub(crate) struct LectureChaptersArgs {
    /// Lecture id, as `oculus list -l` prints it; a unique prefix is enough
    #[arg(value_name = "LECTURE_ID")]
    pub(crate) id: String,
    // No defaults here: the job's selection comes from `harness::jobs`, shared
    // with the app; a flag overrides the part it names for one run.
    /// Which CLI to drive (default: the configured one)
    #[arg(short, long, value_parser = ["claude", "codex", "opencode"])]
    pub(crate) provider: Option<String>,
    /// Model to request (default: the configured one)
    #[arg(short, long)]
    pub(crate) model: Option<String>,
    /// Reasoning effort — low, medium, high, xhigh, max (default: the
    /// configured one)
    #[arg(long)]
    pub(crate) effort: Option<String>,
    /// Re-run over a lecture that already has chapters, replacing them
    #[arg(long)]
    pub(crate) force: bool,
    /// Which captured stream to read — 1 or 2. Default: source 1, unless it
    /// turns out to be dead, in which case source 2 if it is downloaded
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u8).range(1..=2))]
    pub(crate) source: Option<u8>,
}

/// Find where a recording's lecture ends, before its Q&A and dead air
///
/// Recordings run on after the lecturer signs off — students at the lectern,
/// packing up, a black projector — and the microphone keeps transcribing.
/// This hands the last 15 minutes of the transcript to a model in one short
/// turn with no tools, and asks which line the lecturer finishes on. The
/// model cites that line's second and quotes it; both are checked against the
/// transcript, and a reply that fails is asked again once with the reason. A
/// recording cut off mid-lecture has no end, and is stored as such.
///
/// If the recording is downloaded, its last 15 minutes are decoded too: a
/// projector that goes black for good is mentioned to the model as a hint,
/// never applied on its own. Only the transcript is required.
///
/// The end stored is the end of the line the quote finishes in. A lecture
/// already watched to within 10 seconds of it is marked done.
#[derive(Args)]
#[command(group(clap::ArgGroup::new("which").required(true).args(["ids", "all"])))]
pub(crate) struct LectureEndArgs {
    /// Lecture ids, as `oculus list -l` prints them; a unique prefix is enough
    #[arg(value_name = "LECTURE_ID")]
    pub(crate) ids: Vec<String>,
    /// Every lecture with a transcript whose end has not been looked for
    /// (with --force, every lecture with a transcript)
    #[arg(long)]
    pub(crate) all: bool,
    // As for chapters: flags override the job's configured selection.
    /// Which CLI to drive (default: the configured one)
    #[arg(short, long, value_parser = ["claude", "codex", "opencode"])]
    pub(crate) provider: Option<String>,
    /// Model to request (default: the configured one)
    #[arg(short, long)]
    pub(crate) model: Option<String>,
    /// Reasoning effort — low, medium, high, xhigh, max (default: the
    /// configured one)
    #[arg(long)]
    pub(crate) effort: Option<String>,
    /// Re-run over a lecture whose end is already found, replacing it
    #[arg(long)]
    pub(crate) force: bool,
    /// Ask and print, but write nothing — not the end, not the status, not done
    #[arg(long)]
    pub(crate) dry_run: bool,
    /// Print the brief and the prompt each lecture would send, and stop there
    #[arg(long, hide = true)]
    pub(crate) print_prompt: bool,
}
