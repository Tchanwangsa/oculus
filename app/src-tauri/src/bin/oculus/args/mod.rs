//! The clap command tree: every subcommand, flag and its `--help` text.
//!
//! `Cli` and the `Command` enum stay in this one file: the variant order is the
//! `--help` order, and `docs/cli-reference.md` is generated from it. The
//! per-command argument structs live in the files beside it.

use crate::*;

mod lecture;
mod memory;
mod misc;
mod planning;
mod query;

pub(crate) use lecture::*;
pub(crate) use memory::*;
pub(crate) use misc::*;
pub(crate) use planning::*;
pub(crate) use query::*;

#[derive(Parser)]
#[command(
    name = "oculus",
    version,
    about = "Sync Canvas subjects and lectures into your local Oculus library"
)]
pub(crate) struct Cli {
    /// Print machine-readable JSON instead of formatted text
    ///
    /// Honoured by every command that prints: status, list, search, grep,
    /// read, files, calendar, project, task and keyd. On failure the JSON is
    /// `{"error": "..."}` on stderr and the exit code is 1.
    #[arg(long, global = true)]
    pub(crate) json: bool,
    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Session, library and parse status
    Status,
    /// Sign in to Canvas, or sign out
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// Install, inspect or remove oculus-keyd, the credential broker
    Keyd {
        #[command(subcommand)]
        action: KeydAction,
    },
    /// List subjects or lectures
    List(ListArgs),
    /// Scrape Canvas content or sync lectures
    Run(RunArgs),
    /// Re-parse and re-embed PDFs already on record
    Index(IndexArgs),
    // No doc comments here: clap would let the variant's text shadow the fuller
    // help on the Args struct.
    Search(SearchArgs),
    Grep(GrepArgs),
    Read(ReadArgs),
    Files(FilesArgs),
    Calendar(CalendarArgs),
    /// Plan work: projects, their boards, and what is on them
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },
    /// Add, move, refile, finish and delete tasks, on a board or on none
    Task {
        #[command(subcommand)]
        action: TaskAction,
    },
    /// Read and write the memory store the agents keep in the library
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
    /// Look inside a downloaded lecture recording
    Lecture {
        #[command(subcommand)]
        action: LectureAction,
    },
    Transcribe(TranscribeArgs),
    Docs(DocsArgs),
    Agent(AgentArgs),
}
