//! `oculus` — the Oculus command line.
//!
//! Same engine the app runs, without the window: it reads the session cookie
//! and the database the app already maintains, so a CLI sync and an in-app sync
//! are the same operation and either can follow the other.

use std::collections::HashMap;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use app_lib::agents;
use app_lib::paths;
use app_lib::projects;
use app_lib::store;
use app_lib::sync::{self, Engine, FileEvent, Progress, Reporter};
use clap::{Args, CommandFactory, Parser, Subcommand};
use serde::Serialize;
use sqlx::{Row, SqlitePool};
use tokio::runtime::Runtime;

#[derive(Parser)]
#[command(
    name = "oculus",
    version,
    about = "Sync Canvas subjects and lectures into your local Oculus library"
)]
struct Cli {
    /// Print machine-readable JSON instead of formatted text
    ///
    /// Honoured by every command that prints: status, list, search, grep,
    /// read, files, calendar, project and task. On failure the JSON is
    /// `{"error": "..."}` on stderr and the exit code is 1.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    /// The reference is only trustworthy if it covers everything, so a new
    /// subcommand that nobody remembers to document still fails this.
    #[test]
    fn generated_docs_cover_every_command() {
        let markdown = render_cli_docs();
        let mut root = Cli::command();
        root.build();
        for sub in root.get_subcommands() {
            if sub.get_name() == "help" {
                continue;
            }
            let heading = format!("## `oculus {}`", sub.get_name());
            assert!(markdown.contains(&heading), "missing {heading}");
        }
        assert!(markdown.contains("### `oculus auth login`"), "nested commands");
        assert!(
            markdown.contains("### `oculus lecture reading`"),
            "lecture reading command"
        );
        assert!(!markdown.contains('\u{1b}'), "no ANSI escapes in a file");
    }

    /// Global options are worth one paragraph, not fifteen. `--json` is the
    /// only one left now that `--memory-cap` has gone with the sidecar, so it
    /// carries the rule on its own: a global flag repeated under every
    /// subcommand would fail this.
    #[test]
    fn generated_docs_list_global_options_once() {
        let markdown = render_cli_docs();
        assert_eq!(markdown.matches("      --json").count(), 1);
    }

    #[test]
    fn lecture_reading_accepts_one_run_overrides() {
        let cli = Cli::try_parse_from([
            "oculus",
            "lecture",
            "reading",
            "a1b2c3d4",
            "--force",
            "--provider",
            "codex",
            "--model",
            "gpt-example",
            "--effort",
            "medium",
        ])
        .expect("reading flags");
        let Some(Command::Lecture {
            action: LectureAction::Reading(args),
        }) = cli.command
        else {
            panic!("lecture reading command");
        };
        assert_eq!(args.id, "a1b2c3d4");
        assert_eq!(args.provider.as_deref(), Some("codex"));
        assert_eq!(args.model.as_deref(), Some("gpt-example"));
        assert_eq!(args.effort.as_deref(), Some("medium"));
        assert!(args.force);
    }
}

#[derive(Subcommand)]
enum Command {
    /// Session, library and parse status
    Status,
    /// Sign in to Canvas, or sign out
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// List subjects or lectures
    List(ListArgs),
    /// Scrape Canvas content or sync lectures
    Run(RunArgs),
    /// Re-parse and re-embed PDFs already on record
    Index(IndexArgs),
    // No doc comments on these five: clap would take the variant's text for
    // both `about` and `long_about` and shadow the fuller help on the Args
    // struct, where each command explains what it needs and what it costs.
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
    /// Look inside a downloaded lecture recording
    Lecture {
        #[command(subcommand)]
        action: LectureAction,
    },
    Docs(DocsArgs),
    Agent(AgentArgs),
}

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
struct AgentArgs {
    /// What to ask
    #[arg(value_name = "PROMPT")]
    prompt: String,
    /// Which CLI to drive
    #[arg(short, long, value_parser = ["claude", "codex", "opencode", "antigravity"], default_value = "claude")]
    provider: String,
    /// Model to request (provider-specific name or alias)
    #[arg(short, long)]
    model: Option<String>,
    /// Codex reasoning effort (low, medium, high, xhigh)
    #[arg(long)]
    effort: Option<String>,
    /// Scope the turn to one subject, as the app's chat does
    #[arg(short = 's', long, value_name = "SUBJECT_CODE")]
    subject: Option<String>,
}

#[derive(Args)]
struct IndexArgs {
    /// Subject codes to index. Omit for every subject.
    #[arg(value_name = "SUBJECT_CODE")]
    codes: Vec<String>,
}

#[derive(Subcommand)]
enum AuthAction {
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
    Auto,
    /// One keep-alive cycle: roll the session forward, rebuild it if it died
    ///
    /// What the LaunchAgent runs every few hours. Prints nothing and always
    /// exits 0 — it reports into `session-keepalive.log` in the data dir,
    /// because launchd has nowhere to show a failure and a non-zero exit only
    /// makes launchd think the job crashed.
    Tick,
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

#[derive(Args)]
struct ListArgs {
    /// List subjects (default)
    #[arg(short = 's', long)]
    subjects: bool,
    /// List lectures, optionally filtered to the given subject codes
    #[arg(short = 'l', long)]
    lectures: bool,
    /// Refresh the subject list from Canvas before printing
    #[arg(long)]
    refresh: bool,
    /// Subject codes to filter by
    #[arg(value_name = "SUBJECT_CODE")]
    codes: Vec<String>,
}

#[derive(Args)]
struct RunArgs {
    /// Scrape Canvas content: pages, announcements, modules, PDFs (default)
    #[arg(short = 's', long)]
    subjects: bool,
    /// Sync the Echo360 lecture list for the given subjects
    #[arg(short = 'l', long)]
    lectures: bool,
    /// Include subjects from past terms, not just the current one
    #[arg(long)]
    all: bool,
    /// Skip PDF processing entirely: no parsing and no embedding
    #[arg(long)]
    no_parse: bool,
    /// Parse PDFs but do not embed them into the retrieval index
    #[arg(long)]
    no_embed: bool,
    /// With -l: also download and trim the lecture videos
    #[arg(long)]
    videos: bool,
    /// With -l: also download the lecture transcripts
    #[arg(long)]
    transcripts: bool,
    /// Subject codes to sync. Omit for every selected current subject.
    #[arg(value_name = "SUBJECT_CODE")]
    codes: Vec<String>,
}

// ── Read-only query commands ─────────────────────────────────────────────────
//
// `search`, `grep`, `read`, `files` and `calendar` are the library's query
// surface: everything an agent — or a person in a terminal — needs to find
// coursework and quote it, without the app's UI and without a protocol in
// between. They only read; nothing here scrapes, parses or writes.
//
// The `--help` text is the whole interface documentation for an agent that
// has never seen this binary, so it says what each command needs and what it
// costs, not just what it does.

/// Search the library by meaning (needs network and an API key).
///
/// The query is embedded by the same vision model that embedded every page
/// image, so this finds a slide about Lagrange multipliers when you ask for
/// "constrained optimisation". Embedding happens in the cloud, so this needs
/// a network connection and the Voyage key from Settings → Library; without
/// either, and over an index that is empty or built by a retired model, it
/// fails loudly and points at `oculus grep`, which searches the same text
/// with no model at all.
///
/// Only PDF and Office pages are ranked here — Canvas pages, announcements
/// and Ed threads are markdown on disk and are covered by `oculus grep`.
#[derive(Args)]
struct SearchArgs {
    /// What to look for, in plain language
    #[arg(value_name = "QUERY")]
    query: String,
    /// Restrict to one subject; prefix codes are fine (MULT20015)
    #[arg(short = 's', long, value_name = "SUBJECT_CODE")]
    subject: Option<String>,
    /// How many pages to return (1-50)
    #[arg(short = 'n', long, default_value_t = 8)]
    limit: i64,
    /// Print each hit's whole page instead of a one-line snippet
    #[arg(long)]
    full: bool,
}

/// Search the library by pattern (offline, no model).
///
/// Covers both halves of the library: the markdown on disk (Canvas pages,
/// announcements, assignments, Ed threads) and the page text extracted from
/// PDFs, which lives only in the database — ripgrep over the library
/// directory cannot see it, which is why this exists.
///
/// Needs no network and no model, so it is the fallback whenever `oculus
/// search` cannot run. The pattern is a regular expression by default and
/// case-insensitive unless you ask otherwise.
#[derive(Args)]
struct GrepArgs {
    /// Regular expression to look for
    #[arg(value_name = "PATTERN")]
    pattern: String,
    /// Restrict to these subjects; prefix codes are fine. Repeatable.
    #[arg(short = 's', long, value_name = "SUBJECT_CODE")]
    subject: Vec<String>,
    #[arg(short = 'c', long, value_name = "CATEGORY", help = category_help())]
    category: Vec<String>,
    /// Treat the pattern as literal text, not a regular expression
    #[arg(short = 'F', long)]
    fixed: bool,
    /// Match case exactly
    #[arg(long)]
    case_sensitive: bool,
    /// Print matching file paths only, one per line
    #[arg(short = 'l', long)]
    files_with_matches: bool,
    /// Stop after this many matches
    #[arg(short = 'n', long, default_value_t = 40)]
    limit: usize,
}

/// Print the text of one library file.
///
/// For a PDF or Office document this is the parsed page markdown from the
/// database, so `--pages` addresses the same page numbers `oculus search`
/// and the app's viewer report. For markdown and other text it is the file on
/// disk. A PDF that has never been parsed says so rather than printing
/// nothing — run `oculus index <SUBJECT_CODE>` for it.
///
/// FILE may be a full library path, a bare filename, or any distinctive
/// fragment of either. An ambiguous fragment lists the candidates instead of
/// guessing.
#[derive(Args)]
struct ReadArgs {
    /// Library path, filename, or a fragment of either
    #[arg(value_name = "FILE")]
    file: String,
    /// Pages to print: 12, 12-15, 12,14,20-22, or 30- for "30 to the end"
    #[arg(short = 'p', long, value_name = "RANGE")]
    pages: Option<String>,
    /// Disambiguate by subject; prefix codes are fine
    #[arg(short = 's', long, value_name = "SUBJECT_CODE")]
    subject: Option<String>,
}

/// List the files in the library.
///
/// The `indexed` column is how many pages of a document are searchable; a
/// PDF showing none has not been parsed yet.
#[derive(Args)]
struct FilesArgs {
    /// Subjects to list. Omit for every subject.
    #[arg(value_name = "SUBJECT_CODE")]
    codes: Vec<String>,
    /// Only this extension (pdf, md, pptx, docx, png …)
    #[arg(short = 't', long, value_name = "EXT")]
    r#type: Option<String>,
    #[arg(short = 'c', long, value_name = "CATEGORY", help = category_help())]
    category: Vec<String>,
    /// Only paths containing this text (case-insensitive)
    #[arg(short = 'm', long, value_name = "TEXT")]
    r#match: Option<String>,
    /// Only files with pages in the retrieval index
    #[arg(long)]
    indexed: bool,
    /// Stop after this many files
    #[arg(short = 'n', long, default_value_t = 200)]
    limit: usize,
}

/// Class times and assignment due dates.
///
/// Sourced from each subject's Canvas calendar, refreshed by `oculus run -s`.
/// Times are shown in this machine's local timezone; `--json` also carries
/// the raw UTC timestamp.
#[derive(Args)]
struct CalendarArgs {
    /// Subjects to include. Omit for every subject.
    #[arg(value_name = "SUBJECT_CODE")]
    codes: Vec<String>,
    /// How far ahead to look
    #[arg(short = 'd', long, default_value_t = 14, value_name = "DAYS")]
    days: i64,
    /// Only assignment due dates, not class times
    #[arg(long)]
    due: bool,
    /// Include events that have already happened
    #[arg(long)]
    past: bool,
}

// ── Planning: projects and tasks ─────────────────────────────────────────────
//
// The write half of the agent's surface. `search`, `grep` and `read` answer
// "what does the library say"; these answer "what am I doing about it" — a
// project is a piece of work, its board is columns of tasks, and a task may
// have one level of subtask under it.
//
// The database is the only door: the app's board reads these same rows live,
// which is why nothing here asks for `oculus.db` to be opened directly.

#[derive(Subcommand)]
enum ProjectAction {
    List(ProjectListArgs),
    Show(ProjectShowArgs),
    Create(ProjectCreateArgs),
    Update(ProjectUpdateArgs),
}

/// List projects and how far along they are.
///
/// Active projects only, unless `--archived`. Each line starts with the id
/// every other project and task command takes, and ends with finished/total
/// tasks.
#[derive(Args)]
struct ProjectListArgs {
    /// Only this subject's projects; prefix codes are fine (COMP30026)
    #[arg(short = 's', long, value_name = "SUBJECT_CODE")]
    subject: Option<String>,
    /// Only projects belonging to no subject
    #[arg(long, conflicts_with = "subject")]
    personal: bool,
    /// Archived projects instead of active ones
    #[arg(long)]
    archived: bool,
}

/// Show one project: its brief, its board, and every task on it.
///
/// Tasks are printed under their column in the board's own order, subtasks
/// indented under their parent. The bracketed name after each column heading
/// is the column **id** — that is what `--column` takes.
#[derive(Args)]
struct ProjectShowArgs {
    /// Project id, as `oculus project list` prints it
    #[arg(value_name = "ID")]
    id: i64,
}

/// Create a project.
///
/// It opens with the app's default board — `backlog`, `todo`, `doing`, `done`
/// — and no tasks; `oculus task add --batch` is how a breakdown goes in. Rows
/// written by this binary are marked `source: agent`, so the board can show
/// what it did not write itself.
///
/// Prints the new project's id.
#[derive(Args)]
struct ProjectCreateArgs {
    /// What the project is called
    #[arg(value_name = "NAME")]
    name: String,
    /// Scope it to a subject; prefix codes are fine (COMP30026), and the
    /// current term wins a tie. Omit for a personal project.
    #[arg(short = 's', long, value_name = "SUBJECT_CODE")]
    subject: Option<String>,
    /// When the whole thing is due, ISO 8601 (2026-09-20T23:59:00Z)
    #[arg(long, value_name = "ISO")]
    due: Option<String>,
    /// When work on it starts, ISO 8601
    #[arg(long, value_name = "ISO")]
    starts: Option<String>,
    /// A paragraph of what it is — the assignment brief, the plan
    #[arg(long, value_name = "TEXT")]
    brief: Option<String>,
    /// Comma-separated labels for the About page (report,group,week-5)
    #[arg(long, value_name = "TAGS")]
    tags: Option<String>,
}

/// Change a project's name, dates, brief, tags or status.
///
/// Only the flags you pass are written; everything else is left alone. Pass an
/// **empty string** to clear a field: `--due ""` takes the due date off.
///
/// `--status archived` is how a project leaves the board without being
/// deleted; its tasks stay and `--status active` brings it back.
#[derive(Args)]
struct ProjectUpdateArgs {
    /// Project id
    #[arg(value_name = "ID")]
    id: i64,
    /// Rename it
    #[arg(long, value_name = "NAME")]
    name: Option<String>,
    /// Due date, ISO 8601, or "" to clear
    #[arg(long, value_name = "ISO")]
    due: Option<String>,
    /// Start date, ISO 8601, or "" to clear
    #[arg(long, value_name = "ISO")]
    starts: Option<String>,
    /// Replace the brief, or "" to clear it
    #[arg(long, value_name = "TEXT")]
    brief: Option<String>,
    /// active or archived
    #[arg(long, value_parser = ["active", "archived"])]
    status: Option<String>,
    /// Replace every tag with this comma-separated list, or "" to clear them.
    /// There is no add/remove: the whole set is written at once, the same way
    /// the About page's editor writes it.
    #[arg(long, value_name = "TAGS")]
    tags: Option<String>,
}

#[derive(Subcommand)]
enum TaskAction {
    List(TaskListArgs),
    Add(TaskAddArgs),
    Update(TaskUpdateArgs),
    Move(TaskMoveArgs),
    Refile(TaskRefileArgs),
    Rm(TaskRmArgs),
}

/// List tasks — one project's, or every task there is.
///
/// Grouped by column in the board's order, subtasks under their parent. A task
/// sitting in a `done` column carries the time it landed there.
///
/// Without `-p` this spans **every** project and includes the tasks that
/// belong to none, printed as one board per project under its name, with the
/// unfiled ones first. `--unfiled` lists only those: the pile with no board of
/// its own, which is the one that needs looking at.
#[derive(Args)]
struct TaskListArgs {
    /// Which project (omit for every task in the library)
    #[arg(short = 'p', long, value_name = "ID")]
    project: Option<i64>,
    /// Only tasks that belong to no project at all
    #[arg(long, conflicts_with = "project")]
    unfiled: bool,
    /// Only this board column (its id, e.g. todo) — needs --project, since a
    /// column id only means something against one board
    #[arg(short = 'c', long, value_name = "ID", requires = "project")]
    column: Option<String>,
    /// Only tasks due before this ISO 8601 timestamp. Compared as text, so
    /// pass the same shape the dates were written in (UTC, usually).
    #[arg(long, value_name = "ISO")]
    due_before: Option<String>,
}

/// Add one task, or a whole breakdown in one call.
///
/// **Without `-p` the task belongs to no project at all** — the same thing the
/// app's Tasks page writes by default, and the answer to "write this down, I
/// have not decided where it goes". That is the absence of a project, not a
/// project called Inbox, so nothing needs cleaning up if it is never filed;
/// `oculus task refile` files it later. Its board is the default one
/// (`backlog`, `todo`, `doing`, `done`), so filing it into a project created by
/// this binary needs no translation.
///
/// A task lands at the end of its column; without `--column` that is the first
/// column of its board. The column id is checked against that board and an
/// unknown one is refused, listing the ids it does have — a task filed under a
/// column that does not exist is drawn by nothing, in any view. Landing in a
/// `done` column marks the task finished, exactly as moving it there would.
///
/// `--parent` makes the task a subtask. Subtasks are one level deep: a subtask
/// cannot itself be given children.
///
/// BREAKDOWNS: `--batch -` reads a JSON array of tasks from stdin (or a file,
/// `--batch tasks.json`) and writes them in one call — use it for anything
/// past two or three:
///
///   [{"title":"Read the brief","column":"todo","due":"2026-09-20T23:59:00Z"},
///   {"title":"Outline","key":"outline"}, {"title":"Draft intro","parent":"outline"}]
///
/// Per task: `title` (required), `column`, `body`, `due`, `starts`,
/// `estimate` (minutes), `parent`, `key`. `parent` is either an existing
/// task's id (a number) or the `key` of an **earlier task in the same batch**,
/// which is how a parent and its subtasks go in together. `key` is never
/// stored. An unknown field is an error rather than a silent no-op.
///
/// The batch is **all or nothing**: one transaction, so a bad item — unknown
/// column, a parent that is already a subtask, a date that is not a date —
/// writes none of them and says which item failed. Fix it and re-send; it can
/// never leave half a breakdown on the board.
///
/// Prints the new task ids in the order they were given.
#[derive(Args)]
struct TaskAddArgs {
    /// Which project (omit to file it nowhere)
    #[arg(short = 'p', long, value_name = "ID")]
    project: Option<i64>,
    /// The task's title. Omit when using --batch.
    #[arg(value_name = "TITLE")]
    title: Option<String>,
    /// Board column id (default: the first column of its board)
    #[arg(short = 'c', long, value_name = "ID")]
    column: Option<String>,
    /// Make this a subtask of that task id
    #[arg(long, value_name = "TASK_ID")]
    parent: Option<i64>,
    /// Due date, ISO 8601
    #[arg(long, value_name = "ISO")]
    due: Option<String>,
    /// Start date, ISO 8601
    #[arg(long, value_name = "ISO")]
    starts: Option<String>,
    /// How long you think it will take, in minutes
    #[arg(long, value_name = "MIN")]
    estimate: Option<i64>,
    /// Notes on the task
    #[arg(long, value_name = "TEXT")]
    body: Option<String>,
    /// Read a JSON array of tasks from stdin (-) or a file
    #[arg(long, value_name = "FILE", conflicts_with_all = ["title", "column", "parent", "due", "starts", "estimate", "body"])]
    batch: Option<String>,
}

/// Change a task's title, notes, dates or estimate.
///
/// Only the flags you pass are written. Pass an **empty string** to clear a
/// field: `--due ""`, `--estimate ""`.
///
/// Where a task *sits* is not here: column, order and done-ness are one fact,
/// and `oculus task move` is their only writer — it is the command that reads
/// the board to learn whether the destination column means finished.
#[derive(Args)]
struct TaskUpdateArgs {
    /// Task id
    #[arg(value_name = "ID")]
    id: i64,
    /// Rename it
    #[arg(long, value_name = "TEXT")]
    title: Option<String>,
    /// Replace the notes, or "" to clear
    #[arg(long, value_name = "TEXT")]
    body: Option<String>,
    /// Due date, ISO 8601, or "" to clear
    #[arg(long, value_name = "ISO")]
    due: Option<String>,
    /// Start date, ISO 8601, or "" to clear
    #[arg(long, value_name = "ISO")]
    starts: Option<String>,
    /// Minutes, or "" to clear
    #[arg(long, value_name = "MIN")]
    estimate: Option<String>,
}

/// Move a task to another column, or reorder it within one.
///
/// This is also how a task is finished: landing in a column whose kind is
/// `done` stamps it, and leaving one clears that again. The column's *kind*
/// decides, not its name — which is why there is no `--done` flag anywhere.
///
/// Without `--after` or `--before` the task goes to the end of the column.
/// Both name tasks already in the destination column: `--after 12` puts it
/// straight below task 12, `--before 12` straight above it.
#[derive(Args)]
struct TaskMoveArgs {
    /// Task id
    #[arg(value_name = "ID")]
    id: i64,
    /// Destination column id (e.g. done)
    #[arg(short = 'c', long, value_name = "ID")]
    column: String,
    /// Put it directly below this task
    #[arg(long, value_name = "TASK_ID")]
    after: Option<i64>,
    /// Put it directly above this task
    #[arg(long, value_name = "TASK_ID")]
    before: Option<i64>,
}

/// File a task under another project, or under none at all.
///
/// The one command that changes which project a task belongs to. It takes the
/// task's **subtasks with it** — a subtask sits in its parent's project, so
/// there is no honest half of this move, and a subtask on its own is refused
/// and names its parent instead.
///
/// The column maps across by *kind*: a task in a column that means "in flight"
/// lands in the **first** column of that kind on the destination's board, so
/// entering a kind puts you at its start. A board with no column of that kind —
/// no Done column for a finished task — is refused rather than given the
/// nearest one; there is no nearest kind. Whether the task is finished follows
/// the column it lands in, as it does everywhere else.
///
/// It lands at the **end** of that column: `position` is an order inside one
/// project's column and means nothing across two, so there is no slot in the
/// destination to aim at. `oculus task move` is how it is then placed.
#[derive(Args)]
struct TaskRefileArgs {
    /// Task id
    #[arg(value_name = "ID")]
    id: i64,
    /// File it under this project
    #[arg(short = 'p', long, value_name = "ID")]
    project: Option<i64>,
    /// Take it out of every project instead
    #[arg(long, conflicts_with = "project")]
    unfiled: bool,
}

/// Delete a task, and its subtasks with it.
///
/// There is no undo, and nothing else cleans these up — a task that is merely
/// finished belongs in a `done` column (`oculus task move`), not deleted.
#[derive(Args)]
struct TaskRmArgs {
    /// Task id
    #[arg(value_name = "ID")]
    id: i64,
}

// ── Lectures: what is inside a recording ───────────────────────────────────
//
// `run -l` puts recordings on disk; this reads one back. Nothing here touches
// the database or the network — it decodes the file that is already there.

#[derive(Subcommand)]
enum LectureAction {
    Candidates(LectureCandidatesArgs),
    Chapters(LectureChaptersArgs),
    Reading(LectureReadingArgs),
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
struct LectureCandidatesArgs {
    /// Lecture id, as `oculus list -l` prints it; a unique prefix is enough
    #[arg(value_name = "LECTURE_ID")]
    id: String,
    /// Also write one JPEG per candidate into the lecture's `frames/` folder,
    /// so the boundaries can be checked by eye
    #[arg(long)]
    frames: bool,
    /// Which captured stream to read — 1 or 2. Default: source 1, unless it
    /// turns out to be dead, in which case source 2 if it is downloaded
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u8).range(1..=2))]
    source: Option<u8>,
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
struct LectureChaptersArgs {
    /// Lecture id, as `oculus list -l` prints it; a unique prefix is enough
    #[arg(value_name = "LECTURE_ID")]
    id: String,
    // None of the three has a default here any more: the job's agent, model
    // and level are configured in Settings → AI and read from the `job_models`
    // registry (`app/src-tauri/src/harness/jobs.rs`), so the app and the CLI
    // run the same thing. A flag overrides that selection for one run.
    /// Which CLI to drive (default: the configured one)
    #[arg(short, long, value_parser = ["claude", "codex", "opencode"])]
    provider: Option<String>,
    /// Model to request (default: the configured one)
    #[arg(short, long)]
    model: Option<String>,
    /// Reasoning effort — low, medium, high, xhigh, max (default: the
    /// configured one)
    #[arg(long)]
    effort: Option<String>,
    /// Re-run over a lecture that already has chapters, replacing them
    #[arg(long)]
    force: bool,
    /// Which captured stream to read — 1 or 2. Default: source 1, unless it
    /// turns out to be dead, in which case source 2 if it is downloaded
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u8).range(1..=2))]
    source: Option<u8>,
}

