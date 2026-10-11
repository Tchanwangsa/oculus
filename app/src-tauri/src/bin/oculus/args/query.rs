//! Arguments of the read-only query commands; nothing here scrapes, parses or
//! writes. The `--help` text is an agent's whole documentation, so it says what
//! each command needs and costs.

use crate::*;

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
