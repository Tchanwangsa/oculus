//! The clap command tree: every subcommand, flag and its `--help` text.

use super::*;

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
    /// A manual sign-in: it skips the wait between automatic attempts, and
    /// success resumes automatic sign-in after a lockout or a rejected
    /// password paused it.
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

// ── Read-only query commands ─────────────────────────────────────────────────
// The read-only query surface; nothing here scrapes, parses or writes. The
// `--help` text is an agent's whole documentation, so it says what each
// command needs and costs.

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
pub(crate) struct SearchArgs {
    /// What to look for, in plain language
    #[arg(value_name = "QUERY")]
    pub(crate) query: String,
    /// Restrict to one subject; prefix codes are fine (MULT20015)
    #[arg(short = 's', long, value_name = "SUBJECT_CODE")]
    pub(crate) subject: Option<String>,
    /// How many pages to return (1-50)
    #[arg(short = 'n', long, default_value_t = 8)]
    pub(crate) limit: i64,
    /// Print each hit's whole page instead of a one-line snippet
    #[arg(long)]
    pub(crate) full: bool,
}

/// Search the library by pattern (offline, no model).
///
/// Covers both halves of the library: the markdown on disk (Canvas pages,
/// announcements, assignments, Ed threads) and the page text of PDFs and
/// spreadsheets, which lives in the database — ripgrep over the library
/// directory cannot see it, which is why this exists.
///
/// Needs no network and no model, so it is the fallback whenever `oculus
/// search` cannot run. The pattern is a regular expression by default and
/// case-insensitive unless you ask otherwise.
#[derive(Args)]
pub(crate) struct GrepArgs {
    /// Regular expression to look for
    #[arg(value_name = "PATTERN")]
    pub(crate) pattern: String,
    /// Restrict to these subjects; prefix codes are fine. Repeatable.
    #[arg(short = 's', long, value_name = "SUBJECT_CODE")]
    pub(crate) subject: Vec<String>,
    #[arg(short = 'c', long, value_name = "CATEGORY", help = category_help())]
    pub(crate) category: Vec<String>,
    /// Treat the pattern as literal text, not a regular expression
    #[arg(short = 'F', long)]
    pub(crate) fixed: bool,
    /// Match case exactly
    #[arg(long)]
    pub(crate) case_sensitive: bool,
    /// Print matching file paths only, one per line
    #[arg(short = 'l', long)]
    pub(crate) files_with_matches: bool,
    /// Stop after this many matches
    #[arg(short = 'n', long, default_value_t = 40)]
    pub(crate) limit: usize,
}

/// Print the text of one library file.
///
/// For a PDF or Office document this is the parsed page markdown from the
/// database, so `--pages` addresses the same page numbers `oculus search`
/// and the app's viewer report. A spreadsheet's text is one page per sheet,
/// in workbook order. For markdown and other text it is the file on disk. A PDF that has never been parsed says so rather than printing
/// nothing — run `oculus index <SUBJECT_CODE>` for it.
///
/// FILE may be a full library path, a bare filename, or any distinctive
/// fragment of either. An ambiguous fragment lists the candidates instead of
/// guessing.
#[derive(Args)]
pub(crate) struct ReadArgs {
    /// Library path, filename, or a fragment of either
    #[arg(value_name = "FILE")]
    pub(crate) file: String,
    /// Pages to print: 12, 12-15, 12,14,20-22, or 30- for "30 to the end"
    #[arg(short = 'p', long, value_name = "RANGE")]
    pub(crate) pages: Option<String>,
    /// Disambiguate by subject; prefix codes are fine
    #[arg(short = 's', long, value_name = "SUBJECT_CODE")]
    pub(crate) subject: Option<String>,
}

/// List the files in the library.
///
/// The `indexed` column is how many pages of a document are searchable; a
/// PDF showing none has not been parsed yet.
#[derive(Args)]
pub(crate) struct FilesArgs {
    /// Subjects to list. Omit for every subject.
    #[arg(value_name = "SUBJECT_CODE")]
    pub(crate) codes: Vec<String>,
    /// Only this extension (pdf, md, pptx, docx, png …)
    #[arg(short = 't', long, value_name = "EXT")]
    pub(crate) r#type: Option<String>,
    #[arg(short = 'c', long, value_name = "CATEGORY", help = category_help())]
    pub(crate) category: Vec<String>,
    /// Only paths containing this text (case-insensitive)
    #[arg(short = 'm', long, value_name = "TEXT")]
    pub(crate) r#match: Option<String>,
    /// Only files with pages in the retrieval index
    #[arg(long)]
    pub(crate) indexed: bool,
    /// Stop after this many files
    #[arg(short = 'n', long, default_value_t = 200)]
    pub(crate) limit: usize,
}