/// Write a recording's reading copy with a CLI agent
///
/// Rewrites the transcript as text a student can read: one sentence per
/// line, each pinned to the second it was said, with spoken maths set as
/// maths and speech-recognition errors fixed from the slide. The lecture is
/// split at its slide changes — which become paragraph breaks — and grouped
/// into roughly ten-minute windows, one agent turn each. Each window is
/// validated and written before the next starts, so a long run has useful
/// partial results if a later window fails.
///
/// Unlike chapter naming, this needs the transcript: the reading copy is the
/// transcript, rewritten. The recording and transcript must both have been
/// downloaded first.
#[derive(Args)]
struct LectureReadingArgs {
    /// Lecture id, as `oculus list -l` prints it; a unique prefix is enough
    #[arg(value_name = "LECTURE_ID")]
    id: String,
    // The job registry supplies the defaults shared with the app. A flag
    // replaces only the named part for this run, exactly as chapter naming
    // does above.
    /// Which CLI to drive (default: the configured one)
    #[arg(short, long, value_parser = ["claude", "codex", "opencode"])]
    provider: Option<String>,
    /// Model to request (default: the configured one)
    #[arg(short, long)]
    model: Option<String>,
    /// Reasoning effort — low, medium, high, xhigh, max (default: the
    /// configured one)
    #[arg(long)]
    effort: Option<String>,
    /// Re-run over a lecture that already has a reading copy, replacing it
    #[arg(long)]
    force: bool,
    /// Which captured stream to read — 1 or 2. Default: source 1, unless it
    /// turns out to be dead, in which case source 2 if it is downloaded
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u8).range(1..=2))]
    source: Option<u8>,
}

/// Write the agent-facing docs into the library
///
/// Fills `agents/` in the data directory: `OCULUS-CLI.md`, rendered from this
/// binary's own `--help` so it can never drift from the flags it documents,
/// and one `AGENTS.md` symlinked into every course folder. `OCULUS.md`,
/// `TASTE.md` and the `MEMORY.md` index in each memory folder are stubbed on
/// first run and never touched again — they are what an agent writes back to.
///
/// Idempotent, and run by `cli:install`, so the docs always describe the
/// binary that is actually installed.
#[derive(Args)]
struct DocsArgs {
    /// Print the markdown instead of writing the file
    #[arg(long)]
    stdout: bool,
}

fn main() {
    restore_sigpipe();
    let cli = Cli::parse();
    let ctx = Ctx::new(cli.json);

    let result = match cli.command {
        None | Some(Command::Status) => ctx.status(),
        Some(Command::Auth { action }) => match action {
            AuthAction::Login => ctx.login(),
            AuthAction::Logout => ctx.logout(),
            AuthAction::Status => ctx.auth_status(),
            AuthAction::Setup => ctx.auth_setup(),
            AuthAction::Auto => ctx.auth_auto(),
            AuthAction::Tick => ctx.auth_tick(),
            AuthAction::Forget => ctx.auth_forget(),
            AuthAction::Diagnose => {
                print!("{}", app_lib::okta::diagnose());
                Ok(())
            }
            AuthAction::Ed { token } => ctx.auth_ed(token.as_deref()),
        },
        Some(Command::List(args)) => ctx.list(args),
        Some(Command::Run(args)) => ctx.run(args),
        Some(Command::Index(args)) => ctx.index(&args),
        Some(Command::Search(args)) => ctx.search(&args),
        Some(Command::Grep(args)) => ctx.grep(&args),
        Some(Command::Read(args)) => ctx.read(&args),
        Some(Command::Files(args)) => ctx.files(&args),
        Some(Command::Calendar(args)) => ctx.calendar(&args),
        Some(Command::Project { action }) => match action {
            ProjectAction::List(a) => ctx.project_list(&a),
            ProjectAction::Show(a) => ctx.project_show(&a),
            ProjectAction::Create(a) => ctx.project_create(&a),
            ProjectAction::Update(a) => ctx.project_update(&a),
        },
        Some(Command::Task { action }) => match action {
            TaskAction::List(a) => ctx.task_list(&a),
            TaskAction::Add(a) => ctx.task_add(&a),
            TaskAction::Update(a) => ctx.task_update(&a),
            TaskAction::Move(a) => ctx.task_move(&a),
            TaskAction::Refile(a) => ctx.task_refile(&a),
            TaskAction::Rm(a) => ctx.task_rm(&a),
        },
        Some(Command::Lecture { action }) => match action {
            LectureAction::Candidates(a) => ctx.lecture_candidates(&a),
            LectureAction::Chapters(a) => ctx.lecture_chapters(&a),
            LectureAction::Reading(a) => ctx.lecture_reading(&a),
        },
        Some(Command::Docs(args)) => ctx.docs(&args),
        Some(Command::Agent(args)) => ctx.agent(&args),
    };

    if let Err(e) = result {
        // Machine-readable failures too: an agent parsing stdout should not
        // have to fall back to reading prose to find out what went wrong.
        if cli.json {
            eprintln!("{}", serde_json::json!({ "error": e }));
        } else {
            eprintln!("{} {e}", paint("error:", RED));
        }
        std::process::exit(1);
    }
}

fn read_line(prompt: &str) -> Result<String, String> {
    use std::io::Write;
    print!("{prompt}");
    std::io::stdout().flush().ok();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).map_err(|e| e.to_string())?;
    Ok(line.trim().to_string())
}

/// Prompt without echoing. `stty` rather than a crate — this is the only
/// place in the CLI that needs it, and echo is restored even if the read
/// fails, so a stray Ctrl-C cannot leave the terminal mute.
fn read_secret(prompt: &str) -> Result<String, String> {
    use std::io::Write;
    print!("{prompt}");
    std::io::stdout().flush().ok();

    let hidden = std::process::Command::new("stty").arg("-echo").status().is_ok();
    let mut line = String::new();
    let read = std::io::stdin().read_line(&mut line);
    if hidden {
        std::process::Command::new("stty").arg("echo").status().ok();
        println!();
    }
    read.map_err(|e| e.to_string())?;

    let value = line.trim().to_string();
    if value.is_empty() {
        return Err("nothing entered".to_string());
    }
    Ok(value)
}

