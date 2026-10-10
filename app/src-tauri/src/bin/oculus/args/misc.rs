//! Arguments of the commands that stand alone: `agent`, `index`, `auth`,
//! `keyd`, `list`, `run`, `transcribe` and `docs`.

use crate::*;

#[derive(Args)]
#[command(
    about = "Run one prompt through a CLI agent (Claude Code, Codex, opencode or Antigravity)",
    long_about = "Run one prompt through a CLI agent and print what it does.\n\n\
The same bridges the app's chat uses, without the window: the agent runs from \
the library's agents/ folder with the app's instructions appended, can read the \
whole library and write only there, and its normalized events are printed as they \
arrive. Needs the provider's CLI installed and signed in (`claude`, `codex`, `opencode` or \
`agy`). \
Nothing is recorded in the database; this is for checking a bridge works."
)]
pub(crate) struct AgentArgs {
    /// What to ask
    #[arg(value_name = "PROMPT")]
    pub(crate) prompt: String,
    /// Which CLI to drive
    #[arg(short, long, value_parser = ["claude", "codex", "opencode", "antigravity"], default_value = "claude")]
    pub(crate) provider: String,
    /// Model to request (provider-specific name or alias)
    #[arg(short, long)]
    pub(crate) model: Option<String>,
    /// Codex reasoning effort (low, medium, high, xhigh)
    #[arg(long)]
    pub(crate) effort: Option<String>,
    /// Scope the turn to one subject, as the app's chat does
    #[arg(short = 's', long, value_name = "SUBJECT_CODE")]
    pub(crate) subject: Option<String>,
}

#[derive(Args)]
pub(crate) struct IndexArgs {
    /// Subject codes to index. Omit for every subject.
    #[arg(value_name = "SUBJECT_CODE")]
    pub(crate) codes: Vec<String>,
    /// Parse again every PDF whose record predates the current parser version.
    /// Spends MinerU allowance; embeddings are kept
    #[arg(long)]
    pub(crate) reparse: bool,
}

#[derive(Subcommand)]
pub(crate) enum AuthAction {
    /// Open the app's Canvas sign-in window and wait for the session
    Login,
    /// Forget the saved session
    Logout,
    /// Whether the saved session still works
    Status,
    /// Store the credentials that let Oculus sign in without a browser
    ///
    /// Needs a TOTP factor (Google Authenticator) enrolled and its setup key.
    /// A code cannot be derived from other codes, so the key must come from
    /// the enrolment screen — re-enrol the factor if you never copied it.
    Setup,
    /// Sign in headlessly with the stored credentials, now
    ///
    /// A manual sign-in: it skips the 10 min to 6 h wait between automatic
    /// attempts, but no two attempts start within a minute, and three failed
    /// manual attempts in a row wait like automatic ones. It cannot lift the
    /// pause a lockout or a rejected password leaves: fix the credentials in
    /// the app (Settings → Canvas) or run `oculus auth setup`.
    Auto,
    /// Forget the stored sign-in credentials
    Forget,
    /// Report what the Okta sign-in page looks like, when `auto` fails
    Diagnose,
    /// Show the Ed Discussion session status, or set a token manually
    ///
    /// Normally unnecessary — syncs mint the Ed session from the Canvas
    /// session via the course's LTI launch. The manual token (DevTools →
    /// Network → any edstem /api request → `x-token` header) is an override.
    Ed {
        /// An x-token JWT to save. Omit to check the current session.
        token: Option<String>,
    },
}

#[derive(Subcommand)]
pub(crate) enum KeydAction {
    /// Install oculus-keyd and load its LaunchAgent
    ///
    /// A keyd inside an app bundle is registered where it is; any other is
    /// copied to `bin/` in the data dir first, so the LaunchAgent never points
    /// into a build tree. Loading it prompts for nothing: keyd reads the
    /// keychain only when something first uses a stored key.
    Install {
        /// The signed keyd to install. Defaults to the one beside this binary
        /// in the app bundle, or, in a debug build, `bun run keyd`'s output.
        #[arg(long, value_name = "PATH")]
        from: Option<PathBuf>,
        /// Do nothing when the installed keyd was built from the same source
        /// and the agent already runs it
        #[arg(long)]
        if_changed: bool,
    },
    /// Whether keyd is installed, loaded, current and answering
    ///
    /// Sends keyd a `ping`, which starts it if launchd has it loaded. Never
    /// reads or prints a stored key.
    Status,
    /// Unload keyd and remove its LaunchAgent, binary and stamp
    ///
    /// The vault and the keychain's master key stay, so a reinstall finds
    /// every stored key again.
    Uninstall,
}