/// Class times and assignment due dates.
///
/// Sourced from each subject's Canvas calendar, refreshed by `oculus run -s`.
/// Times are shown in this machine's local timezone; `--json` also carries
/// the raw UTC timestamp.
#[derive(Args)]
pub(crate) struct CalendarArgs {
    /// Subjects to include. Omit for every subject.
    #[arg(value_name = "SUBJECT_CODE")]
    pub(crate) codes: Vec<String>,
    /// How far ahead to look
    #[arg(short = 'd', long, default_value_t = 14, value_name = "DAYS")]
    pub(crate) days: i64,
    /// Only assignment due dates, not class times
    #[arg(long)]
    pub(crate) due: bool,
    /// Include events that have already happened
    #[arg(long)]
    pub(crate) past: bool,
}

// ── Planning: projects and tasks ─────────────────────────────────────────────
// The write half of the agent's surface, through the database the app's
// board reads live.

#[derive(Subcommand)]
pub(crate) enum ProjectAction {
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
pub(crate) struct ProjectListArgs {
    /// Only this subject's projects; prefix codes are fine (COMP30026)
    #[arg(short = 's', long, value_name = "SUBJECT_CODE")]
    pub(crate) subject: Option<String>,
    /// Only projects belonging to no subject
    #[arg(long, conflicts_with = "subject")]
    pub(crate) personal: bool,
    /// Archived projects instead of active ones
    #[arg(long)]
    pub(crate) archived: bool,
}

/// Show one project: its brief, its board, and every task on it.
///
/// Tasks are printed under their column in the board's own order, subtasks
/// indented under their parent. The bracketed name after each column heading
/// is the column **id** — that is what `--column` takes.
#[derive(Args)]
pub(crate) struct ProjectShowArgs {
    /// Project id, as `oculus project list` prints it
    #[arg(value_name = "ID")]
    pub(crate) id: i64,
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
pub(crate) struct ProjectCreateArgs {
    /// What the project is called
    #[arg(value_name = "NAME")]
    pub(crate) name: String,
    /// Scope it to a subject; prefix codes are fine (COMP30026), and the
    /// current term wins a tie. Omit for a personal project.
    #[arg(short = 's', long, value_name = "SUBJECT_CODE")]
    pub(crate) subject: Option<String>,
    /// When the whole thing is due, ISO 8601 (2026-09-20T23:59:00Z)
    #[arg(long, value_name = "ISO")]
    pub(crate) due: Option<String>,
    /// When work on it starts, ISO 8601
    #[arg(long, value_name = "ISO")]
    pub(crate) starts: Option<String>,
    /// A paragraph of what it is — the assignment brief, the plan
    #[arg(long, value_name = "TEXT")]
    pub(crate) brief: Option<String>,
    /// Comma-separated labels for the About page (report,group,week-5)
    #[arg(long, value_name = "TAGS")]
    pub(crate) tags: Option<String>,
}

/// Change a project's name, dates, brief, tags or status.
///
/// Only the flags you pass are written; everything else is left alone. Pass an
/// **empty string** to clear a field: `--due ""` takes the due date off.
///
/// `--status archived` is how a project leaves the board without being
/// deleted; its tasks stay and `--status active` brings it back.
#[derive(Args)]
pub(crate) struct ProjectUpdateArgs {
    /// Project id
    #[arg(value_name = "ID")]
    pub(crate) id: i64,
    /// Rename it
    #[arg(long, value_name = "NAME")]
    pub(crate) name: Option<String>,
    /// Due date, ISO 8601, or "" to clear
    #[arg(long, value_name = "ISO")]
    pub(crate) due: Option<String>,
    /// Start date, ISO 8601, or "" to clear
    #[arg(long, value_name = "ISO")]
    pub(crate) starts: Option<String>,
    /// Replace the brief, or "" to clear it
    #[arg(long, value_name = "TEXT")]
    pub(crate) brief: Option<String>,
    /// active or archived
    #[arg(long, value_parser = ["active", "archived"])]
    pub(crate) status: Option<String>,
    /// Replace every tag with this comma-separated list, or "" to clear them.
    /// There is no add/remove: the whole set is written at once, the same way
    /// the About page's editor writes it.
    #[arg(long, value_name = "TAGS")]
    pub(crate) tags: Option<String>,
}

#[derive(Subcommand)]
pub(crate) enum TaskAction {
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
pub(crate) struct TaskListArgs {
    /// Which project (omit for every task in the library)
    #[arg(short = 'p', long, value_name = "ID")]
    pub(crate) project: Option<i64>,
    /// Only tasks that belong to no project at all
    #[arg(long, conflicts_with = "project")]
    pub(crate) unfiled: bool,
    /// Only this board column (its id, e.g. todo) — needs --project, since a
    /// column id only means something against one board
    #[arg(short = 'c', long, value_name = "ID", requires = "project")]
    pub(crate) column: Option<String>,
    /// Only tasks due before this ISO 8601 timestamp. Compared as text, so
    /// pass the same shape the dates were written in (UTC, usually).
    #[arg(long, value_name = "ISO")]
    pub(crate) due_before: Option<String>,
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
pub(crate) struct TaskAddArgs {
    /// Which project (omit to file it nowhere)
    #[arg(short = 'p', long, value_name = "ID")]
    pub(crate) project: Option<i64>,
    /// The task's title. Omit when using --batch.
    #[arg(value_name = "TITLE")]
    pub(crate) title: Option<String>,
    /// Board column id (default: the first column of its board)
    #[arg(short = 'c', long, value_name = "ID")]
    pub(crate) column: Option<String>,
    /// Make this a subtask of that task id
    #[arg(long, value_name = "TASK_ID")]
    pub(crate) parent: Option<i64>,
    /// Due date, ISO 8601
    #[arg(long, value_name = "ISO")]
    pub(crate) due: Option<String>,
    /// Start date, ISO 8601
    #[arg(long, value_name = "ISO")]
    pub(crate) starts: Option<String>,
    /// How long you think it will take, in minutes
    #[arg(long, value_name = "MIN")]
    pub(crate) estimate: Option<i64>,
    /// Notes on the task
    #[arg(long, value_name = "TEXT")]
    pub(crate) body: Option<String>,
    /// Read a JSON array of tasks from stdin (-) or a file
    #[arg(long, value_name = "FILE", conflicts_with_all = ["title", "column", "parent", "due", "starts", "estimate", "body"])]
    pub(crate) batch: Option<String>,
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
pub(crate) struct TaskUpdateArgs {
    /// Task id
    #[arg(value_name = "ID")]
    pub(crate) id: i64,
    /// Rename it
    #[arg(long, value_name = "TEXT")]
    pub(crate) title: Option<String>,
    /// Replace the notes, or "" to clear
    #[arg(long, value_name = "TEXT")]
    pub(crate) body: Option<String>,
    /// Due date, ISO 8601, or "" to clear
    #[arg(long, value_name = "ISO")]
    pub(crate) due: Option<String>,
    /// Start date, ISO 8601, or "" to clear
    #[arg(long, value_name = "ISO")]
    pub(crate) starts: Option<String>,
    /// Minutes, or "" to clear
    #[arg(long, value_name = "MIN")]
    pub(crate) estimate: Option<String>,
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
pub(crate) struct TaskMoveArgs {
    /// Task id
    #[arg(value_name = "ID")]
    pub(crate) id: i64,
    /// Destination column id (e.g. done)
    #[arg(short = 'c', long, value_name = "ID")]
    pub(crate) column: String,
    /// Put it directly below this task
    #[arg(long, value_name = "TASK_ID")]
    pub(crate) after: Option<i64>,
    /// Put it directly above this task
    #[arg(long, value_name = "TASK_ID")]
    pub(crate) before: Option<i64>,
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
pub(crate) struct TaskRefileArgs {
    /// Task id
    #[arg(value_name = "ID")]
    pub(crate) id: i64,
    /// File it under this project
    #[arg(short = 'p', long, value_name = "ID")]
    pub(crate) project: Option<i64>,
    /// Take it out of every project instead
    #[arg(long, conflicts_with = "project")]
    pub(crate) unfiled: bool,
}

/// Delete a task, and its subtasks with it.
///
/// There is no undo, and nothing else cleans these up — a task that is merely
/// finished belongs in a `done` column (`oculus task move`), not deleted.
#[derive(Args)]
pub(crate) struct TaskRmArgs {
    /// Task id
    #[arg(value_name = "ID")]
    pub(crate) id: i64,
}

// ── Lectures: what is inside a recording ───────────────────────────────────
// Decodes a recording already on disk; no database, no network.

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

// ── transcribe ──────────────────────────────────────────────────────────────

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

// ── memory ──────────────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub(crate) enum MemoryAction {
    List(MemoryListArgs),
    Read(MemoryReadArgs),
    Write(MemoryWriteArgs),
    Rm(MemoryRmArgs),
    Move(MemoryMoveArgs),
    Reindex(MemoryReindexArgs),
    Promote(MemoryPromoteArgs),
}

/// What is already known, as the index shows it.
///
/// Read this before answering, not after: it is a few hundred bytes and it is
/// the only thing that carries between conversations. Without `-s` it lists
/// the cross-subject bucket; `--all` walks every bucket there is.
///
/// Each line leads with the **name**, which is what `read`, `rm`, `move` and
/// `--link` take, and what a second `write` under updates rather than
/// duplicates. Its one-line description follows underneath.
#[derive(Args)]
pub(crate) struct MemoryListArgs {
    /// Only this subject's memories (a code, e.g. INFO30006)
    #[arg(short = 's', long, value_name = "CODE")]
    pub(crate) subject: Option<String>,
    /// Every bucket: across subjects, then one per course
    #[arg(long, conflicts_with = "subject")]
    pub(crate) all: bool,
    /// Only memories of this type
    #[arg(long, value_name = "TYPE", value_parser = app_lib::memory::TYPES)]
    pub(crate) r#type: Option<String>,
}

/// Print one memory in full.
///
/// The name is the file's, without `.md` — `memory list` prints it. A
/// close-enough spelling finds it anyway: the hyphens need not fall where the
/// filename puts them, the title works in place of the name, and a prefix or a
/// distinctive fragment works when only one memory answers to it. Both buckets
/// are searched unless `-s` narrows it.
#[derive(Args)]
pub(crate) struct MemoryReadArgs {
    /// Which memory
    #[arg(value_name = "NAME")]
    pub(crate) name: String,
    /// Look only in this subject's bucket
    #[arg(short = 's', long, value_name = "CODE")]
    pub(crate) subject: Option<String>,
}

/// Write a memory — front matter, dates and index included.
///
/// **This is the way to remember something, rather than writing the markdown
/// by hand.** A memory is two files — the fact, and a line in that folder's
/// `MEMORY.md` — and the second is the one that gets skipped, which is the one
/// that decides whether the next conversation ever opens the first. Here the
/// index is rewritten from the files every time, so it cannot go stale, and
/// the front matter is built from the flags: pass the fact and how it is
/// filed, and `name`, `created`, `updated` and the index are not yours to
/// remember.
///
/// **Filing is `-s` or nothing.** A fact that names one subject takes
/// `-s <CODE>` and lands in that subject's bucket — always, including when it
/// came up in a conversation scoped to nothing. Without `-s` it goes in the
/// cross-subject bucket, which is for the student themselves and for what
/// spans subjects.
///
/// **A name that is already filed is updated, not duplicated**, keeping its
/// `created` date and anything this call leaves out. That is what makes the
/// store a record of what is true rather than a log of what was said.
///
/// The body is one line as `--text`, or a file with `--body` (`-` for stdin) —
/// the shell an in-app agent runs through refuses a newline inside an
/// argument, so anything with paragraphs in it is written to a file first.
///
///     oculus memory write --type reference --about "Ed answers are the marking authority for INFO30006; the brief is not" --text "Staff said in Ed #66 that everything in lectures and tutorials is assessable." -s INFO30006
///
///     oculus memory write tchan-study-workflow --type feedback --about "Wants a verdict then the evidence, never a survey of options" --text "Triages by return on investment and does the arithmetic before asking." --why "He is asking for the missing evidence, not to be told what to do." --how "Lead with one recommendation, then the specific evidence under it."
#[derive(Args)]
pub(crate) struct MemoryWriteArgs {
    /// The file's name. Omit and it is taken from --about — but **name it to
    /// update it**: a write that derives a name from a changed line is a new
    /// memory, the same as it would be for any other file.
    #[arg(value_name = "NAME")]
    pub(crate) name: Option<String>,
    /// File it under this subject (omit for the cross-subject bucket)
    #[arg(short = 's', long, value_name = "CODE")]
    pub(crate) subject: Option<String>,
    /// The one line the index shows — what a reader sees before opening it
    #[arg(long, visible_alias = "description", value_name = "TEXT")]
    pub(crate) about: Option<String>,
    /// What kind of memory this is
    #[arg(long, value_name = "TYPE", value_parser = app_lib::memory::TYPES)]
    pub(crate) r#type: Option<String>,
    /// What the index calls it (default: the name, read back as words)
    #[arg(long, value_name = "TEXT")]
    pub(crate) title: Option<String>,
    /// The fact itself, on one line
    #[arg(long, value_name = "TEXT", conflicts_with = "body")]
    pub(crate) text: Option<String>,
    /// The fact, from a file — or `-` for stdin
    #[arg(long, value_name = "FILE")]
    pub(crate) body: Option<String>,
    /// Why it is true, or why it matters. Required for feedback and project.
    #[arg(long, value_name = "TEXT")]
    pub(crate) why: Option<String>,
    /// What a later session should do about it. Required for feedback and project.
    #[arg(long, value_name = "TEXT")]
    pub(crate) how: Option<String>,
    /// Where the fact came from — a library path, an Ed number, a lecture
    #[arg(long, value_name = "TEXT")]
    pub(crate) source: Option<String>,
    /// Another memory this one bears on, by name. Repeatable.
    #[arg(long, value_name = "NAME")]
    pub(crate) link: Vec<String>,
    /// Any other front-matter field, as key=value. Repeatable.
    #[arg(long, value_name = "K=V")]
    pub(crate) meta: Vec<String>,
}

/// Delete a memory that turned out to be wrong.
///
/// The store is what is true, not what was said, so a fact that has been
/// overtaken is deleted rather than left to be read again. **There is no
/// undo** — nothing upstream has a copy of this, the way a course file comes
/// back on the next sync. The index is rewritten without it.
#[derive(Args)]
pub(crate) struct MemoryRmArgs {
    /// Which memory
    #[arg(value_name = "NAME")]
    pub(crate) name: String,
    /// Look only in this subject's bucket
    #[arg(short = 's', long, value_name = "CODE")]
    pub(crate) subject: Option<String>,
}

/// File a memory under a different bucket.
///
/// For the mistake the two buckets exist to make visible: a fact about one
/// subject that ended up in the cross-subject store, or the reverse. The file
/// moves whole — body, dates and all — and both indexes are rewritten, which
/// is the part that made this worth a command rather than a delete and a
/// retype.
#[derive(Args)]
pub(crate) struct MemoryMoveArgs {
    /// Which memory
    #[arg(value_name = "NAME")]
    pub(crate) name: String,
    /// File it under this subject
    #[arg(short = 's', long, value_name = "CODE", conflicts_with = "global")]
    pub(crate) subject: Option<String>,
    /// File it across subjects instead
    #[arg(long)]
    pub(crate) global: bool,
}

/// Rebuild a bucket's `MEMORY.md` from the files in it.
///
/// Every write does this already, so it is here for the store as it stands
/// today: memories written by hand before this command existed, or a file
/// dropped in from somewhere. Whatever prose sits above the generated list is
/// kept.
#[derive(Args)]
pub(crate) struct MemoryReindexArgs {
    /// Only this subject's bucket
    #[arg(short = 's', long, value_name = "CODE")]
    pub(crate) subject: Option<String>,
    /// Every bucket there is
    #[arg(long, conflicts_with = "subject")]
    pub(crate) all: bool,
}

/// Turn a memory into a standing preference in `TASTE.md`.
///
/// The second half of the rule that file states: the first time something is
/// said about how work should be done it is a `feedback` memory, and when it
/// comes up again it earns a line in `TASTE.md` and the memory goes. This does
/// both ends of that in one call, so the promotion is not a hand-edit nobody
/// makes.
#[derive(Args)]
pub(crate) struct MemoryPromoteArgs {
    /// Which memory
    #[arg(value_name = "NAME")]
    pub(crate) name: String,
    /// Which heading it belongs under
    #[arg(long, value_name = "NAME", value_parser = ["writing", "working", "study"])]
    pub(crate) section: String,
    /// The bullet, written as an instruction (default: the memory's own line)
    #[arg(long = "as", value_name = "TEXT")]
    pub(crate) text: Option<String>,
    /// Leave the memory in place instead of deleting it
    #[arg(long)]
    pub(crate) keep: bool,
    /// Look only in this subject's bucket
    #[arg(short = 's', long, value_name = "CODE")]
    pub(crate) subject: Option<String>,
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