/// Rust ignores SIGPIPE at startup, which turns `oculus list | head` into a
/// panic on a closed pipe instead of a quiet exit. Put the default back.
fn restore_sigpipe() {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

// ── Shared context ───────────────────────────────────────────────────────────

struct Ctx {
    data_dir: PathBuf,
    rt: Runtime,
    json: bool,
}

impl Ctx {
    fn new(json: bool) -> Self {
        Ctx {
            data_dir: app_lib::paths::data_dir(),
            rt: Runtime::new().expect("tokio runtime"),
            json,
        }
    }

    /// One JSON document on stdout. Pretty-printed: these outputs are read by
    /// people as often as by agents, and the extra bytes cost nothing.
    fn emit(&self, value: &impl Serialize) -> Result<(), String> {
        let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
        println!("{text}");
        Ok(())
    }

    fn engine(&self, parse: bool) -> Engine {
        Engine::new(&self.data_dir, Box::new(TermReporter::new())).with_pdf_parsing(parse)
    }

    /// The database, or `None` with a warning printed. A missing database is
    /// not fatal: scraping still writes the library to disk.
    fn db(&self) -> Option<SqlitePool> {
        match self.rt.block_on(store::open(&self.data_dir)) {
            Ok(p) => Some(p),
            Err(e) => {
                eprintln!("{} {e}", paint("warning:", YELLOW));
                None
            }
        }
    }

    // ── status ───────────────────────────────────────────────────────────────

    fn status(&self) -> Result<(), String> {
        #[derive(Serialize)]
        struct Service {
            connected: bool,
            user: Option<String>,
            /// Why not, when `connected` is false. Prose, for a human or a
            /// caller deciding what to do about it.
            detail: Option<String>,
        }
        /// What the configured parse backend says about itself. There is no
        /// process to report on any more — parsing happens in this one — so
        /// this is the seam's own `Health`, plus why it could not be reached
        /// when it could not be. `parser_version` stays visible because it is
        /// the version handshake: a backend stamping a different number writes
        /// artifacts this binary cannot read as its own.
        #[derive(Serialize)]
        struct ParserStatus {
            backend: String,
            ready: bool,
            parser_version: Option<u32>,
            detail: Option<String>,
        }
        #[derive(Serialize)]
        struct Counts {
            total: usize,
            current: usize,
            synced: usize,
        }
        #[derive(Serialize)]
        struct FileCounts {
            total: i64,
            parsed: i64,
        }
        fn describe(s: &Service) -> String {
            match (&s.user, &s.detail) {
                (Some(name), _) => format!("{} as {name}", paint("connected", GREEN)),
                (None, Some(why)) => paint(why, YELLOW),
                (None, None) => paint("unknown", DIM),
            }
        }

        let canvas = app_lib::canvas::Canvas::open(&self.data_dir);
        let canvas_status = match canvas.whoami() {
            Ok(name) => Service { connected: true, user: Some(name), detail: None },
            Err(_) if !canvas.has_session() => Service {
                connected: false,
                user: None,
                detail: Some("signed out — run `oculus auth login`".to_string()),
            },
            Err(e) => Service { connected: false, user: None, detail: Some(e) },
        };

        let ed = app_lib::ed::Ed::open(&self.data_dir);
        let ed_status = if !ed.has_session() {
            Service {
                connected: false,
                user: None,
                detail: Some("not connected — connects automatically on the next sync".to_string()),
            }
        } else {
            match ed.whoami() {
                Ok(name) => Service { connected: true, user: Some(name), detail: None },
                Err(e) => Service { connected: false, user: None, detail: Some(e) },
            }
        };

        // Constructing the backend reads the settings row and the keychain, and
        // `preflight` asks it about itself. On the cloud engine that is purely
        // local and costs no quota. On the local engine `health()` probes the
        // server over loopback — still no cloud call and still no quota, but a
        // connect, which is why that probe carries a short timeout of its own.
        let parser = match app_lib::parse::backend() {
            Ok(backend) => match app_lib::parse::preflight(backend.as_ref()) {
                Ok(health) => ParserStatus {
                    backend: health.backend,
                    ready: health.ready,
                    parser_version: Some(health.parser_version),
                    detail: None,
                },
                Err(e) => ParserStatus {
                    backend: backend.health().backend,
                    ready: false,
                    parser_version: Some(backend.health().parser_version),
                    detail: Some(e.to_string()),
                },
            },
            Err(e) => ParserStatus {
                backend: app_lib::parse::parse_config().engine.as_str().to_string(),
                ready: false,
                parser_version: None,
                detail: Some(e.to_string()),
            },
        };

        let pool = self.db();
        let (subjects, files) = match &pool {
            Some(pool) => self.rt.block_on(async {
                let rows = store::subjects(pool).await.unwrap_or_default();
                let counts = Counts {
                    total: rows.len(),
                    current: rows.iter().filter(|s| s.is_current).count(),
                    synced: rows.iter().filter(|s| s.last_synced_at.is_some()).count(),
                };
                let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files")
                    .fetch_one(pool)
                    .await
                    .unwrap_or(0);
                let parsed: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM files WHERE parse_status = 'quality'",
                )
                .fetch_one(pool)
                .await
                .unwrap_or(0);
                (Some(counts), Some(FileCounts { total, parsed }))
            }),
            None => (None, None),
        };

        // Whether `oculus search` can answer at all, which is the one thing a
        // caller most needs to know before trying it.
        let index = pool.as_ref().and_then(|_| {
            self.rt
                .block_on(app_lib::retrieval::stats(&app_lib::paths::db_path(&self.data_dir)))
                .ok()
        });

        if self.json {
            #[derive(Serialize)]
            struct Report<'a> {
                data_dir: String,
                canvas: &'a Service,
                ed: &'a Service,
                parser: &'a ParserStatus,
                subjects: &'a Option<Counts>,
                files: &'a Option<FileCounts>,
                index: &'a Option<app_lib::retrieval::IndexStats>,
            }
            return self.emit(&Report {
                data_dir: self.data_dir.display().to_string(),
                canvas: &canvas_status,
                ed: &ed_status,
                parser: &parser,
                subjects: &subjects,
                files: &files,
                index: &index,
            });
        }

        println!("{}  {}", paint("library", DIM), self.data_dir.display());
        println!("{}   {}", paint("canvas", DIM), describe(&canvas_status));
        println!("{}       {}", paint("ed", DIM), describe(&ed_status));
        println!(
            "{}   {}",
            paint("parser", DIM),
            match (&parser.detail, parser.parser_version) {
                (None, Some(v)) => paint(&format!("{} (v{v})", parser.backend), GREEN),
                (Some(why), _) => paint(why, YELLOW),
                (None, None) => paint("unknown", DIM),
            }
        );
        if let Some(c) = &subjects {
            println!(
                "{} {} ({} current, {} synced)",
                paint("subjects", DIM),
                c.total,
                c.current,
                c.synced
            );
        }
        if let Some(f) = &files {
            println!("{}    {} ({} parsed)", paint("files", DIM), f.total, f.parsed);
        }
        if let Some(i) = &index {
            println!(
                "{}    {} page(s) across {} file(s){}",
                paint("index", DIM),
                i.pages_embedded,
                i.files_embedded,
                match &i.model {
                    Some(m) => paint(&format!("  {m}"), DIM),
                    None => String::new(),
                }
            );
            // Stored but not searchable. Printed as its own line rather than
            // folded into the count above, because a caller deciding whether
            // `oculus search` can answer needs the searchable number plain.
            if i.pages_stale > 0 {
                println!(
                    "{}    {} page(s) need re-embedding{}",
                    paint("stale", DIM),
                    i.pages_stale,
                    match i.stale_models.is_empty() {
                        true => String::new(),
                        false => paint(&format!("  from {}", i.stale_models.join(", ")), DIM),
                    }
                );
            }
        }
        Ok(())
    }

    fn auth_status(&self) -> Result<(), String> {
        let canvas = app_lib::canvas::Canvas::open(&self.data_dir);
        match canvas.whoami() {
            Ok(name) => {
                println!("{} as {name}", paint("connected", GREEN));
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Walk the user through storing credentials. The instructions matter as
    /// much as the prompts: the setup key is the one thing that cannot be
    /// recovered later, and the second QR scan is what stops this from being
    /// a one-way door out of your own account.
    fn auth_setup(&self) -> Result<(), String> {
        println!("{}", paint("Automated sign-in setup", DIM));
        println!();
        println!("This needs a TOTP factor enrolled and its setup key. A TOTP code is a");
        println!("one-way function of a secret seed, so it cannot be worked out from other");
        println!("codes — the seed is shown only at enrolment. If you never copied it:");
        println!();
        println!("  1. Open https://sso.unimelb.edu.au/enduser/settings");
        println!("  2. Google Authenticator → remove it, then set it up again");
        println!("  3. On the QR page click \"Can't scan?\" to reveal the setup key");
        println!("  4. {} scan the QR with your phone as well, so you keep", paint("Also", YELLOW));
        println!("     a working authenticator if this Mac is ever unavailable.");
        println!();

        let username = read_line("Username (e.g. chanwangsat): ")?;
        let password = read_secret("Password: ")?;
        let secret = read_secret("Authenticator setup key: ")?;

        app_lib::okta::store_credentials(&username, &password, &secret)?;
        let code = app_lib::okta::totp_now(&secret)?;

        println!();
        println!("{} to the macOS keychain", paint("saved", GREEN));
        println!(
            "This Mac's code right now is {} — confirm it matches your phone",
            paint(&code, GREEN)
        );
        println!("before relying on this, then run `oculus auth auto`.");
        Ok(())
    }

    /// Run the headless sign-in and report precisely why it failed, since the
    /// first real run against the live Okta policy is also the diagnosis.
    fn auth_auto(&self) -> Result<(), String> {
        match app_lib::okta::sign_in(&self.data_dir) {
            Ok(_) => {
                let name = app_lib::canvas::Canvas::open(&self.data_dir).whoami()?;
                // The app treats a missing flag as "never signed in" and will
                // not so much as probe the cookie we just wrote.
                app_lib::paths::mark_authenticated(&self.data_dir);
                println!("{} as {name}", paint("connected", GREEN));
                Ok(())
            }
            Err(e @ app_lib::okta::LoginError::BadPassword(_)) => {
                app_lib::okta::clear_password().ok();
                Err(format!("{e}\nThe stored password has been discarded — run `oculus auth setup` again."))
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// One keep-alive cycle, for the LaunchAgent.
    ///
    /// Every outcome is a log line and `Ok(())`. Nothing here is an error
    /// launchd can act on: it has no console, and a non-zero exit would just be
    /// recorded as a crashed job.
    fn auth_tick(&self) -> Result<(), String> {
        use app_lib::canvas::SessionProbe;

        let log = |m: &str| app_lib::paths::append_keepalive_log(&self.data_dir, m);

        // The probe *is* the keep-alive: Canvas extends the session on use and
        // the client folds any rotated cookie back to disk on the way through.
        match app_lib::canvas::Canvas::open(&self.data_dir).probe() {
            SessionProbe::Valid(name) => {
                app_lib::paths::mark_authenticated(&self.data_dir);
                log(&format!("session extended ({name})"));
                return Ok(());
            }
            // Nothing is wrong with the session we hold — the network is down.
            // Re-authenticating here would spend an Okta attempt to learn that.
            SessionProbe::Unreachable(why) => {
                log(&format!("skipped — {why}"));
                return Ok(());
            }
            SessionProbe::Rejected(why) => log(&format!("session rejected — {why}")),
        }

        match app_lib::okta::sign_in(&self.data_dir) {
            Ok(_) => match app_lib::canvas::Canvas::open(&self.data_dir).whoami() {
                Ok(name) => {
                    app_lib::paths::mark_authenticated(&self.data_dir);
                    log(&format!("session rebuilt ({name})"));
                }
                // Signed in, but the new cookie did not verify. Leave the flag
                // alone rather than assert a session we could not confirm.
                Err(e) => log(&format!("signed in but could not verify: {e}")),
            },
            Err(e @ app_lib::okta::LoginError::BadPassword(_)) => {
                // Drop it now: replaying a wrong password every six hours,
                // unattended, is how the account gets locked.
                app_lib::okta::clear_password().ok();
                log(&format!("{e} — stored password discarded, run `oculus auth setup`"));
            }
            Err(e) => log(&format!("automated sign-in failed: {e}")),
        }
        Ok(())
    }

    fn auth_forget(&self) -> Result<(), String> {
        app_lib::okta::clear_credentials()?;
        println!("{}", paint("forgotten", YELLOW));
        Ok(())
    }

    fn auth_ed(&self, token: Option<&str>) -> Result<(), String> {
        match token {
            Some(t) => {
                let name = app_lib::ed::Ed::set_token(&self.data_dir, t)?;
                println!("{} as {name}", paint("connected", GREEN));
                Ok(())
            }
            None => {
                let ed = app_lib::ed::Ed::open(&self.data_dir);
                let name = ed.whoami()?;
                println!("{} as {name}", paint("connected", GREEN));
                Ok(())
            }
        }
    }

    // ── auth ─────────────────────────────────────────────────────────────────

    /// Canvas sign-in is SAML through the university IdP, which needs a real
    /// browser. The app already has one, so login means: start the app, then
    /// watch for the cookie it saves. Nothing here can shortcut that.
    fn login(&self) -> Result<(), String> {
        let canvas = app_lib::canvas::Canvas::open(&self.data_dir);
        if let Ok(name) = canvas.whoami() {
            println!("{} as {name}", paint("already connected", GREEN));
            return Ok(());
        }

        let cookie = app_lib::paths::cookie_path(&self.data_dir);
        let before = std::fs::metadata(&cookie).and_then(|m| m.modified()).ok();

        println!("opening Oculus for Canvas sign-in…");
        #[cfg(target_os = "macos")]
        let launched = std::process::Command::new("open").args(["-a", "Oculus"]).status().is_ok_and(|s| s.success());
        #[cfg(not(target_os = "macos"))]
        let launched = false;

        if !launched {
            println!("could not launch the app — start Oculus yourself and sign in.");
        }
        println!("waiting for the session (Ctrl-C to give up)…");

        // Poll rather than watch: the app writes the cookie from a background
        // thread two seconds after the SSO redirect lands, and a changed file
        // is the only signal that crosses process boundaries.
        for _ in 0..600 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let now = std::fs::metadata(&cookie).and_then(|m| m.modified()).ok();
            if now.is_none() || now == before {
                continue;
            }
            let canvas = app_lib::canvas::Canvas::open(&self.data_dir);
            if let Ok(name) = canvas.whoami() {
                println!("{} as {name}", paint("connected", GREEN));
                return Ok(());
            }
        }
        Err("timed out waiting for sign-in".to_string())
    }

    fn logout(&self) -> Result<(), String> {
        let cookie = app_lib::paths::cookie_path(&self.data_dir);
        let session_dir = self.data_dir.join("canvas-session");
        let had = cookie.exists() || session_dir.exists();

        std::fs::remove_file(&cookie).ok();
        // Removes the SSO profile too, so the next login is a fresh one.
        std::fs::remove_dir_all(&session_dir).ok();

        println!("{}", if had { paint("signed out", GREEN) } else { paint("no session to clear", DIM) });
        Ok(())
    }

    // ── list ─────────────────────────────────────────────────────────────────

    fn list(&self, args: ListArgs) -> Result<(), String> {
        if args.lectures {
            return self.list_lectures(&args.codes);
        }
        self.list_subjects(args.refresh)
    }

    fn list_subjects(&self, refresh: bool) -> Result<(), String> {
        let pool = self.db();

        // Fetch live when asked, and also when the database has nothing to show
        // — a first run should not print an empty table.
        let mut rows = match &pool {
            Some(p) => self.rt.block_on(store::subjects(p))?,
            None => Vec::new(),
        };
        if refresh || rows.is_empty() {
            let engine = self.engine(false);
            if !engine.canvas.has_session() {
                return Err("not connected — run `oculus auth login`".to_string());
            }
            let courses = engine.list_courses()?;
            if let Some(p) = &pool {
                self.rt.block_on(store::upsert_subjects(p, &courses))?;
                rows = self.rt.block_on(store::subjects(p))?;
            } else {
                for c in &courses {
                    println!(
                        "{:<24} {:<12} {}",
                        c.code,
                        c.term.clone().unwrap_or_default(),
                        c.name
                    );
                }
                return Ok(());
            }
        }

        if self.json {
            #[derive(Serialize)]
            struct Subject<'a> {
                id: i64,
                code: &'a str,
                name: &'a str,
                term: Option<&'a str>,
                current: bool,
                selected: bool,
                last_synced_at: Option<&'a str>,
            }
            let out: Vec<Subject> = rows
                .iter()
                .map(|s| Subject {
                    id: s.id,
                    code: &s.code,
                    name: &s.name,
                    term: s.term_name.as_deref(),
                    current: s.is_current,
                    selected: s.selected,
                    last_synced_at: s.last_synced_at.as_deref(),
                })
                .collect();
            return self.emit(&out);
        }

        if rows.is_empty() {
            println!("{}", paint("no subjects", DIM));
            return Ok(());
        }
        for s in &rows {
            let mark = if s.is_current { paint("●", GREEN) } else { paint("○", DIM) };
            let synced = s
                .last_synced_at
                .clone()
                .map(|t| t.chars().take(10).collect::<String>())
                .unwrap_or_else(|| "never".into());
            println!(
                "{mark} {:<24} {:<14} {} {}",
                s.code,
                s.term_name.clone().unwrap_or_default(),
                paint(&format!("{synced:<11}"), DIM),
                s.name
            );
        }
        Ok(())
    }

    fn list_lectures(&self, codes: &[String]) -> Result<(), String> {
        let pool = self.db().ok_or("lectures live in the database")?;
        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let wanted = filter_subjects(&subjects, codes, true)?;

        if self.json {
            #[derive(Serialize)]
            struct Lecture<'a> {
                id: &'a str,
                subject: &'a str,
                title: &'a str,
                date: &'a str,
                duration_seconds: i64,
                has_video: bool,
                has_transcript: bool,
            }
            let mut out: Vec<Lecture> = Vec::new();
            let rows: Vec<(String, Vec<store::LectureRow>)> = wanted
                .iter()
                .map(|s| Ok((s.code.clone(), self.rt.block_on(store::lectures(&pool, s.id))?)))
                .collect::<Result<_, String>>()?;
            for (code, lectures) in &rows {
                out.extend(lectures.iter().map(|l| Lecture {
                    id: &l.id,
                    subject: code,
                    title: &l.title,
                    date: &l.date,
                    duration_seconds: l.duration_seconds,
                    has_video: l.has_video,
                    has_transcript: l.has_transcript,
                }));
            }
            return self.emit(&out);
        }

        for s in &wanted {
            let rows = self.rt.block_on(store::lectures(&pool, s.id))?;
            println!("{}  {}", paint(&s.code, BOLD), paint(&format!("{} lectures", rows.len()), DIM));
            for l in &rows {
                let mins = l.duration_seconds / 60;
                let marks = format!(
                    "{}{}",
                    if l.has_video { "video" } else { "     " },
                    if l.has_transcript { " transcript" } else { "" }
                );
                // The id leads, dimmed, the way `project list` prints an id
                // — it is the only handle `oculus lecture` takes, and the
                // first eight characters of a UUID are enough for the prefix
                // match that command already does.
                println!(
                    "  {} {:<11} {mins:>4}m  {} {}",
                    paint(&l.id.chars().take(8).collect::<String>(), DIM),
                    l.date.chars().take(10).collect::<String>(),
                    paint(&format!("{marks:<22}"), DIM),
                    l.title
                );
            }
        }
        Ok(())
    }

    // ── run ──────────────────────────────────────────────────────────────────

    fn run(&self, args: RunArgs) -> Result<(), String> {
        if args.lectures {
            return self.run_lectures(&args);
        }
        self.run_subjects(&args)
    }

    /// Sync the Echo360 lecture list, and optionally pull the media. Each
    /// subject gets its own LTI launch — the Echo360 session is per course.
    fn run_lectures(&self, args: &RunArgs) -> Result<(), String> {
        let pool = self.db().ok_or("lectures are stored in the database")?;
        let cookie = std::fs::read_to_string(app_lib::paths::cookie_path(&self.data_dir))
            .map_err(|_| "not connected — run `oculus auth login`".to_string())?;

        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let wanted = filter_subjects(&subjects, &args.codes, !args.all)?;
        if wanted.is_empty() {
            return Err("no subjects to sync — pass subject codes or --all".to_string());
        }

        let ffmpeg = (args.videos)
            .then(|| app_lib::echo360::find_ffmpeg(None))
            .flatten();
        if args.videos && ffmpeg.is_none() {
            return Err("ffmpeg not found — run `bun run ffmpeg` in app/".to_string());
        }

        for s in &wanted {
            println!("{}", paint(&s.code, BOLD));
            let session = match app_lib::echo360::connect(cookie.trim(), s.id) {
                Ok(sess) => sess,
                Err(e) => {
                    eprintln!("  {} {e}", paint("skip", YELLOW));
                    continue;
                }
            };
            let lectures = app_lib::echo360::syllabus(&session)?;
            self.rt.block_on(store::upsert_lectures(&pool, s.id, &lectures))?;
            println!("  {} lecture(s)", lectures.len());

            for l in &lectures {
                let dir = app_lib::echo360::lecture_dir(&self.data_dir, &l.id);
                let date = l.date.chars().take(10).collect::<String>();

                if args.transcripts {
                    let path = dir.join("transcript.vtt");
                    if !path.exists() {
                        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                        match app_lib::echo360::transcript(&session, &l.lesson_id, &l.id) {
                            Ok(vtt) => {
                                std::fs::write(&path, vtt).map_err(|e| e.to_string())?;
                                self.rt.block_on(store::set_lecture_path(
                                    &pool, &l.id, "transcript_path", &path.to_string_lossy(),
                                ))?;
                                println!("  {}  {date}  {}", paint("transcript", DIM), l.title);
                            }
                            Err(e) => eprintln!("  {} {}: {e}", paint("warn", YELLOW), l.title),
                        }
                    }
                }

                if let Some(ffmpeg) = &ffmpeg {
                    // Source 1 only — the Presenter screen is what an archive is
                    // for, and pulling every room camera as well would roughly
                    // double a semester on disk. The app downloads the camera
                    // on demand, per lecture.
                    let final_ = app_lib::echo360::source_path(&dir, 1);
                    if final_.exists() {
                        continue;
                    }
                    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                    let raw = app_lib::echo360::partial_path(&dir, 1);
                    print!("  {}       {date}  {} ", paint("video", DIM), l.title);
                    let _ = std::io::stdout().flush();

                    let outcome = app_lib::echo360::download_url(&session, &l.id, &l.lesson_id, 1)
                        // Ctrl-C is the CLI's cancel; nothing here can ask to stop.
                        .and_then(|url| {
                            app_lib::echo360::stream_to_file(&url, &raw, &|_| {}, &|| false)
                        })
                        .and_then(|bytes| {
                            if app_lib::echo360::trim_video(ffmpeg, &raw, &final_) {
                                Ok(bytes)
                            } else {
                                Err("ffmpeg trim failed".to_string())
                            }
                        });
                    std::fs::remove_file(&raw).ok();

                    match outcome {
                        Ok(bytes) => {
                            println!("{}", paint(&human_bytes(bytes), DIM));
                            self.rt.block_on(store::set_lecture_path(
                                &pool, &l.id, "video_path", &final_.to_string_lossy(),
                            ))?;
                        }
                        Err(e) => {
                            std::fs::remove_file(&final_).ok();
                            println!("{}", paint(&e, RED));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn run_subjects(&self, args: &RunArgs) -> Result<(), String> {
        // No in-scrape parse triggering here: `index_pdfs` below is the CLI's
        // parse tier and it walks the same files serially. With both on, every
        // PDF was submitted twice — once by the engine's queue as it landed,
        // once by `index_pdfs` — and the two could be mid-parse on
        // the same `.md`/`.pages.json` at the same time. The app keeps the
        // in-scrape trigger; it has a UI that wants progress as files arrive.
        let engine = self.engine(false);
        if !engine.canvas.has_session() {
            return Err("not connected — run `oculus auth login`".to_string());
        }
        let who = engine.canvas.whoami()?;

        let pool = self.db();

        // Refresh the course list first: a subject added this week would
        // otherwise be invisible to a code filter.
        let courses = engine.list_courses()?;
        if let Some(p) = &pool {
            self.rt.block_on(store::upsert_subjects(p, &courses))?;
        }

        let targets: Vec<sync::Subject> = match (&pool, args.codes.is_empty()) {
            // No filter and a database: honour the selection made in the app.
            (Some(p), true) => {
                let rows = self.rt.block_on(store::subjects(p))?;
                rows.iter()
                    .filter(|s| s.selected && (args.all || s.is_current))
                    .map(|s| sync::Subject { id: s.id, code: s.code.clone() })
                    .collect()
            }
            _ => courses
                .iter()
                .filter(|c| {
                    if args.codes.is_empty() {
                        args.all || c.is_current
                    } else {
                        args.codes.iter().any(|w| matches_code(&c.code, w))
                    }
                })
                .map(|c| sync::Subject { id: c.id, code: c.code.clone() })
                .collect(),
        };

        if targets.is_empty() {
            return Err(if args.codes.is_empty() {
                "no current subjects selected — pass subject codes or --all".to_string()
            } else {
                format!("no subject matched {}", args.codes.join(", "))
            });
        }

        println!("{} as {who}", paint("canvas", DIM));
        println!("syncing {} subject(s): {}", targets.len(), targets.iter().map(|s| s.code.as_str()).collect::<Vec<_>>().join(", "));
        println!();

        let target_codes: Vec<String> = targets.iter().map(|s| s.code.clone()).collect();
        let run_id = pool
            .as_ref()
            .and_then(|p| self.rt.block_on(store::start_run(p, &target_codes)).ok());

        // The engine reports through a plain trait; here that means printing a
        // line per artifact and, when there is a database, upserting the same
        // rows the app's event listener would have written.
        let reporter = TermReporter::new();
        let sink = reporter.sink();
        // The engine's fire-and-forget parse is for the app, which wants a
        // progress bar moving while it downloads. Here the index phase below
        // owns the parsing, one PDF at a time, so the two never overlap.
        let engine = Engine::new(&self.data_dir, Box::new(reporter)).with_pdf_parsing(false);

        let started = std::time::Instant::now();
        let done = engine.scrape(&targets);

        let written = sink.lock().unwrap().clone();
        if let Some(p) = &pool {
            self.rt.block_on(async {
                for f in &written {
                    if let Err(e) = store::upsert_file(p, f.subject_id, &f.relative_path, f.size_bytes, &f.category, f.canvas_id, f.source_url.as_deref(), f.action != "unchanged").await {
                        eprintln!("{} {e}", paint("db:", YELLOW));
                    }
                }
                if let Some(id) = run_id {
                    store::finish_run(p, id, "completed", done, None).await.ok();
                    store::add_log(p, "info", &format!("CLI synced {done} subject(s)"), Some(id)).await.ok();
                }
            });
        }

        println!();
        println!(
            "{} {done} subject(s), {} artifact(s) in {:.1}s",
            paint("done", GREEN),
            written.len(),
            started.elapsed().as_secs_f64()
        );

        // Class times and due dates. Not part of the scrape: these come from
        // Canvas's calendar API and live only in the database, so there is no
        // artifact to report and nothing on disk to skip.
        if let Some(p) = &pool {
            let mut total = 0usize;
            for t in &targets {
                match app_lib::calendar::fetch(&engine.canvas, t.id) {
                    Ok(events) => {
                        total += events.len();
                        if let Err(e) =
                            self.rt.block_on(store::replace_calendar_events(p, t.id, &events))
                        {
                            eprintln!("{} {e}", paint("calendar:", YELLOW));
                        }
                    }
                    Err(e) => eprintln!("{} {}: {e}", paint("calendar:", YELLOW), t.code),
                }
            }
            println!("{} {total} calendar event(s)", paint("calendar", DIM));
        }

        if args.no_parse {
            return Ok(());
        }
        let Some(p) = &pool else {
            println!("{}", paint("no database — skipping parse and index", YELLOW));
            return Ok(());
        };

        let pdfs: Vec<(i64, String)> = written
            .iter()
            .filter(|f| f.relative_path.to_lowercase().ends_with(".pdf"))
            .map(|f| (f.subject_id, f.relative_path.clone()))
            .collect();
        self.index_pdfs(p, &pdfs, !args.no_embed)
    }

    // ── Parse + embed ────────────────────────────────────────────────────────

    /// Parse each PDF and fold it into the retrieval index.
    ///
    /// Serial by design: one at a time is the only way the log stays readable,
    /// and both halves are idempotent, so re-running this over an
    /// already-indexed library is cheap.
    ///
    /// **Each file now blocks for its whole cloud round trip — minutes, not
    /// the seconds the sidecar's fast pass answered in.** That is the point
    /// rather than a cost: this call used to return as soon as *some* markdown
    /// existed and leave the real parse running in another process, so the
    /// text only appeared on some later `oculus index`. It finishes the parse
    /// now. The per-page callback below is what keeps the terminal from
    /// looking hung while it does.
    ///
    /// **The embed half is slower still, and on the free programme it is the
    /// governor.** Voyage allows 3 requests and 10K tokens a minute to an
    /// account with no payment method, which at ~3,571 tokens for a 200-DPI
    /// page is about 2.8 pages a minute however well they are batched — hours
    /// for a large deck, against a couple of minutes on tier 1. No timeout is
    /// imposed from here: the client paces itself against the tier it
    /// detected, a 429 is routine rather than a failure, and a deadline
    /// invented at this level could only abandon work that was still
    /// progressing. What this level owes the user instead is an honest
    /// counter, which is the second callback below.
    fn index_pdfs(&self, pool: &SqlitePool, pdfs: &[(i64, String)], embed: bool) -> Result<(), String> {
        if pdfs.is_empty() {
            return Ok(());
        }
        println!();
        println!(
            "{} {} PDF(s)",
            paint(if embed { "indexing" } else { "parsing" }, BOLD),
            pdfs.len()
        );

        let mut pages_total = 0usize;
        let mut failed = 0usize;

        let mut missing = 0usize;

        for (subject_id, rel) in pdfs {
            // For Office documents the parse/embed target is the derived
            // sibling PDF, not the library file itself.
            let Some(pdf_rel) = app_lib::paths::doc_pdf_rel(rel) else {
                continue;
            };
            // Rows can outlive their file — a course renamed, a library moved.
            // Those are not failures worth shouting about, just nothing to do.
            if !self.data_dir.join(&pdf_rel).is_file() {
                missing += 1;
                continue;
            }
            let name = rel.rsplit('/').next().unwrap_or(rel);
            let label = format!("  {:<52} ", truncate(name, 52));
            print!("{label}");
            let _ = std::io::stdout().flush();

            // Rewrite the one line in place as pages arrive. `\x1b[K` clears
            // whatever the longer previous count left behind.
            let outcome =
                app_lib::sync::parse_pdf_reporting(&self.data_dir, rel, *subject_id, &|p| {
                    let seen = match p.total_pages {
                        0 => format!("{} pages", p.pages_done),
                        total => format!("{}/{total} pages", p.pages_done),
                    };
                    print!("\r{label}{}\x1b[K", paint(&seen, DIM));
                    let _ = std::io::stdout().flush();
                });
            print!("\r{label}\x1b[K");
            let parsed = match outcome {
                Ok(summary) => summary.to_string(),
                Err(e) => {
                    println!("{}", paint(&e.to_string(), RED));
                    failed += 1;
                    continue;
                }
            };
            print!("{}", paint(&parsed, DIM));
            let _ = std::io::stdout().flush();

            if !embed {
                println!();
                continue;
            }

            // Everything printed so far on this line, so the embed's own
            // counter can rewrite in place after it. Embedding is the slower
            // half now — on an account with no payment method Voyage allows
            // ~2.8 pages a minute, so a 200-page deck is over an hour and a
            // line that never changes is indistinguishable from a hang.
            let stem = format!("{label}{}  ", paint(&parsed, DIM));
            let outcome = self.rt.block_on(async {
                let Some(file_id) = store::file_id(pool, *subject_id, rel).await? else {
                    return Err("not in the database".to_string());
                };
                let abs = self.data_dir.join(&pdf_rel).to_string_lossy().to_string();
                let line = stem.clone();
                app_lib::retrieval::ingest_reporting(
                    &app_lib::paths::db_path(&self.data_dir),
                    file_id,
                    abs,
                    false,
                    std::sync::Arc::new(move |p: app_lib::embed::Progress| {
                        let seen = match p.total_pages {
                            0 => format!("embedding {} pages", p.pages_done),
                            total => format!("embedding {}/{total} pages", p.pages_done),
                        };
                        print!("\r{line}{}\x1b[K", paint(&seen, DIM));
                        let _ = std::io::stdout().flush();
                    }),
                )
                .await
                // The discriminants are for the app's pipeline row; a terminal
                // has one line and prints the sentence.
                .map_err(|e| e.message)
            });
            print!("\r{stem}\x1b[K");

            match outcome {
                Ok(s) => {
                    pages_total += s.pages_embedded;
                    println!(
                        "{}",
                        paint(&format!("{} pages, {} with text", s.pages_embedded, s.pages_with_markdown), DIM)
                    );
                }
                Err(e) => {
                    println!("{}", paint(&e, RED));
                    failed += 1;
                }
            }
        }

        self.rt
            .block_on(store::reconcile_parse_status(pool, &self.data_dir))
            .ok();

        println!();
        if embed {
            println!("{} {pages_total} page(s) indexed", paint("done", GREEN));
        }
        if failed > 0 {
            println!("{} {failed} PDF(s) failed", paint("warning", YELLOW));
        }
        if missing > 0 {
            println!(
                "{} {missing} database row(s) point at files no longer on disk",
                paint("warning", YELLOW)
            );
        }
        Ok(())
    }

    /// Re-run parse and embed over PDFs already on record — the cheap way to
    /// pick up quality-parse text without re-downloading anything.
    fn index(&self, args: &IndexArgs) -> Result<(), String> {
        let pool = self.db().ok_or("the index lives in the database")?;
        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let wanted = filter_subjects(&subjects, &args.codes, false)?;
        let ids: Vec<i64> = wanted.iter().map(|s| s.id).collect();

        let pdfs = self.rt.block_on(store::pdf_files(&pool, &ids))?;
        if pdfs.is_empty() {
            println!("{}", paint("no PDFs on record — run a sync first", DIM));
            return Ok(());
        }
        self.index_pdfs(&pool, &pdfs, true)
    }

    // ── search ───────────────────────────────────────────────────────────────

    /// Rank pages by meaning.
    ///
    /// The failure modes below are the point of this function. A caller handed
    /// an empty result concludes the library has no answer and stops; a caller
    /// told *why* it is empty tries the other door. So an empty index names
    /// `index`, and an index full of vectors from a retired model says so in
    /// those words rather than reporting nothing indexed — both are errors
    /// rather than a silent zero-hit success.
    fn search(&self, args: &SearchArgs) -> Result<(), String> {
        let pool = self.db().ok_or("the retrieval index lives in the database")?;
        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let ids: Vec<i64> = match &args.subject {
            // A bare code can match the same subject in two terms, and both
            // sets of pages are legitimately in scope.
            Some(code) => filter_subjects(&subjects, std::slice::from_ref(code), false)?
                .iter()
                .map(|s| s.id)
                .collect(),
            None => Vec::new(),
        };
        let codes: HashMap<i64, String> =
            subjects.iter().map(|s| (s.id, s.code.clone())).collect();

        let db_file = app_lib::paths::db_path(&self.data_dir);
        let stats = self.rt.block_on(app_lib::retrieval::stats(&db_file))?;
        if stats.pages_embedded == 0 {
            // "Nothing searchable" and "nothing stored" look identical from a
            // count of zero and call for different actions, so they are told
            // apart here. A library embedded by a retired model is the state
            // every install is in immediately after the move to Voyage.
            if stats.pages_stale > 0 {
                return Err(format!(
                    "{} page(s) are stored, but they were embedded by {} and cannot be\n       \
                     compared against a query from {}. Re-run `oculus index` to rebuild them.",
                    stats.pages_stale,
                    if stats.stale_models.is_empty() {
                        "a retired model".to_string()
                    } else {
                        stats.stale_models.join(", ")
                    },
                    stats.model.as_deref().unwrap_or("the current model"),
                ));
            }
            return Err(
                "nothing is indexed yet, so there is nothing to rank.\n       \
                 Run `oculus index` over PDFs already on record, or `oculus run -s` to scrape."
                    .to_string(),
            );
        }

        let hits = self.rt.block_on(app_lib::retrieval::search_in(
            &db_file,
            args.query.clone(),
            args.limit,
            &ids,
        ))?;

        #[derive(Serialize)]
        struct Hit {
            score: f32,
            subject: String,
            path: String,
            filename: String,
            page_no: i64,
            markdown: String,
        }
        let hits: Vec<Hit> = hits
            .into_iter()
            .map(|h| Hit {
                score: h.score,
                subject: codes.get(&h.subject_id).cloned().unwrap_or_default(),
                path: h.relative_path,
                filename: h.filename,
                page_no: h.page_no,
                markdown: h.markdown,
            })
            .collect();

        if self.json {
            return self.emit(&hits);
        }
        if hits.is_empty() {
            println!("{}", paint("no matching pages", DIM));
            return Ok(());
        }
        for h in &hits {
            // Subject plus in-course path, not the bare filename: `12.pdf` is
            // a name two courses can both have, and a library can hold the
            // same deck under two folders.
            let short = h.path.splitn(3, '/').nth(2).unwrap_or(&h.path);
            println!(
                "{} {} {} {}",
                paint(&format!("{:.3}", h.score), BOLD),
                paint(&format!("{:<20}", truncate(&h.subject, 20)), DIM),
                truncate(short, 44),
                paint(&format!("p{}", h.page_no), DIM)
            );
            if args.full {
                println!("{}\n", h.markdown);
            } else {
                println!("      {}", snippet(&h.markdown, 96));
            }
        }
        if !args.full {
            if let Some(top) = hits.first() {
                println!();
                println!(
                    "{} oculus read {} --pages {}",
                    paint("read one:", DIM),
                    shell_quote(&top.path),
                    top.page_no
                );
            }
        }
        Ok(())
    }

    // ── grep ─────────────────────────────────────────────────────────────────

    /// Pattern search across both halves of the library.
    ///
    /// Scans in path order and stops at the limit, so PDF text and markdown
    /// interleave the way a caller expects instead of the database half
    /// crowding out the disk half.
    fn grep(&self, args: &GrepArgs) -> Result<(), String> {
        let pool = self.db().ok_or("the library index lives in the database")?;
        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let ids: Vec<i64> = filter_subjects(&subjects, &args.subject, false)?
            .iter()
            .map(|s| s.id)
            .collect();
        let files = filter_categories(self.library_files(&pool, &ids)?, &args.category)?;
        let pages = self.page_text(&pool, &ids)?;
        let re = build_regex(&args.pattern, args.fixed, args.case_sensitive)?;
        let limit = args.limit.max(1);

        #[derive(Serialize)]
        struct Match {
            subject: String,
            path: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            page_no: Option<i64>,
            #[serde(skip_serializing_if = "Option::is_none")]
            line_no: Option<usize>,
            line: String,
        }

        let mut hits: Vec<Match> = Vec::new();
        let mut truncated = false;

        'files: for f in &files {
            // Parsed documents: the text is in the database, never on disk.
            if let Some(pages) = pages.get(&f.id) {
                for (page_no, markdown) in pages {
                    for line in markdown.lines() {
                        if !re.is_match(line) {
                            continue;
                        }
                        if hits.len() >= limit {
                            truncated = true;
                            break 'files;
                        }
                        hits.push(Match {
                            subject: f.code.clone(),
                            path: f.relative_path.clone(),
                            page_no: Some(*page_no),
                            line_no: None,
                            line: line.trim().to_string(),
                        });
                    }
                }
                continue;
            }

            // Everything else worth scanning is text on disk: Canvas pages,
            // announcements, assignments, Ed threads.
            if !is_text_file(&f.relative_path) {
                continue;
            }
            let Ok(body) = std::fs::read_to_string(self.data_dir.join(&f.relative_path)) else {
                continue;
            };
            for (i, line) in body.lines().enumerate() {
                if !re.is_match(line) {
                    continue;
                }
                if hits.len() >= limit {
                    truncated = true;
                    break 'files;
                }
                hits.push(Match {
                    subject: f.code.clone(),
                    path: f.relative_path.clone(),
                    page_no: None,
                    line_no: Some(i + 1),
                    line: line.trim().to_string(),
                });
            }
        }

        if args.files_with_matches {
            let mut seen: Vec<&str> = Vec::new();
            for h in &hits {
                if !seen.contains(&h.path.as_str()) {
                    seen.push(&h.path);
                }
            }
            if self.json {
                return self.emit(&seen);
            }
            for p in seen {
                println!("{p}");
            }
            return Ok(());
        }

        if self.json {
            return self.emit(&hits);
        }
        if hits.is_empty() {
            println!("{}", paint("no matches", DIM));
            return Ok(());
        }
        for h in &hits {
            let at = match (h.page_no, h.line_no) {
                (Some(p), _) => format!("p{p}"),
                (_, Some(l)) => format!("L{l}"),
                _ => String::new(),
            };
            println!(
                "{}{} {}",
                h.path,
                paint(&format!(":{at}:"), DIM),
                truncate(&h.line, 140)
            );
        }
        if truncated {
            println!();
            println!(
                "{}",
                paint(&format!("stopped at {limit} matches — raise it with -n"), DIM)
            );
        }
        Ok(())
    }

    // ── read ─────────────────────────────────────────────────────────────────

    /// Print one file's text: page markdown for a parsed document, the bytes
    /// on disk for anything else.
    fn read(&self, args: &ReadArgs) -> Result<(), String> {
        let pool = self.db().ok_or("the library index lives in the database")?;
        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let ids: Vec<i64> = match &args.subject {
            Some(code) => filter_subjects(&subjects, std::slice::from_ref(code), false)?
                .iter()
                .map(|s| s.id)
                .collect(),
            None => Vec::new(),
        };
        let files = self.library_files(&pool, &ids)?;
        let file = resolve_file(&files, &args.file)?;

        let wanted = args.pages.as_deref().map(parse_page_spec).transpose()?;

        #[derive(Serialize)]
        struct Page {
            page_no: i64,
            markdown: String,
        }
        #[derive(Serialize)]
        struct Document<'a> {
            path: &'a str,
            filename: &'a str,
            subject: &'a str,
            file_type: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            pages: Option<Vec<Page>>,
            #[serde(skip_serializing_if = "Option::is_none")]
            text: Option<String>,
        }

        let stored = self.pages_of(&pool, file.id)?;
        let stored = (!stored.is_empty()).then_some(stored);

        // A document that should have pages but has none is the one case worth
        // an error: printing nothing looks identical to a file with no content.
        if stored.is_none() && app_lib::paths::doc_pdf_rel(&file.relative_path).is_some() {
            return Err(format!(
                "{} has not been parsed, so there is no text to read.\n       \
                 Run `oculus index {}` with the Oculus app open.",
                file.relative_path, file.code
            ));
        }

        let doc = match &stored {
            Some(rows) => {
                let selected: Vec<Page> = rows
                    .iter()
                    .filter(|(n, _)| wanted.as_ref().is_none_or(|w| page_wanted(w, *n)))
                    .map(|(n, md)| Page { page_no: *n, markdown: md.clone() })
                    .collect();
                if selected.is_empty() {
                    let last = rows.last().map(|(n, _)| *n).unwrap_or(0);
                    return Err(format!(
                        "no such page — {} has pages 1-{last}",
                        file.relative_path
                    ));
                }
                Document {
                    path: &file.relative_path,
                    filename: &file.filename,
                    subject: &file.code,
                    file_type: &file.file_type,
                    pages: Some(selected),
                    text: None,
                }
            }
            None => {
                let abs = self.data_dir.join(&file.relative_path);
                if !is_text_file(&file.relative_path) {
                    return Err(format!(
                        "{} is not text — it is on disk at {}",
                        file.relative_path,
                        abs.display()
                    ));
                }
                let text = std::fs::read_to_string(&abs)
                    .map_err(|e| format!("{}: {e}", abs.display()))?;
                Document {
                    path: &file.relative_path,
                    filename: &file.filename,
                    subject: &file.code,
                    file_type: &file.file_type,
                    pages: None,
                    text: Some(text),
                }
            }
        };

        if self.json {
            return self.emit(&doc);
        }
        match (&doc.pages, &doc.text) {
            (Some(pages), _) => {
                println!(
                    "{}  {}",
                    paint(doc.path, BOLD),
                    paint(&format!("{} page(s)", pages.len()), DIM)
                );
                for p in pages {
                    println!();
                    println!("{}", paint(&format!("── page {} ──", p.page_no), DIM));
                    println!("{}", p.markdown);
                }
            }
            (_, Some(text)) => {
                println!("{}", paint(doc.path, BOLD));
                println!();
                print!("{text}");
            }
            _ => {}
        }
        Ok(())
    }

    // ── files ────────────────────────────────────────────────────────────────

    fn files(&self, args: &FilesArgs) -> Result<(), String> {
        let pool = self.db().ok_or("the library index lives in the database")?;
        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let ids: Vec<i64> = filter_subjects(&subjects, &args.codes, false)?
            .iter()
            .map(|s| s.id)
            .collect();

        let needle = args.r#match.as_ref().map(|m| m.to_lowercase());
        let all = filter_categories(self.library_files(&pool, &ids)?, &args.category)?;
        let rows: Vec<&LibFile> = all
            .iter()
            .filter(|f| {
                args.r#type.as_ref().is_none_or(|t| f.file_type.eq_ignore_ascii_case(t))
                    && needle
                        .as_ref()
                        .is_none_or(|n| f.relative_path.to_lowercase().contains(n))
                    && (!args.indexed || f.indexed_pages > 0)
            })
            .take(args.limit.max(1))
            .collect();

        #[derive(Serialize)]
        struct Entry<'a> {
            subject: &'a str,
            path: &'a str,
            filename: &'a str,
            file_type: &'a str,
            category: Option<&'a str>,
            size_bytes: Option<i64>,
            parse_status: Option<&'a str>,
            indexed_pages: i64,
        }
        if self.json {
            let out: Vec<Entry> = rows
                .iter()
                .map(|f| Entry {
                    subject: &f.code,
                    path: &f.relative_path,
                    filename: &f.filename,
                    file_type: &f.file_type,
                    category: f.category.as_deref(),
                    size_bytes: f.size_bytes,
                    parse_status: f.parse_status.as_deref(),
                    indexed_pages: f.indexed_pages,
                })
                .collect();
            return self.emit(&out);
        }

        if rows.is_empty() {
            println!("{}", paint("no matching files", DIM));
            return Ok(());
        }
        let mut current = "";
        for f in &rows {
            if f.code != current {
                current = &f.code;
                println!("{}", paint(current, BOLD));
            }
            let indexed = if f.indexed_pages > 0 {
                format!("{}p", f.indexed_pages)
            } else {
                "-".to_string()
            };
            // The course prefix is already the section header.
            let short = f.relative_path.splitn(3, '/').nth(2).unwrap_or(&f.relative_path);
            println!(
                "  {} {} {} {}",
                paint(&format!("{:<5}", truncate(&f.file_type, 5)), DIM),
                paint(&format!("{indexed:>5}"), DIM),
                paint(&format!("{:>9}", human_bytes(f.size_bytes.unwrap_or(0) as u64)), DIM),
                short
            );
        }
        Ok(())
    }

    // ── calendar ─────────────────────────────────────────────────────────────

    fn calendar(&self, args: &CalendarArgs) -> Result<(), String> {
        let pool = self.db().ok_or("the calendar lives in the database")?;
        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let ids: Vec<i64> = filter_subjects(&subjects, &args.codes, false)?
            .iter()
            .map(|s| s.id)
            .collect();

        let mut clauses: Vec<String> = Vec::new();
        if !ids.is_empty() {
            let list: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
            clauses.push(format!("e.subject_id IN ({})", list.join(",")));
        }
        if args.due {
            clauses.push("e.kind = 'due'".to_string());
        }
        // Stored as ISO-8601 UTC, so a string comparison against SQLite's own
        // UTC clock is the whole date filter — no date library needed.
        if !args.past {
            clauses.push("e.start_at >= strftime('%Y-%m-%dT%H:%M:%SZ','now')".to_string());
        }
        clauses.push(format!(
            "e.start_at < strftime('%Y-%m-%dT%H:%M:%SZ','now','+{} days')",
            args.days.max(0)
        ));
        let sql = format!(
            r#"SELECT s.code AS code, e.kind AS kind, e.title AS title,
                      e.start_at AS start_at, e.location AS location, e.url AS url,
                      strftime('%Y-%m-%d %H:%M', e.start_at, 'localtime') AS local_at
               FROM calendar_events e JOIN subjects s ON s.id = e.subject_id
               WHERE {}
               ORDER BY e.start_at"#,
            clauses.join(" AND ")
        );

        #[derive(Serialize)]
        struct Event {
            subject: String,
            kind: String,
            title: String,
            start_at: String,
            starts_local: String,
            location: Option<String>,
            url: Option<String>,
        }
        let events: Vec<Event> = self.rt.block_on(async {
            let rows = sqlx::query(&sql).fetch_all(&pool).await.map_err(|e| e.to_string())?;
            Ok::<_, String>(
                rows.iter()
                    .map(|r| Event {
                        subject: r.try_get("code").unwrap_or_default(),
                        kind: r.try_get("kind").unwrap_or_default(),
                        title: r.try_get("title").unwrap_or_default(),
                        start_at: r.try_get("start_at").unwrap_or_default(),
                        starts_local: r.try_get("local_at").unwrap_or_default(),
                        location: r.try_get("location").ok().flatten(),
                        url: r.try_get("url").ok().flatten(),
                    })
                    .collect(),
            )
        })?;

        if self.json {
            return self.emit(&events);
        }
        if events.is_empty() {
            println!(
                "{}",
                paint(
                    &format!("nothing in the next {} day(s)", args.days.max(0)),
                    DIM
                )
            );
            return Ok(());
        }
        for e in &events {
            println!(
                "{} {} {:<20} {}{}",
                paint(&e.starts_local, DIM),
                if e.kind == "due" { paint("due  ", YELLOW) } else { paint("class", DIM) },
                truncate(&e.subject, 20),
                truncate(&e.title, 44),
                match e.location.as_deref().filter(|l| !l.is_empty()) {
                    Some(l) => paint(&format!("  {}", truncate(l, 34)), DIM),
                    None => String::new(),
                }
            );
        }
        Ok(())
    }

    // ── projects and tasks ───────────────────────────────────────────────────

    fn planning_db(&self) -> Result<SqlitePool, String> {
        self.db().ok_or_else(|| "projects live in the database".to_string())
    }

    /// One subject id from a code.
    ///
    /// Prefix codes are fine — the same match `run` and `calendar` use — but a
    /// project points at exactly one subject, and a bare code legitimately
    /// matches the same course in two terms. So a tie is broken in favour of
    /// the **current** term, which is the one a student naming a course
    /// without a term means; a tie that survives that is reported with the
    /// full codes rather than guessed.
    fn one_subject(&self, pool: &SqlitePool, code: &str) -> Result<i64, String> {
        let subjects = self.rt.block_on(store::subjects(pool))?;
        let matched = filter_subjects(&subjects, &[code.to_string()], false)?;
        if matched.len() == 1 {
            return Ok(matched[0].id);
        }
        let current: Vec<&store::SubjectRow> =
            matched.iter().filter(|s| s.is_current).collect();
        if current.len() == 1 {
            return Ok(current[0].id);
        }
        let codes: Vec<&str> = matched.iter().map(|s| s.code.as_str()).collect();
        Err(format!(
            "{code} matched {} subjects ({}) — pass the full code",
            matched.len(),
            codes.join(", ")
        ))
    }

    fn project_list(&self, args: &ProjectListArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let filter = if args.personal {
            projects::SubjectFilter::Personal
        } else if let Some(code) = &args.subject {
            projects::SubjectFilter::Subject(self.one_subject(&pool, code)?)
        } else {
            projects::SubjectFilter::Any
        };
        let status = if args.archived { "archived" } else { "active" };
        let rows = self.rt.block_on(projects::projects(&pool, filter, status))?;

        #[derive(Serialize)]
        struct Entry<'a> {
            #[serde(flatten)]
            project: &'a projects::Project,
            tasks_total: i64,
            tasks_done: i64,
        }
        let mut entries: Vec<Entry> = Vec::with_capacity(rows.len());
        for p in &rows {
            let (total, done) = self.rt.block_on(projects::task_counts(&pool, p.id))?;
            entries.push(Entry { project: p, tasks_total: total, tasks_done: done });
        }

        if self.json {
            return self.emit(&entries);
        }
        if entries.is_empty() {
            println!("{}", paint(&format!("no {status} projects"), DIM));
            return Ok(());
        }
        for e in &entries {
            println!(
                "{} {:<34} {} {} {}",
                paint(&format!("{:>4}", e.project.id), DIM),
                truncate(&e.project.name, 34),
                paint(
                    &format!("{:<12}", truncate(e.project.subject_code.as_deref().unwrap_or("personal"), 12)),
                    DIM
                ),
                paint(&format!("{:>7}", format!("{}/{}", e.tasks_done, e.tasks_total)), DIM),
                match &e.project.due_at {
                    Some(d) => paint(&format!("  due {d}"), YELLOW),
                    None => String::new(),
                }
            );
        }
        Ok(())
    }

    fn project_show(&self, args: &ProjectShowArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let project = self
            .rt
            .block_on(projects::project(&pool, args.id))?
            .ok_or_else(|| format!("project {} does not exist", args.id))?;
        let tasks = self.rt.block_on(projects::tasks(&pool, args.id))?;

        if self.json {
            return self.emit(&serde_json::json!({ "project": project, "tasks": tasks }));
        }
        println!(
            "{} {}{}",
            paint(&format!("#{}", project.id), DIM),
            paint(&project.name, BOLD),
            match &project.subject_code {
                Some(c) => paint(&format!("  {c}"), DIM),
                None => String::new(),
            }
        );
        let mut meta: Vec<String> = vec![project.status.clone()];
        if let Some(d) = &project.starts_at {
            meta.push(format!("starts {d}"));
        }
        if let Some(d) = &project.due_at {
            meta.push(format!("due {d}"));
        }
        if !project.tags.is_empty() {
            meta.push(project.tags.join(", "));
        }
        // Said, not resolved: the calendar is three tables and the CLI has no
        // reader for the grid, so the most this can honestly report is that the
        // project is pinned to something the app will draw.
        if project.event_id.is_some() {
            meta.push("pinned to a calendar event".to_string());
        }
        println!("{}", paint(&meta.join("  ·  "), DIM));
        if let Some(brief) = project.brief.as_deref().filter(|b| !b.trim().is_empty()) {
            println!("\n{brief}");
        }
        println!();
        if tasks.is_empty() {
            println!(
                "{}",
                paint(
                    &format!("no tasks yet — oculus task add -p {} --batch -", project.id),
                    DIM
                )
            );
            return Ok(());
        }
        print_board(&project, &tasks);
        Ok(())
    }

    fn project_create(&self, args: &ProjectCreateArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let subject_id = match &args.subject {
            Some(code) => Some(self.one_subject(&pool, code)?),
            None => None,
        };
        let input = projects::NewProject {
            name: args.name.trim().to_string(),
            subject_id,
            brief: args.brief.clone(),
            starts_at: args.starts.as_deref().map(projects::check_iso8601).transpose()?,
            due_at: args.due.as_deref().map(projects::check_iso8601).transpose()?,
            tags: split_tags(args.tags.as_deref()),
            source: AGENT_SOURCE.to_string(),
        };
        if input.name.is_empty() {
            return Err("a project needs a name".to_string());
        }
        let id = self.rt.block_on(projects::create_project(&pool, &input))?;
        let created = self
            .rt
            .block_on(projects::project(&pool, id))?
            .ok_or("the project was written but could not be read back")?;
        if self.json {
            return self.emit(&created);
        }
        println!("{} {}", paint(&format!("project {id}"), BOLD), created.name);
        Ok(())
    }

    fn project_update(&self, args: &ProjectUpdateArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let patch = projects::ProjectPatch {
            name: args.name.clone(),
            subject_id: None,
            brief: nullable_text(args.brief.as_ref()),
            status: args.status.clone(),
            starts_at: nullable_date(args.starts.as_ref())?,
            due_at: nullable_date(args.due.as_ref())?,
            // `--tags ""` clears rather than being ignored, matching the other
            // "" flags here — so the empty list is `Some(vec![])`, not `None`.
            tags: args.tags.as_deref().map(|t| split_tags(Some(t))),
        };
        if patch.name.is_none()
            && patch.brief.is_none()
            && patch.status.is_none()
            && patch.starts_at.is_none()
            && patch.due_at.is_none()
            && patch.tags.is_none()
        {
            return Err(
                "nothing to change: pass --name, --due, --starts, --brief, --tags or --status".into(),
            );
        }
        self.rt.block_on(projects::update_project(&pool, args.id, &patch))?;
        let updated = self
            .rt
            .block_on(projects::project(&pool, args.id))?
            .ok_or_else(|| format!("project {} does not exist", args.id))?;
        if self.json {
            return self.emit(&updated);
        }
        println!("{} {}", paint(&format!("project {}", updated.id), BOLD), updated.name);
        Ok(())
    }

    fn task_list(&self, args: &TaskListArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let Some(project_id) = args.project else {
            return self.task_list_across(&pool, args);
        };
        let project = self
            .rt
            .block_on(projects::project(&pool, project_id))?
            .ok_or_else(|| format!("project {project_id} does not exist"))?;
        let mut tasks = self.rt.block_on(projects::tasks(&pool, project_id))?;

        if let Some(column) = &args.column {
            if !project.columns.iter().any(|c| &c.id == column) {
                let known: Vec<&str> = project.columns.iter().map(|c| c.id.as_str()).collect();
                return Err(format!(
                    "project {} has no column \"{column}\" (has: {})",
                    project.id,
                    known.join(", ")
                ));
            }
            tasks.retain(|t| &t.column_id == column);
        }
        if let Some(before) = &args.due_before {
            let before = projects::check_iso8601(before)?;
            tasks.retain(|t| t.due_at.as_deref().is_some_and(|d| d < before.as_str()));
        }

        if self.json {
            return self.emit(&tasks);
        }
        if tasks.is_empty() {
            println!("{}", paint("no matching tasks", DIM));
            return Ok(());
        }
        print_board(&project, &tasks);
        Ok(())
    }

    /// `task list` with no `-p`: every task there is, or only the unfiled ones.
    ///
    /// One board per project, under the project's own name, with the unfiled
    /// pile first — the order `projects::all_tasks` comes back in, so the
    /// grouping is one pass rather than a query per project. A column id could
    /// not be filtered on here at all (it only means something against one
    /// board), which is why `--column` `requires` `--project`.
    fn task_list_across(&self, pool: &SqlitePool, args: &TaskListArgs) -> Result<(), String> {
        let scope = if args.unfiled {
            projects::TaskScope::Unfiled
        } else {
            projects::TaskScope::All
        };
        let mut tasks = self.rt.block_on(projects::all_tasks(pool, scope))?;
        if let Some(before) = &args.due_before {
            let before = projects::check_iso8601(before)?;
            tasks.retain(|t| t.due_at.as_deref().is_some_and(|d| d < before.as_str()));
        }
        if self.json {
            return self.emit(&tasks);
        }
        if tasks.is_empty() {
            println!(
                "{}",
                paint(
                    if args.unfiled {
                        "nothing unfiled \u{2014} every task you have belongs to a project"
                    } else {
                        "no tasks yet \u{2014} oculus task add \"something to do\""
                    },
                    DIM
                )
            );
            return Ok(());
        }

        // Read once and reused per group: a task carries its project's id, not
        // its board, and the board is what the columns are printed in the
        // order of. `status: "all"` — an archived project's tasks are still
        // tasks, and dropping them would be a silently short list.
        let all = self.rt.block_on(projects::projects(pool, projects::SubjectFilter::Any, "all"))?;

        // A blank line *between* groups, never before the first one.
        let mut written = false;
        let unfiled: Vec<&projects::Task> =
            tasks.iter().filter(|t| t.project_id.is_none()).collect();
        if !unfiled.is_empty() {
            println!("{}", paint("Unfiled", BOLD));
            // The default board, which is the one an unfiled task's column is
            // checked against (`projects::board_of`).
            let rows: Vec<projects::Task> = unfiled.into_iter().cloned().collect();
            print_columns(&projects::default_columns(), &rows);
            written = true;
        }
        for project in &all {
            let here: Vec<projects::Task> = tasks
                .iter()
                .filter(|t| t.project_id == Some(project.id))
                .cloned()
                .collect();
            if here.is_empty() {
                continue;
            }
            if written {
                println!();
            }
            written = true;
            println!(
                "{} {}{}",
                paint(&format!("#{}", project.id), DIM),
                paint(&project.name, BOLD),
                match &project.subject_code {
                    Some(c) => paint(&format!("  {c}"), DIM),
                    None => String::new(),
                }
            );
            print_board(project, &here);
        }
        Ok(())
    }

    fn task_add(&self, args: &TaskAddArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let mut items: Vec<projects::NewTask> = match &args.batch {
            Some(source) => {
                let text = if source == "-" {
                    let mut buffer = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer)
                        .map_err(|e| format!("reading the batch from stdin: {e}"))?;
                    buffer
                } else {
                    std::fs::read_to_string(source)
                        .map_err(|e| format!("reading {source}: {e}"))?
                };
                if text.trim().is_empty() {
                    return Err("--batch got an empty input".to_string());
                }
                serde_json::from_str(&text)
                    .map_err(|e| format!("--batch wants a JSON array of tasks: {e}"))?
            }
            None => {
                let title = args
                    .title
                    .clone()
                    .ok_or("give a TITLE, or --batch - to read a JSON array of tasks from stdin")?;
                vec![projects::NewTask {
                    title,
                    column: args.column.clone(),
                    parent: args.parent.map(projects::ParentRef::Id),
                    body: args.body.clone(),
                    due: args.due.clone(),
                    starts: args.starts.clone(),
                    estimate: args.estimate,
                    key: None,
                }]
            }
        };

        // Dates are validated here and stored verbatim — the library keeps
        // every timestamp exactly as its source wrote it.
        let many = items.len() > 1;
        for (n, item) in items.iter_mut().enumerate() {
            let at = |e: String| if many { format!("task {}: {e}", n + 1) } else { e };
            if let Some(due) = &item.due {
                item.due = Some(projects::check_iso8601(due).map_err(at)?);
            }
            if let Some(starts) = &item.starts {
                item.starts = Some(projects::check_iso8601(starts).map_err(at)?);
            }
        }

        let ids = self
            .rt
            .block_on(projects::create_tasks(&pool, args.project, &items, AGENT_SOURCE))?;
        let mut created: Vec<projects::Task> = Vec::with_capacity(ids.len());
        for id in &ids {
            if let Some(task) = self.rt.block_on(projects::task(&pool, *id))? {
                created.push(task);
            }
        }
        if self.json {
            return self.emit(&created);
        }
        for task in &created {
            println!(
                "{} {}",
                paint(&format!("task {}", task.id), BOLD),
                truncate(&task.title, 60)
            );
        }
        Ok(())
    }

    fn task_update(&self, args: &TaskUpdateArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let patch = projects::TaskPatch {
            title: args.title.clone().filter(|t| !t.trim().is_empty()),
            body: nullable_text(args.body.as_ref()),
            parent_id: None,
            starts_at: nullable_date(args.starts.as_ref())?,
            due_at: nullable_date(args.due.as_ref())?,
            estimate_minutes: nullable_minutes(args.estimate.as_ref())?,
        };
        if patch.title.is_none()
            && patch.body.is_none()
            && patch.starts_at.is_none()
            && patch.due_at.is_none()
            && patch.estimate_minutes.is_none()
        {
            return Err("nothing to change: pass --title, --body, --due, --starts or --estimate".into());
        }
        self.rt.block_on(projects::update_task(&pool, args.id, &patch))?;
        self.print_task(&pool, args.id, None)
    }

    fn task_move(&self, args: &TaskMoveArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        self.rt.block_on(projects::move_task(
            &pool,
            args.id,
            &args.column,
            args.after,
            args.before,
        ))?;
        self.print_task(&pool, args.id, Some(&args.column))
    }

    fn task_refile(&self, args: &TaskRefileArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        // Two ways to say it and no default: refiling somewhere and refiling
        // nowhere are both deliberate, and a bare `task refile 12` could only
        // guess which was meant.
        let destination = match (args.project, args.unfiled) {
            (Some(id), false) => Some(id),
            (None, true) => None,
            _ => {
                return Err(
                    "say where: -p <PROJECT_ID>, or --unfiled to take it out of every project"
                        .to_string(),
                )
            }
        };
        let rows = self
            .rt
            .block_on(projects::refile_task(&pool, args.id, destination))?;

        // The project's *name*, not its id: the id is what was typed, and the
        // name is the confirmation that it was the right one.
        let label = match destination {
            Some(id) => self
                .rt
                .block_on(projects::project(&pool, id))?
                .map(|p| p.name)
                .unwrap_or_else(|| format!("project {id}")),
            None => "unfiled".to_string(),
        };
        if self.json {
            return self.emit(&serde_json::json!({
                "refiled": args.id,
                "project_id": destination,
                "project": label,
                "rows": rows,
            }));
        }
        if rows == 0 {
            println!("{}", paint(&format!("task {} is already there", args.id), DIM));
            return Ok(());
        }
        self.print_task(&pool, args.id, Some(&label))?;
        if rows > 1 {
            println!(
                "{}",
                paint(&format!("  {} subtask(s) came with it", rows - 1), DIM)
            );
        }
        Ok(())
    }

    fn task_rm(&self, args: &TaskRmArgs) -> Result<(), String> {
        let pool = self.planning_db()?;
        let rows = self.rt.block_on(projects::delete_task(&pool, args.id))?;
        if self.json {
            return self.emit(&serde_json::json!({ "deleted": args.id, "rows": rows }));
        }
        println!(
            "deleted task {}{}",
            args.id,
            match rows {
                1 => String::new(),
                n => format!(" and {} subtask(s)", n - 1),
            }
        );
        Ok(())
    }

    // ── lecture ──────────────────────────────────────────────────────────────

    /// Where a recording changes topic, from the recording itself.
    ///
    /// No cache to check and nothing written unless `--frames` is passed: one
    /// decode pass is cheaper than a table to invalidate. See
    /// `app_lib::chapters` for why the thresholds are constants.
    fn lecture_candidates(&self, args: &LectureCandidatesArgs) -> Result<(), String> {
        let pool = self.db().ok_or("lectures live in the database")?;
        let (id, title, duration, video, transcript) = self.one_lecture(&pool, &args.id)?;

        let video = video.ok_or_else(|| {
            format!("{title} is not downloaded — `oculus run -l --videos` fetches it")
        })?;
        let video = PathBuf::from(&video);
        if !video.exists() {
            return Err(format!(
                "{} is on record but missing from disk",
                video.display()
            ));
        }
        // The CLI has no resource dir, so this finds the dev copy under
        // src-tauri/binaries or a system ffmpeg — the same search the
        // downloader uses.
        let ffmpeg = app_lib::echo360::find_ffmpeg(None)
            .ok_or("no ffmpeg found — install it, or run `bun run ffmpeg`")?;

        // A missing or unreadable transcript costs the pause bonus and nothing
        // else, so it is not worth failing over.
        let gaps = transcript
            .as_deref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|vtt| app_lib::chapters::cue_gaps(&vtt))
            .unwrap_or_default();
        let dir = app_lib::echo360::lecture_dir(&self.data_dir, &id);
        // Which stream holds the slides is measured, not assumed; `--source`
        // overrules it. See `app_lib::chapters::detect`.
        let detected = app_lib::chapters::detect(
            &ffmpeg,
            &dir,
            &video,
            &gaps,
            duration as u32,
            args.source,
            |_| {},
        )?;
        let found = &detected.candidates;

        let frames = if args.frames {
            let seconds: Vec<u32> = found.iter().map(|c| c.seconds).collect();
            Some(app_lib::chapters::extract_frames(
                &ffmpeg,
                &detected.video,
                &seconds,
                &dir.join("frames"),
                |_| {},
            )?)
        } else {
            None
        };

        if self.json {
            #[derive(Serialize)]
            struct Out<'a> {
                lecture: &'a str,
                title: &'a str,
                duration_seconds: i64,
                source: u8,
                sampled_seconds: usize,
                candidates: &'a [app_lib::chapters::Candidate],
                #[serde(skip_serializing_if = "Option::is_none")]
                frames: Option<Vec<String>>,
            }
            return self.emit(&Out {
                lecture: &id,
                title: &title,
                duration_seconds: duration,
                source: detected.source,
                sampled_seconds: detected.diffs.len() + 1,
                candidates: found,
                frames: frames
                    .as_ref()
                    .map(|f| f.iter().map(|p| p.to_string_lossy().into_owned()).collect()),
            });
        }

        println!(
            "{}  {}  {}",
            paint(&title, BOLD),
            paint(&clock(duration as u32), DIM),
            paint(&format!("source {}", detected.source), DIM)
        );
        if found.is_empty() {
            println!("{}", paint("no boundaries — one continuous slide?", DIM));
            return Ok(());
        }
        for c in found {
            println!(
                "  {}  {}{}",
                clock(c.seconds),
                paint(&format!("{:>7.1}", c.score), DIM),
                if c.pause { paint("  pause", DIM) } else { String::new() }
            );
        }
        println!(
            "{}",
            paint(&format!("{} candidate(s)", found.len()), DIM)
        );
        if let Some(frames) = &frames {
            if let Some(first) = frames.first().and_then(|p| p.parent()) {
                println!("{}", paint(&format!("frames in {}", first.display()), DIM));
            }
        }
        Ok(())
    }

    /// Name a recording's chapters with a CLI agent, and store them.
    ///
    /// The detector from `lecture_candidates` runs first and a frame is
    /// grabbed for every candidate, because the whole advantage of driving a
    /// coding agent rather than calling a model API is that it can *open* the
    /// five frames it is unsure about — a stuffed prompt would have to carry
    /// all fifty. So the prompt stays small (the candidate list, two paths,
    /// the title and the duration) and the agent does the reading.
    ///
    /// The agent replies with JSON and nothing more: chapters are derived data
    /// like `pages`, not the student's own planning, so unlike `oculus project`
    /// there is no write door for a model here. Rust parses the reply,
    /// validates it against the candidate set, and writes the rows.
    ///
    /// The job itself is `chapters::run`, which the app's
    /// `lecture_find_chapters` command runs too — this function is the flags,
    /// the resolved selection and the printing around it, and nothing else.
    fn lecture_chapters(&self, args: &LectureChaptersArgs) -> Result<(), String> {
        use app_lib::harness::{jobs, Provider};

        let pool = self.db().ok_or("lectures live in the database")?;
        let (id, title, ..) = self.one_lecture(&pool, &args.id)?;

        // `chapters::run` guards this too — it has to, the app calls it — but
        // the shared message cannot name a flag only this door has, and an
        // error that does not say how to get past it is half an error.
        if !args.force {
            let existing = self.rt.block_on(store::chapters(&pool, &id))?;
            if !existing.is_empty() {
                return Err(format!(
                    "{title} already has {} chapter(s) — `--force` re-runs and replaces them",
                    existing.len()
                ));
            }
        }

        // The configured selection is the default; a flag replaces the part of
        // it that was named. Nothing is hardcoded on this side any more — the
        // app runs the same job off the same row.
        let mut selection = self
            .rt
            .block_on(jobs::selection(&pool, jobs::Job::LectureChapters));
        if let Some(p) = &args.provider {
            selection.provider = Provider::parse(p).ok_or("unknown provider")?;
        }
        if let Some(m) = &args.model {
            selection.model = m.clone();
        }
        if let Some(e) = &args.effort {
            selection.reasoning_effort = Some(e.clone());
        }

        let quiet = self.json;
        if !quiet {
            println!(
                "{}",
                paint(
                    &format!(
                        "{} · {}{}",
                        selection.provider.label(),
                        selection.model,
                        match selection.effort() {
                            Some(e) => format!(" · {e} reasoning"),
                            None => String::new(),
                        }
                    ),
                    DIM
                )
            );
        }

        // Assistant text is not echoed: the reply *is* the chapter JSON, and
        // it is printed properly below. The tool rows are the interesting part
        // while the turn runs.
        let printer = AgentPrinter::new(false);
        let outcome = app_lib::chapters::run(
            self.rt.handle(),
            &pool,
            &app_lib::chapters::Run {
                data_dir: &self.data_dir,
                lecture_id: &id,
                selection: &selection,
                force: args.force,
                source: args.source,
            },
            // The terminal's progress is the agent's tool rows below; of the
            // pipeline's own steps only the candidate set is worth a line, and
            // a decode percentage redrawn over itself would have to know
            // whether stdout is a tty.
            |step| {
                if let app_lib::chapters::Step::Detected { title, duration, candidates } = step {
                    if !quiet {
                        println!(
                            "{}  {}  {}",
                            paint(title, BOLD),
                            paint(&clock(duration), DIM),
                            paint(&format!("{candidates} candidate(s)"), DIM)
                        );
                    }
                }
            },
            move |ev| {
                if !quiet {
                    printer.print(ev);
                }
            },
        )?;

        if self.json {
            #[derive(Serialize)]
            struct Out<'a> {
                lecture: &'a str,
                title: &'a str,
                duration_seconds: u32,
                provider: &'a str,
                model: &'a str,
                effort: Option<&'a str>,
                source: u8,
                candidates: usize,
                chapters: &'a [app_lib::chapters::Chapter],
            }
            return self.emit(&Out {
                lecture: &id,
                title: &outcome.title,
                duration_seconds: outcome.duration_seconds,
                provider: selection.provider.as_str(),
                model: &selection.model,
                effort: selection.effort(),
                source: outcome.source,
                candidates: outcome.candidates,
                chapters: &outcome.chapters,
            });
        }

        println!();
        for chapter in &outcome.chapters {
            println!(
                "  {}  {}",
                paint(&clock(chapter.start_seconds), DIM),
                paint(&chapter.title, BOLD)
            );
            println!("            {}", paint(&chapter.summary, DIM));
        }
        println!(
            "{}",
            paint(
                &format!(
                    "{} chapter(s) written for {}",
                    outcome.chapters.len(),
                    outcome.title
                ),
                DIM
            )
        );
        Ok(())
    }

    /// Write a recording's reading copy with a CLI agent.
    ///
    /// `reading::run` is the one implementation shared with the app. It owns
    /// the file checks, segmentation, retries, per-window transactions and
    /// status changes; this door only resolves the lecture and model flags
    /// and turns its progress into terminal output.
    fn lecture_reading(&self, args: &LectureReadingArgs) -> Result<(), String> {
        use app_lib::harness::{jobs, Provider};

        let pool = self.db().ok_or("lectures live in the database")?;
        let (id, title, ..) = self.one_lecture(&pool, &args.id)?;

        // Give the CLI-specific escape hatch in the early error. The runner
        // repeats this guard because the app calls it directly too.
        if !args.force {
            let existing = self.rt.block_on(store::reading(&pool, &id))?;
            if !existing.is_empty() {
                return Err(format!(
                    "{title} already has a reading copy of {} line(s) — `--force` re-runs and replaces it",
                    existing.len()
                ));
            }
        }

        let mut selection = self
            .rt
            .block_on(jobs::selection(&pool, jobs::Job::LectureReading));
        if let Some(p) = &args.provider {
            selection.provider = Provider::parse(p).ok_or("unknown provider")?;
        }
        if let Some(m) = &args.model {
            selection.model = m.clone();
        }
        if let Some(e) = &args.effort {
            selection.reasoning_effort = Some(e.clone());
        }

        let quiet = self.json;
        if !quiet {
            println!(
                "{}",
                paint(
                    &format!(
                        "{} · {}{}",
                        selection.provider.label(),
                        selection.model,
                        match selection.effort() {
                            Some(e) => format!(" · {e} reasoning"),
                            None => String::new(),
                        }
                    ),
                    DIM
                )
            );
        }

        // The reply text is machine-shaped JSON and becomes the lines below;
        // tool rows remain useful while each sequential window is running.
        let printer = AgentPrinter::new(false);
        let outcome = app_lib::reading::run(
            self.rt.handle(),
            &pool,
            &app_lib::reading::Run {
                data_dir: &self.data_dir,
                lecture_id: &id,
                selection: &selection,
                force: args.force,
                source: args.source,
            },
            |step| {
                if quiet {
                    return;
                }
                match step {
                    app_lib::reading::Step::Segmented {
                        title,
                        duration,
                        segments,
                    } => println!(
                        "{}  {}  {}",
                        paint(title, BOLD),
                        paint(&clock(duration), DIM),
                        paint(&format!("{segments} segment(s)"), DIM)
                    ),
                    app_lib::reading::Step::Window {
                        done,
                        total,
                        start,
                        end,
                    } => println!(
                        "{}",
                        paint(
                            &format!(
                                "window {done} of {total} — {}–{}",
                                clock(start),
                                clock(end)
                            ),
                            DIM
                        )
                    ),
                    _ => {}
                }
            },
            move |ev| {
                if !quiet {
                    printer.print(ev);
                }
            },
        )?;

        if self.json {
            #[derive(Serialize)]
            struct Out<'a> {
                lecture: &'a str,
                title: &'a str,
                duration_seconds: u32,
                provider: &'a str,
                model: &'a str,
                effort: Option<&'a str>,
                source: u8,
                segments: usize,
                windows: usize,
                lines: &'a [app_lib::reading::ReadingLine],
            }
            return self.emit(&Out {
                lecture: &id,
                title: &outcome.title,
                duration_seconds: outcome.duration_seconds,
                provider: selection.provider.as_str(),
                model: &selection.model,
                effort: selection.effort(),
                source: outcome.source,
                segments: outcome.segments,
                windows: outcome.windows,
                lines: &outcome.lines,
            });
        }

        println!();
        for line in &outcome.lines {
            println!("  {}  {}", paint(&clock(line.start_seconds), DIM), line.text);
        }
        println!(
            "{}",
            paint(
                &format!(
                    "{} line(s) written for {}",
                    outcome.lines.len(),
                    outcome.title
                ),
                DIM
            )
        );
        Ok(())
    }

    /// One lecture row from an id, or a unique prefix of one. Lecture ids are
    /// UUIDs nobody types in full, and a prefix that matches two lectures is
    /// reported rather than guessed — the same rule `one_subject` follows.
    #[allow(clippy::type_complexity)]
    fn one_lecture(
        &self,
        pool: &SqlitePool,
        id: &str,
    ) -> Result<(String, String, i64, Option<String>, Option<String>), String> {
        let rows = self
            .rt
            .block_on(
                sqlx::query(
                    "SELECT id, title, duration_seconds, video_path, transcript_path
                     FROM lectures WHERE id = ?1 OR id LIKE ?2 ORDER BY id",
                )
                .bind(id)
                .bind(format!("{id}%"))
                .fetch_all(pool),
            )
            .map_err(|e| e.to_string())?;
        let row = match rows.len() {
            0 => {
                return Err(format!(
                    "no lecture {id} — `oculus list -l` prints the id of every lecture on record"
                ))
            }
            1 => &rows[0],
            n => {
                let found: Vec<String> = rows.iter().map(|r| r.get::<String, _>("id")).collect();
                return Err(format!(
                    "{id} matched {n} lectures ({}) — pass more of the id",
                    found.join(", ")
                ));
            }
        };
        Ok((
            row.get("id"),
            row.get("title"),
            row.get("duration_seconds"),
            row.get("video_path"),
            row.get("transcript_path"),
        ))
    }

    /// Read one task back and report it — what `task update` and `task move`
    /// print, so a caller always sees the row as it now stands rather than the
    /// arguments it sent.
    fn print_task(&self, pool: &SqlitePool, id: i64, moved_to: Option<&str>) -> Result<(), String> {
        let task = self
            .rt
            .block_on(projects::task(pool, id))?
            .ok_or_else(|| format!("task {id} does not exist"))?;
        if self.json {
            return self.emit(&task);
        }
        let where_ = match moved_to {
            Some(column) => format!(" → {column}"),
            None => String::new(),
        };
        println!(
            "{}{} {}{}",
            paint(&format!("task {}", task.id), BOLD),
            paint(&where_, DIM),
            truncate(&task.title, 56),
            match &task.done_at {
                Some(at) => paint(&format!("  done {at}"), GREEN),
                None => String::new(),
            }
        );
        Ok(())
    }

    // ── shared loaders ───────────────────────────────────────────────────────

    /// Every library file for the given subjects (all of them when empty),
    /// ordered by subject then path — the order `grep` scans in and `files`
    /// prints in.
    fn library_files(&self, pool: &SqlitePool, ids: &[i64]) -> Result<Vec<LibFile>, String> {
        let filter = if ids.is_empty() {
            String::new()
        } else {
            let list: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
            format!(" WHERE f.subject_id IN ({})", list.join(","))
        };
        let sql = format!(
            r#"SELECT f.id AS id, s.code AS code, f.filename AS filename,
                      f.relative_path AS relative_path, f.file_type AS file_type,
                      f.category AS category, f.size_bytes AS size_bytes,
                      f.parse_status AS parse_status,
                      (SELECT COUNT(*) FROM pages p
                        WHERE p.file_id = f.id AND p.markdown != '') AS indexed_pages
               FROM files f JOIN subjects s ON s.id = f.subject_id{filter}
               ORDER BY s.code, f.relative_path"#
        );
        self.rt.block_on(async {
            let rows = sqlx::query(&sql).fetch_all(pool).await.map_err(|e| e.to_string())?;
            Ok(rows
                .iter()
                .map(|r| LibFile {
                    id: r.try_get("id").unwrap_or_default(),
                    code: r.try_get("code").unwrap_or_default(),
                    filename: r.try_get("filename").unwrap_or_default(),
                    relative_path: r.try_get("relative_path").unwrap_or_default(),
                    file_type: r.try_get("file_type").unwrap_or_default(),
                    category: r.try_get("category").ok().flatten(),
                    size_bytes: r.try_get("size_bytes").ok().flatten(),
                    parse_status: r.try_get("parse_status").ok().flatten(),
                    indexed_pages: r.try_get("indexed_pages").unwrap_or_default(),
                })
                .collect())
        })
    }

    /// One file's parsed pages, in order.
    fn pages_of(&self, pool: &SqlitePool, file_id: i64) -> Result<Vec<(i64, String)>, String> {
        self.rt.block_on(async {
            let rows = sqlx::query(
                "SELECT page_no, markdown FROM pages
                  WHERE file_id = ?1 AND markdown != '' ORDER BY page_no",
            )
            .bind(file_id)
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
            Ok(rows
                .iter()
                .map(|r| {
                    (
                        r.try_get("page_no").unwrap_or_default(),
                        r.try_get("markdown").unwrap_or_default(),
                    )
                })
                .collect())
        })
    }

    /// Parsed page markdown, keyed by file and ordered by page number.
    ///
    /// Loaded whole: a degree of coursework is a few thousand pages of a few
    /// hundred characters each, so this is a couple of megabytes and one
    /// query, against a scan that would otherwise be a query per file.
    fn page_text(
        &self,
        pool: &SqlitePool,
        ids: &[i64],
    ) -> Result<HashMap<i64, Vec<(i64, String)>>, String> {
        let filter = if ids.is_empty() {
            String::new()
        } else {
            let list: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
            format!(" AND f.subject_id IN ({})", list.join(","))
        };
        let sql = format!(
            r#"SELECT p.file_id AS file_id, p.page_no AS page_no, p.markdown AS markdown
               FROM pages p JOIN files f ON f.id = p.file_id
               WHERE p.markdown != ''{filter}
               ORDER BY p.file_id, p.page_no"#
        );
        self.rt.block_on(async {
            let rows = sqlx::query(&sql).fetch_all(pool).await.map_err(|e| e.to_string())?;
            let mut out: HashMap<i64, Vec<(i64, String)>> = HashMap::new();
            for r in &rows {
                let id: i64 = r.try_get("file_id").unwrap_or_default();
                let page: i64 = r.try_get("page_no").unwrap_or_default();
                let md: String = r.try_get("markdown").unwrap_or_default();
                out.entry(id).or_default().push((page, md));
            }
            Ok(out)
        })
    }

    // ── docs ──────────────────────────────────────────────────────────────────

    /// One turn of a CLI agent, events to stdout. `--json` prints each
    /// normalized event as a line; otherwise text streams and tool rows are
    /// summarised as they open and close.
    fn agent(&self, args: &AgentArgs) -> Result<(), String> {
        use app_lib::harness::{self, Provider};
        let provider = Provider::parse(&args.provider).ok_or("unknown provider")?;
        // The folder name is the scope; a headless run has no thread row to
        // resolve an id against, so the code is given directly.
        let opts = harness::SendOptions {
            model: args.model.clone(),
            reasoning_effort: args.effort.clone(),
            scope: args.subject.clone(),
            ..Default::default()
        };
        let json = self.json;
        let printer = AgentPrinter::new(true);
        harness::run_once(&self.data_dir, provider, &opts, &args.prompt, move |ev| {
            if json {
                if let Ok(line) = serde_json::to_string(ev) {
                    println!("{line}");
                }
                return;
            }
            printer.print(ev);
        })
    }

    fn docs(&self, args: &DocsArgs) -> Result<(), String> {
        if args.stdout {
            print!("{}", render_cli_docs());
            return Ok(());
        }

        // Everything that does not need clap's command tree is shared with the
        // sync path, so a folder scaffolded by a sync and one scaffolded here
        // cannot drift apart.
        let dir = agents::agents_dir(&self.data_dir);
        let docs = agents::ensure_library_docs(&self.data_dir)?;

        // The one file only this binary can produce: it is rendered from the
        // binary's own help, so it always describes the build that wrote it.
        let path = dir.join(agents::CLI_DOC_NAME);
        std::fs::write(&path, render_cli_docs())
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        let mut written = vec![agents::CLI_DOC_NAME];
        written.extend(docs.generated.iter().copied());

        let links = agents::link_all(&self.data_dir)?;

        if self.json {
            self.emit(&serde_json::json!({
                "dir": dir.to_string_lossy(),
                "generated": written,
                "created": docs.created,
                "linked": links.linked,
                "already_linked": links.current,
                "skipped": links.skipped,
            }))
        } else {
            println!("{} in {}", written.join(", "), dir.display());
            for name in &docs.created {
                println!("created {name} (yours now — it will not be overwritten)");
            }
            let n = links.linked.len();
            println!(
                "{n} course folder{} linked ({} already current)",
                if n == 1 { "" } else { "s" },
                links.current
            );
            for path in &links.skipped {
                eprintln!(
                    "{} {path}/AGENTS.md is a real file, left alone",
                    paint("warning:", YELLOW)
                );
            }
            Ok(())
        }
    }

}

