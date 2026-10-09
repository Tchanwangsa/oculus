//! Arguments of the planning commands: projects and tasks, the write half of
//! the agent's surface, through the database the app's board reads live.

use crate::*;

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
