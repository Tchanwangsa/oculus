//! Arguments of the `memory` commands.

use crate::*;

#[derive(Subcommand)]
pub(crate) enum MemoryAction {
    List(MemoryListArgs),
    Read(MemoryReadArgs),
    Write(MemoryWriteArgs),
    Append(MemoryAppendArgs),
    Fold(MemoryFoldArgs),
    Rm(MemoryRmArgs),
    Move(MemoryMoveArgs),
    Reindex(MemoryReindexArgs),
    Promote(MemoryPromoteArgs),
}

/// What is already known, as the index shows it.
///
/// Read this before answering, not after: it is a few hundred bytes and it is
/// the only thing that carries between conversations. Without `-s` it lists
/// the cross-subject bucket; `--all` walks every bucket there is. Each
/// bucket's heading says how full it is.
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
    #[arg(long, value_name = "TYPE", value_parser = app_lib::agents::memory::TYPES)]
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
/// **A bucket keeps twelve at most.** Past that a new name is refused with the
/// names filed there: the fact is a line in one of them (`memory append`), a
/// rewrite of one, or room made by `memory fold`.
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
    #[arg(long, value_name = "TYPE", value_parser = app_lib::agents::memory::TYPES)]
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

/// Add one line to a memory, or change or remove one.
///
/// For the pages a subject keeps adding to — `gaps`, one assessment's page —
/// so a new fact is a line where it belongs rather than a file of its own. The
/// line becomes a list item. `--replace` swaps the one line containing that
/// text instead (an empty LINE removes it), which is how a gap becomes "got it".
///
///     oculus memory append gaps "Quantum counting: the dummy qubit doubles N; the counting qubits set the precision (L17 p11) — open" -s MULT20015
///
///     oculus memory append gaps "Quantum counting: dummy vs counting qubits — got it 27 Oct" --replace "dummy qubit" -s MULT20015
#[derive(Args)]
pub(crate) struct MemoryAppendArgs {
    /// Which memory, by its exact name
    #[arg(value_name = "NAME")]
    pub(crate) name: String,
    /// The line to add, or the replacement with --replace ("" removes it)
    #[arg(value_name = "LINE")]
    pub(crate) line: String,
    /// Change the one line containing this text instead of adding one
    #[arg(long, value_name = "TEXT")]
    pub(crate) replace: Option<String>,
    /// Look only in this subject's bucket
    #[arg(short = 's', long, value_name = "CODE")]
    pub(crate) subject: Option<String>,
    /// Start the memory with this index line when it is not filed yet
    #[arg(
        long,
        visible_alias = "description",
        value_name = "TEXT",
        requires = "type"
    )]
    pub(crate) about: Option<String>,
    /// Its type, when starting it
    #[arg(long, value_name = "TYPE", value_parser = app_lib::agents::memory::TYPES, requires = "about")]
    pub(crate) r#type: Option<String>,
}

/// Fold several memories into one and delete the rest.
///
/// For a bucket that has become a log — one file per question, per session,
/// per slip — or one that is full. **The body is yours to write**: what holds
/// across them, each episode at most a line of evidence, not the files pasted
/// together. The result keeps the earliest `created` date; `--into` may be one
/// of the sources, which is then rewritten rather than deleted. All of them
/// must be in the one bucket `-s` names.
///
///     oculus memory fold a2-q4c-period-method a2-q4c-split-by-factors a2-q4a-rsa-answer --into assignment-2 --about "A2: what is settled, what is owed" --body /tmp/a2.md -s MULT20015
#[derive(Args)]
pub(crate) struct MemoryFoldArgs {
    /// The memories to fold
    #[arg(value_name = "NAME", required = true, num_args = 1..)]
    pub(crate) names: Vec<String>,
    /// The memory they become — a new name, or one of them
    #[arg(long, value_name = "NAME")]
    pub(crate) into: String,
    /// The bucket they are all in
    #[arg(short = 's', long, value_name = "CODE")]
    pub(crate) subject: Option<String>,
    /// The index line (default: --into's own, when it is filed already)
    #[arg(long, visible_alias = "description", value_name = "TEXT")]
    pub(crate) about: Option<String>,
    /// Its type (default: the one the sources share)
    #[arg(long, value_name = "TYPE", value_parser = app_lib::agents::memory::TYPES)]
    pub(crate) r#type: Option<String>,
    /// What the index calls it
    #[arg(long, value_name = "TEXT")]
    pub(crate) title: Option<String>,
    /// The merged memory, on one line
    #[arg(long, value_name = "TEXT", conflicts_with = "body")]
    pub(crate) text: Option<String>,
    /// The merged memory, from a file — or `-` for stdin
    #[arg(long, value_name = "FILE")]
    pub(crate) body: Option<String>,
    /// Why it is true, or why it matters. Required for feedback and project.
    #[arg(long, value_name = "TEXT")]
    pub(crate) why: Option<String>,
    /// What a later session should do about it. Required for feedback and project.
    #[arg(long, value_name = "TEXT")]
    pub(crate) how: Option<String>,
    /// Where the facts came from
    #[arg(long, value_name = "TEXT")]
    pub(crate) source: Option<String>,
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