/// Fixed so the output depends on the binary, not on the terminal that ran it.
const HELP_WIDTH: usize = 88;

/// Documented once at the root instead of under every subcommand.
const GLOBAL_ARGS: [&str; 1] = ["json"];

/// This binary's whole help tree as markdown.
///
/// Walks clap's own command tree rather than a hand-kept list, so a new
/// subcommand or flag appears here the moment it exists.
fn render_cli_docs() -> String {
    let mut root = Cli::command();
    root.build();

    let mut out = String::new();
    out.push_str("<!-- Generated by `oculus docs` from the binary's own help. Do not edit:\n");
    out.push_str("     change the CLI and regenerate, or the file will lie to whoever reads it. -->\n\n");
    out.push_str("# The `oculus` CLI\n\n");
    out.push_str("`--json` is the one global flag: it works on every command below,\n");
    out.push_str("and is listed once here rather than repeated in each section.\n\n");
    out.push_str(&help_block(&root, "oculus", true));
    for sub in root.get_subcommands() {
        render_subcommand(sub, "oculus", 2, &mut out);
    }
    out
}

fn render_subcommand(cmd: &clap::Command, prefix: &str, depth: usize, out: &mut String) {
    // `help` is clap's own, and hidden commands are hidden for a reason.
    if cmd.is_hide_set() || cmd.get_name() == "help" {
        return;
    }
    let path = format!("{prefix} {}", cmd.get_name());
    out.push_str(&format!("\n{} `{path}`\n\n", "#".repeat(depth)));
    out.push_str(&help_block(cmd, &path, false));
    for nested in cmd.get_subcommands() {
        render_subcommand(nested, &path, depth + 1, out);
    }
}