#[derive(Args)]
pub(crate) struct ListArgs {
    /// List subjects (default)
    #[arg(short = 's', long)]
    pub(crate) subjects: bool,
    /// List lectures, optionally filtered to the given subject codes
    #[arg(short = 'l', long)]
    pub(crate) lectures: bool,
    /// Refresh the subject list from Canvas before printing
    #[arg(long)]
    pub(crate) refresh: bool,
    /// Subject codes to filter by
    #[arg(value_name = "SUBJECT_CODE")]
    pub(crate) codes: Vec<String>,
}

#[derive(Args)]
pub(crate) struct RunArgs {
    /// Scrape Canvas content: pages, announcements, modules, PDFs (default)
    #[arg(short = 's', long)]
    pub(crate) subjects: bool,
    /// Sync the Echo360 lecture list for the given subjects
    #[arg(short = 'l', long)]
    pub(crate) lectures: bool,
    /// Include subjects from past terms, not just the current one
    #[arg(long)]
    pub(crate) all: bool,
    /// Skip PDF processing entirely: no parsing and no embedding
    #[arg(long)]
    pub(crate) no_parse: bool,
    /// Parse PDFs but do not embed them into the retrieval index
    #[arg(long)]
    pub(crate) no_embed: bool,
    /// With -l: also download and trim the lecture videos
    #[arg(long)]
    pub(crate) videos: bool,
    /// With -l: also download the lecture transcripts
    #[arg(long)]
    pub(crate) transcripts: bool,
    /// Subject codes to sync. Omit for every selected current subject.
    #[arg(value_name = "SUBJECT_CODE")]
    pub(crate) codes: Vec<String>,
}

/// Transcribe a video that has no captions, on Groq or on this Mac
///
/// Extracts the video's audio, transcribes it, and writes the timed text
/// beside the video as `<video>.vtt` — `Week 1.mp4` gets `Week 1.mp4.vtt`.
/// Three engines, tried in the order set in Settings → Transcription —
/// Groq, local Whisper, then on-device speech unless changed there — and one
/// switched off there is skipped. Whisper on Groq's free tier needs a Groq
/// API key saved in Settings (the audio is uploaded, split into parts by time
/// when it is too large for one upload); local Whisper needs a model
/// downloaded there; Apple's on-device speech recognition needs macOS 26 or
/// later. Both local engines are free and keep the audio on this Mac. Every
/// engine transcribes in the language set there, English by default.
///
/// Groq's free tier caps how many seconds of audio it takes an hour and a
/// day; past that the run moves on to the next engine, or, with no engine
/// left, fails and says when to try again. Any other failure ends the run.
/// Nothing is written unless every part comes back, and nothing is recorded
/// in the database.
#[derive(Args)]
pub(crate) struct TranscribeArgs {
    /// A video inside the library: absolute, relative to this directory, or
    /// relative to the library root
    #[arg(value_name = "VIDEO")]
    pub(crate) video: String,
    /// Use only this engine, with no fallback (default: the first in the
    /// Settings order that answers)
    #[arg(long, value_parser = ["groq", "whisper", "apple"])]
    pub(crate) engine: Option<String>,
    /// Transcribe again when the `.vtt` already exists, replacing it
    #[arg(long)]
    pub(crate) force: bool,
}

/// Write the agent-facing docs into the library
///
/// Fills `agents/` in the data directory: `OCULUS-CLI.md`, rendered from this
/// binary's own `--help` so it can never drift from the flags it documents,
/// one `AGENTS.md` symlinked into every course folder, and the skills.
///
/// It also brings the two generated-but-shared files up to date.
/// `TASTE.md`'s guidance is re-rendered with the preferences you wrote in it
/// carried across — a file that is half prompt and half content cannot be
/// "written once and never again" without the prompt half going stale. Every
/// `MEMORY.md` index is rewritten from the memories beside it. `OCULUS.md` is
/// the one stub nothing here ever touches twice.
///
/// Idempotent, and run by the dev preflight and by `cli:install`, so a
/// library's instructions always match the binary that is installed.
#[derive(Args)]
pub(crate) struct DocsArgs {
    /// Print the markdown instead of writing the file
    #[arg(long)]
    pub(crate) stdout: bool,
}