/// One command's long help, fenced. `bin_name` is set explicitly so the usage
/// line reads `oculus search`, not the bare subcommand name; the global options
/// are hidden below the root, where clap would otherwise repeat their full text
/// under every single subcommand.
fn help_block(cmd: &clap::Command, path: &str, globals: bool) -> String {
    let mut cmd = cmd
        .clone()
        .bin_name(path.to_string())
        .display_name(path.to_string())
        .term_width(HELP_WIDTH)
        .color(clap::ColorChoice::Never);
    if !globals {
        for id in GLOBAL_ARGS {
            if cmd.get_arguments().any(|a| a.get_id() == id) {
                cmd = cmd.mut_arg(id, |a| a.hide(true));
            }
        }
    }
    let help = cmd.render_long_help().to_string();
    let body: Vec<&str> = help.lines().map(|l| l.trim_end()).collect();
    format!("```\n{}\n```\n", body.join("\n").trim_end())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}

// ── Library lookup ───────────────────────────────────────────────────────────

/// One row of `files` with its subject code and how much of it is searchable.
struct LibFile {
    id: i64,
    code: String,
    filename: String,
    relative_path: String,
    file_type: String,
    category: Option<String>,
    size_bytes: Option<i64>,
    parse_status: Option<String>,
    indexed_pages: i64,
}

/// Extensions whose bytes are worth reading as text. Everything else in a
/// library is a document whose text lives in the database, or a binary.
const TEXT_EXTS: &[&str] = &["md", "txt", "csv", "json", "html", "htm", "vtt", "srt"];

fn is_text_file(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    TEXT_EXTS.iter().any(|e| lower.ends_with(&format!(".{e}")))
}

/// Find the one file a caller meant.
///
/// Tiered rather than fuzzy: an exact path beats an exact filename beats a
/// fragment, and only the *best* tier that matched anything is considered. A
/// tie inside that tier is reported, never guessed — a wrong file quietly
/// substituted is worse than a question.
fn resolve_file<'a>(files: &'a [LibFile], target: &str) -> Result<&'a LibFile, String> {
    let needle = target.to_lowercase();
    let tiers: [Box<dyn Fn(&LibFile) -> bool>; 4] = [
        Box::new(|f: &LibFile| f.relative_path == target),
        Box::new(|f: &LibFile| f.filename == target),
        Box::new(|f: &LibFile| f.filename.to_lowercase() == needle),
        Box::new(|f: &LibFile| f.relative_path.to_lowercase().contains(&needle)),
    ];

    for matches in tiers {
        let hits: Vec<&LibFile> = files.iter().filter(|f| matches(f)).collect();
        match hits.len() {
            0 => continue,
            1 => return Ok(hits[0]),
            _ => {
                let mut message = format!("{} matches {} files:\n", target, hits.len());
                for f in hits.iter().take(12) {
                    message.push_str(&format!("       {}\n", f.relative_path));
                }
                if hits.len() > 12 {
                    message.push_str(&format!("       … and {} more\n", hits.len() - 12));
                }
                message.push_str("       Name one of them, or narrow it with --subject.");
                return Err(message);
            }
        }
    }
    Err(format!(
        "no library file matches {target} — `oculus files -m {}` to look",
        shell_quote(target)
    ))
}

/// `12`, `12-15`, `12,14,20-22`, `30-` (to the end), `-4` (from the start).
fn parse_page_spec(spec: &str) -> Result<Vec<(i64, i64)>, String> {
    let mut ranges = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let bad = || format!("not a page range: {part}");
        let (lo, hi) = match part.split_once('-') {
            None => {
                let n: i64 = part.parse().map_err(|_| bad())?;
                (n, n)
            }
            Some((from, to)) => {
                let lo = if from.trim().is_empty() { 1 } else { from.trim().parse().map_err(|_| bad())? };
                let hi = if to.trim().is_empty() { i64::MAX } else { to.trim().parse().map_err(|_| bad())? };
                (lo, hi)
            }
        };
        if lo > hi {
            return Err(format!("empty page range: {part}"));
        }
        ranges.push((lo, hi));
    }
    if ranges.is_empty() {
        return Err("no pages given".to_string());
    }
    Ok(ranges)
}

fn page_wanted(ranges: &[(i64, i64)], page: i64) -> bool {
    ranges.iter().any(|(lo, hi)| page >= *lo && page <= *hi)
}

fn build_regex(pattern: &str, fixed: bool, case_sensitive: bool) -> Result<regex::Regex, String> {
    let body = if fixed { regex::escape(pattern) } else { pattern.to_string() };
    regex::RegexBuilder::new(&body)
        .case_insensitive(!case_sensitive)
        .build()
        .map_err(|e| format!("bad pattern: {e}"))
}

/// A page of markdown flattened to one line of prose, for a result list.
fn snippet(markdown: &str, max: usize) -> String {
    let flat: Vec<&str> = markdown.split_whitespace().collect();
    truncate(&flat.join(" "), max)
}

/// Quote a suggested command argument, so a filename with spaces in it can be
/// pasted — or run by an agent — without falling apart.
fn shell_quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || "._-/".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod query_tests {
    use super::*;

    #[test]
    fn page_specs_cover_points_ranges_and_open_ends() {
        let spec = parse_page_spec("3,7-9,20-").unwrap();
        for wanted in [3, 7, 8, 9, 20, 4000] {
            assert!(page_wanted(&spec, wanted), "{wanted} should be selected");
        }
        for unwanted in [2, 4, 6, 10, 19] {
            assert!(!page_wanted(&spec, unwanted), "{unwanted} should not be");
        }
        assert!(parse_page_spec("9-4").is_err());
        assert!(parse_page_spec("twelve").is_err());
        assert!(parse_page_spec("").is_err());
    }

    #[test]
    fn fixed_strings_do_not_read_as_patterns() {
        assert!(build_regex("a.c", false, false).unwrap().is_match("abc"));
        assert!(!build_regex("a.c", true, false).unwrap().is_match("abc"));
        assert!(build_regex("a.c", true, false).unwrap().is_match("A.C"));
        assert!(!build_regex("a.c", true, true).unwrap().is_match("A.C"));
    }

    fn file(code: &str, rel: &str) -> LibFile {
        LibFile {
            id: 0,
            code: code.to_string(),
            filename: rel.rsplit('/').next().unwrap().to_string(),
            relative_path: rel.to_string(),
            file_type: "pdf".to_string(),
            category: None,
            size_bytes: None,
            parse_status: None,
            indexed_pages: 0,
        }
    }

    /// The tiers exist so that a filename shared by two subjects still
    /// resolves when the caller typed the full path, and reports rather than
    /// guesses when they did not.
    #[test]
    fn resolution_prefers_the_most_exact_tier() {
        let files = vec![
            file("COMP30026", "courses/COMP30026/files/week-01.pdf"),
            file("MULT20015", "courses/MULT20015/files/week-01.pdf"),
            file("MULT20015", "courses/MULT20015/files/notes.pdf"),
        ];
        assert_eq!(
            resolve_file(&files, "courses/MULT20015/files/week-01.pdf").unwrap().code,
            "MULT20015"
        );
        assert_eq!(resolve_file(&files, "notes.pdf").unwrap().code, "MULT20015");
        assert_eq!(resolve_file(&files, "NOTES.PDF").unwrap().code, "MULT20015");
        assert_eq!(resolve_file(&files, "COMP30026/files/week").unwrap().code, "COMP30026");
        assert!(resolve_file(&files, "week-01.pdf").is_err());
        assert!(resolve_file(&files, "nothing-like-this").is_err());
    }

    fn filed(rel: &str, category: &str) -> LibFile {
        LibFile { category: Some(category.to_string()), ..file("COMP30026", rel) }
    }

    /// A category narrows *before* the match limit, which is the point of
    /// having it: `grep` scans subject then path and stops, so a broad
    /// pattern otherwise spends its whole budget in one folder.
    #[test]
    fn categories_narrow_the_scan() {
        let rows = || {
            vec![
                filed("courses/COMP30026/ed/0001-teams.md", "ed"),
                filed("courses/COMP30026/announcements/2026-07-14-welcome.md", "announcement"),
                filed("courses/COMP30026/files/week-01.pdf", "file"),
            ]
        };

        let kept = filter_categories(rows(), &["ed".into()]).ok().unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].category.as_deref(), Some("ed"));

        // Repeatable, and case is not the caller's problem.
        let pair = filter_categories(rows(), &["ED".into(), "announcement".into()]).ok().unwrap();
        assert_eq!(pair.len(), 2);

        // No flag is every category, not none.
        assert_eq!(filter_categories(rows(), &[]).ok().unwrap().len(), 3);
    }

    /// The split that makes the refusal worth having: a word that is not a
    /// category is a mistake and must say so, while a real category these
    /// rows happen not to have is a fair question with an empty answer.
    /// Validating against the rows collapses the two, and refuses
    /// `--subject INFO30006 --category quiz` for a subject with no quizzes.
    #[test]
    fn a_typo_is_refused_but_an_honest_miss_is_empty() {
        let rows = || vec![filed("courses/COMP30026/ed/0001-teams.md", "ed")];

        let err = filter_categories(rows(), &["eds".into()]).err().unwrap();
        assert!(err.contains("no category \"eds\""), "{err}");
        // The refusal names the real ones, from the one list that defines them.
        assert!(err.contains("announcement"), "{err}");
        assert!(err.contains("quiz"), "{err}");

        // Real category, none in these rows: empty, not an error.
        assert!(filter_categories(rows(), &["quiz".into()]).ok().unwrap().is_empty());

        // And an empty row set does not excuse the typo — the early return
        // that used to sit here let `-s <unsynced> -c nonsense` exit 0.
        assert!(filter_categories(vec![], &["eds".into()]).is_err());
        assert!(filter_categories(vec![], &["quiz".into()]).ok().unwrap().is_empty());
    }

    /// Both commands spell the flag the same way, so they must accept the
    /// same words and offer the same list.
    #[test]
    fn the_category_help_lists_what_the_flag_accepts() {
        let help = category_help();
        for c in paths::CATEGORIES {
            assert!(help.contains(c), "{c:?} missing from {help:?}");
        }
    }
}

// ── Subject filtering ────────────────────────────────────────────────────────

/// `MULT20015` matches `MULT20015_2026_SM2`. Canvas codes carry a term suffix
/// nobody wants to type.
fn matches_code(code: &str, wanted: &str) -> bool {
    let (code, wanted) = (code.to_uppercase(), wanted.to_uppercase());
    code == wanted || code.starts_with(&format!("{wanted}_"))
}

/// Narrow a file list to the categories asked for, and **refuse a category
/// that is not one** rather than returning nothing — the same trade `oculus
/// task add` makes with an unknown column id. A silent empty result from a
/// typo reads exactly like "the library does not cover that", which is the
/// one answer a search must never give by accident.
///
/// Validity is `paths::CATEGORIES`, not the categories these particular rows
/// happen to carry. Reading the set off the rows looks tighter and is the
/// wrong question: `--subject INFO30006 --category quiz` would then be
/// refused for a subject that simply has no quizzes, which is a well-formed
/// query and deserves an empty answer, not an error. What a typo and an
/// honest miss have in common is that both return nothing; what separates
/// them is whether the word is a category at all, and only the canonical
/// list knows that.
///
/// Shared by `grep` and `files` so the same flag spelled the same way on two
/// sibling commands cannot disagree about what it accepts.
fn filter_categories(files: Vec<LibFile>, wanted: &[String]) -> Result<Vec<LibFile>, String> {
    if wanted.is_empty() {
        return Ok(files);
    }
    for c in wanted {
        if !paths::CATEGORIES.iter().any(|k| k.eq_ignore_ascii_case(c)) {
            return Err(format!(
                "no category {c:?} — the categories are: {}",
                paths::CATEGORIES.join(", ")
            ));
        }
    }
    Ok(files
        .into_iter()
        .filter(|f| {
            f.category
                .as_deref()
                .is_some_and(|k| wanted.iter().any(|c| k.eq_ignore_ascii_case(c)))
        })
        .collect())
}

/// The `--category` help on both commands, so the list a reader is offered is
/// the list the flag validates against.
fn category_help() -> String {
    format!("Only these categories ({}). Repeatable", paths::CATEGORIES.join(", "))
}

fn filter_subjects(
    subjects: &[store::SubjectRow],
    codes: &[String],
    current_only_when_empty: bool,
) -> Result<Vec<store::SubjectRow>, String> {
    let picked: Vec<store::SubjectRow> = subjects
        .iter()
        .filter(|s| {
            if codes.is_empty() {
                !current_only_when_empty || s.is_current
            } else {
                codes.iter().any(|w| matches_code(&s.code, w))
            }
        })
        .cloned()
        .collect();

    if picked.is_empty() && !codes.is_empty() {
        return Err(format!("no subject matched {}", codes.join(", ")));
    }
    Ok(picked)
}

// ── Planning helpers ─────────────────────────────────────────────────────────

/// Rows this binary writes are the agent's, not the board's — `source` is what
/// lets the app show which cards it did not put there itself.
const AGENT_SOURCE: &str = "agent";

/// A nullable text field on a patch: absent leaves it alone, `--flag ""`
/// clears it. One convention for every patch flag here, so an agent never has
/// to guess how to take a date off.
fn nullable_text(value: Option<&String>) -> Option<Option<String>> {
    value.map(|v| Some(v.clone()).filter(|v| !v.trim().is_empty()))
}

/// The same for a date, which is checked but never rewritten: the library
/// stores every timestamp exactly as its source gave it, zone included.
fn nullable_date(value: Option<&String>) -> Result<Option<Option<String>>, String> {
    match value {
        None => Ok(None),
        Some(v) if v.trim().is_empty() => Ok(Some(None)),
        Some(v) => Ok(Some(Some(projects::check_iso8601(v)?))),
    }
}

/// A comma-separated `--tags` list. Splitting is all this does — trimming,
/// deduplication and the cap are `projects::normalise_tags`'s, so the CLI and
/// the About page cannot drift on what a tag list is.
fn split_tags(value: Option<&str>) -> Vec<String> {
    value
        .map(|v| v.split(',').map(str::to_string).collect())
        .unwrap_or_default()
}

fn nullable_minutes(value: Option<&String>) -> Result<Option<Option<i64>>, String> {
    match value {
        None => Ok(None),
        Some(v) if v.trim().is_empty() => Ok(Some(None)),
        Some(v) => v
            .trim()
            .parse::<i64>()
            .map(|n| Some(Some(n)))
            .map_err(|_| format!("--estimate takes whole minutes, or \"\" to clear (got {v:?})")),
    }
}

/// Print tasks under their columns, in the board's own order.
///
/// The column headings carry the column **id** as well as its name, because
/// the id is what every `--column` takes and the name is the user's to change.
/// A subtask is printed under its parent when they share a column and
/// indented on its own when they do not — a `--column` listing must not
/// silently drop rows.
/// `HH:MM:SS`, so an offset can be read off against a player's own readout.
/// Normalized harness events as terminal lines.
///
/// One printer for both callers of `harness::run_once`: `oculus agent`, where
/// the answer is the point, and `oculus lecture chapters`, where the answer is
/// JSON this binary parses and prints properly itself — hence `show_text`.
/// Everything else is the same, because "what is the agent doing" looks the
/// same whatever it was asked.
struct AgentPrinter {
    show_text: bool,
    /// Whether the cursor is mid-line inside streamed text, so the next
    /// non-text line breaks before it rather than running on.
    mid: Mutex<bool>,
}

impl AgentPrinter {
    fn new(show_text: bool) -> Self {
        AgentPrinter {
            show_text,
            mid: Mutex::new(false),
        }
    }

    fn print(&self, ev: &app_lib::harness::HarnessEvent) {
        use app_lib::harness::HarnessEvent;
        let mut out = std::io::stdout();
        let mut mid = self.mid.lock().unwrap();
        let end_line = |mid: &mut bool, out: &mut std::io::Stdout| {
            if *mid {
                let _ = writeln!(out);
                *mid = false;
            }
        };
        match ev {
            HarnessEvent::SessionStarted { provider_session_id, model, cwd } => {
                let _ = writeln!(
                    out,
                    "{} session {provider_session_id}{} in {cwd}",
                    paint("·", DIM),
                    model.as_ref().map(|m| format!(" ({m})")).unwrap_or_default()
                );
            }
            HarnessEvent::AssistantDelta { text } => {
                if self.show_text {
                    let _ = write!(out, "{text}");
                    *mid = true;
                }
            }
            HarnessEvent::AssistantMessage { text } => {
                if self.show_text {
                    end_line(&mut mid, &mut out);
                } else {
                    let _ = writeln!(
                        out,
                        "{}",
                        paint(&format!("  · replied ({} chars)", text.chars().count()), DIM)
                    );
                }
            }
            HarnessEvent::Thinking { text } => {
                end_line(&mut mid, &mut out);
                let first = text.lines().next().unwrap_or("");
                let _ = writeln!(out, "{}", paint(&format!("  thinking: {first}"), DIM));
            }
            HarnessEvent::ToolStarted { kind, title, .. } => {
                end_line(&mut mid, &mut out);
                let _ = writeln!(out, "{}", paint(&format!("  ▸ {kind:?}: {title}"), DIM));
            }
            HarnessEvent::ToolFinished { ok, output, .. } => {
                let first = output.lines().next().unwrap_or("");
                let mark = if *ok { "✓" } else { "✗" };
                let _ = writeln!(out, "{}", paint(&format!("    {mark} {first}"), DIM));
            }
            HarnessEvent::Usage { context_tokens, cost_usd, .. } => {
                end_line(&mut mid, &mut out);
                let mut s = String::from("  usage:");
                if let Some(c) = context_tokens {
                    s.push_str(&format!(" {c} context tokens"));
                }
                if let Some(c) = cost_usd {
                    s.push_str(&format!(", ${c:.3}"));
                }
                let _ = writeln!(out, "{}", paint(&s, DIM));
            }
            HarnessEvent::RateLimits { windows } => {
                let parts: Vec<String> = windows
                    .iter()
                    .map(|w| format!("{} {:.0}%", w.label, w.used_percent))
                    .collect();
                let _ = writeln!(out, "{}", paint(&format!("  limits: {}", parts.join(", ")), DIM));
            }
            HarnessEvent::Error { message, .. } => {
                end_line(&mut mid, &mut out);
                let _ = writeln!(out, "{} {message}", paint("error:", RED));
            }
            HarnessEvent::TurnFinished { status } => {
                end_line(&mut mid, &mut out);
                let _ = writeln!(out, "{}", paint(&format!("· turn {status}"), DIM));
            }
            _ => {}
        }
        let _ = out.flush();
    }
}

fn clock(secs: u32) -> String {
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
}

fn print_board(project: &projects::Project, tasks: &[projects::Task]) {
    print_columns(&project.columns, tasks);
}

/// The same, given the columns alone — which is what an **unfiled** task's
/// board is: `projects::default_columns()`, the four ids its `column_id` is
/// checked against, with no project to read them off.
fn print_columns(columns: &[projects::Column], tasks: &[projects::Task]) {
    for column in columns {
        let here: Vec<&projects::Task> =
            tasks.iter().filter(|t| t.column_id == column.id).collect();
        if here.is_empty() {
            continue;
        }
        println!(
            "{} {}",
            paint(&column.name, BOLD),
            paint(&format!("[{}]", column.id), DIM)
        );
        for task in here.iter().filter(|t| t.parent_id.is_none()) {
            print_task_line(task, 0);
            for child in here.iter().filter(|c| c.parent_id == Some(task.id)) {
                print_task_line(child, 1);
            }
        }
        for orphan in here
            .iter()
            .filter(|t| t.parent_id.is_some() && !here.iter().any(|p| Some(p.id) == t.parent_id))
        {
            print_task_line(orphan, 1);
        }
    }
}

fn print_task_line(task: &projects::Task, depth: usize) {
    let mut trail = String::new();
    if let Some(due) = &task.due_at {
        trail.push_str(&format!("  due {due}"));
    }
    if let Some(minutes) = task.estimate_minutes {
        trail.push_str(&format!("  {minutes}m"));
    }
    println!(
        "  {}{} {} {}{}",
        "    ".repeat(depth),
        paint(&format!("{:>4}", task.id), DIM),
        if task.done_at.is_some() { paint("\u{2713}", GREEN) } else { " ".to_string() },
        truncate(&task.title, 54 - depth * 4),
        paint(&trail, DIM)
    );
}

// ── Terminal reporter ────────────────────────────────────────────────────────

/// Prints one line per artifact and remembers them so the caller can write the
/// database in one pass at the end.
struct TermReporter {
    phase: Mutex<String>,
    counter: Mutex<(usize, usize)>,
    written: std::sync::Arc<Mutex<Vec<FileEvent>>>,
    course: Mutex<String>,
}

impl TermReporter {
    fn new() -> Self {
        TermReporter {
            phase: Mutex::new(String::new()),
            counter: Mutex::new((0, 0)),
            written: Default::default(),
            course: Mutex::new(String::new()),
        }
    }
    fn sink(&self) -> std::sync::Arc<Mutex<Vec<FileEvent>>> {
        std::sync::Arc::clone(&self.written)
    }
}

impl Reporter for TermReporter {
    fn progress(&self, p: &Progress) {
        let mut course = self.course.lock().unwrap();
        if *course != p.course {
            *course = p.course.clone();
            println!("{}", paint(&p.course, BOLD));
        }
        *self.phase.lock().unwrap() = p.phase.clone();
        *self.counter.lock().unwrap() = (p.done, p.total);
    }

    fn file(&self, f: &FileEvent) {
        let (done, total) = *self.counter.lock().unwrap();
        let phase = self.phase.lock().unwrap().clone();
        let counter = if phase == "modules" && total > 0 {
            format!("{done}/{total}")
        } else {
            String::new()
        };
        // The course is already the section header, so drop `courses/CODE/`.
        let short = f.relative_path.splitn(3, '/').nth(2).unwrap_or(&f.relative_path);
        // Pad before painting: escape codes count toward a width specifier.
        println!(
            "  {} {}  {short:<52} {}",
            paint(&format!("{phase:<13}"), DIM),
            paint(&format!("{counter:>7}"), DIM),
            paint(&format!("{:>9}", human_bytes(f.size_bytes)), DIM)
        );
        let _ = std::io::stdout().flush();
        self.written.lock().unwrap().push(f.clone());
    }

    fn log(&self, level: &str, course: &str, message: &str) {
        let tag = match level {
            "error" => paint("error", RED),
            "warning" => paint("warn", YELLOW),
            _ => paint("info", DIM),
        };
        eprintln!("  {tag} {course}: {message}");
    }
}

fn human_bytes(n: u64) -> String {
    if n >= 1024 * 1024 {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    } else if n >= 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{n} B")
    }
}

// ── Colour ───────────────────────────────────────────────────────────────────

const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";

fn colour_ok() -> bool {
    static OK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OK.get_or_init(|| std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal())
}

fn paint(s: &str, code: &str) -> String {
    if s.is_empty() || !colour_ok() {
        s.to_string()
    } else {
        format!("{code}{s}\x1b[0m")
    }
}
