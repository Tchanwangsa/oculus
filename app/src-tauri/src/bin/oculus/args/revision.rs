//! Cheat notes, draft questions, quiz sets and saved widget programs.
use crate::*;

#[derive(Subcommand)]
pub(crate) enum CheatAction {
    Add(CheatWriteArgs),
    List(RevisionListArgs),
    Show(RevisionIdArgs),
    Update(CheatUpdateArgs),
    /// Permanently delete an entry; there is no undo
    Rm(RevisionIdArgs),
    /// The cheat sheet's tag registry: name, colour and how many entries carry it
    Tags,
}

#[derive(Args)]
pub(crate) struct RevisionIdArgs {
    pub(crate) id: i64,
}

#[derive(Args, Default)]
pub(crate) struct RevisionListArgs {
    /// Subject code, with unique prefixes accepted
    #[arg(short, long)]
    pub(crate) subject: Option<String>,
    /// Only entries assigned to this topic id
    #[arg(long)]
    pub(crate) topic: Option<i64>,
}

#[derive(Args)]
pub(crate) struct CheatWriteArgs {
    #[arg(long)]
    pub(crate) title: String,
    #[arg(long, conflicts_with = "body_file")]
    pub(crate) body: Option<String>,
    /// UTF-8 Markdown file, or - for stdin
    #[arg(long)]
    pub(crate) body_file: Option<String>,
    /// Subject code; every Revision entry belongs to one subject
    #[arg(short, long)]
    pub(crate) subject: String,
    /// Comma-separated tag names from `oculus cheat tags`; a new name joins the registry
    #[arg(long)]
    pub(crate) tags: Option<String>,
    /// Topic ids, comma-separated; omitted or empty assigns Unsorted
    #[arg(long)]
    pub(crate) topics: Option<String>,
    /// Provenance, e.g. courses/CODE/files/notes.pdf#page=3
    #[arg(long)]
    pub(crate) source: Option<String>,
    #[arg(long, default_value = "manual", value_parser = ["manual", "chat", "file", "lecture", "agent"])]
    pub(crate) source_kind: String,
}

#[derive(Args)]
pub(crate) struct CheatUpdateArgs {
    pub(crate) id: i64,
    #[arg(long)]
    pub(crate) title: Option<String>,
    #[arg(long, conflicts_with = "body_file")]
    pub(crate) body: Option<String>,
    #[arg(long)]
    pub(crate) body_file: Option<String>,
    /// Move the entry to another subject; it returns to that subject's Unsorted
    #[arg(short, long)]
    pub(crate) subject: Option<String>,
    #[arg(long)]
    pub(crate) tags: Option<String>,
    /// Replace topic memberships; an empty string assigns Unsorted
    #[arg(long)]
    pub(crate) topics: Option<String>,
    #[arg(long)]
    pub(crate) source: Option<String>,
}

#[derive(Subcommand)]
pub(crate) enum QuizAction {
    /// Add questions as drafts; --batch reads JSON lines atomically
    Add(QuizAddArgs),
    List(QuizListArgs),
    Show(RevisionIdArgs),
    /// Patch question fields from a JSON object; scheduling state is read-only
    Update(QuizUpdateArgs),
    /// Permanently delete a question and its review history; there is no undo
    Rm(RevisionIdArgs),
    Set {
        #[command(subcommand)]
        action: QuizSetAction,
    },
}

#[derive(Args)]
pub(crate) struct QuizAddArgs {
    /// JSON lines file, or - for stdin. Also accepts a JSON array.
    #[arg(long, conflicts_with_all = ["prompt", "answer", "kind", "level", "options", "tolerance", "rubric", "topic", "topics", "style_ref"])]
    pub(crate) batch: Option<String>,
    /// Subject code, applied to every question added
    #[arg(short, long)]
    pub(crate) subject: String,
    #[arg(long)]
    pub(crate) prompt: Option<String>,
    #[arg(long)]
    pub(crate) answer: Option<String>,
    #[arg(long, value_parser = ["card", "choice", "numeric", "short", "structured"])]
    pub(crate) kind: Option<String>,
    #[arg(long, value_parser = ["easy", "medium", "hard"])]
    pub(crate) level: Option<String>,
    /// JSON string array of choice options; answer is the correct option text
    #[arg(long)]
    pub(crate) options: Option<String>,
    #[arg(long)]
    pub(crate) tolerance: Option<f64>,
    /// JSON array of {point, marks}
    #[arg(long)]
    pub(crate) rubric: Option<String>,
    /// Topic ids, comma-separated; omitted or empty assigns Unsorted
    #[arg(long)]
    pub(crate) topics: Option<String>,
    /// Legacy topic labels (prefer --topics ids)
    #[arg(long)]
    pub(crate) topic: Option<String>,
    #[arg(long)]
    pub(crate) style_ref: Option<String>,
}

#[derive(Args)]
pub(crate) struct QuizListArgs {
    #[command(flatten)]
    pub(crate) scope: RevisionListArgs,
    /// Only active items that are due now
    #[arg(long)]
    pub(crate) due: bool,
    #[arg(long, value_parser = ["easy", "medium", "hard"])]
    pub(crate) level: Option<String>,
    #[arg(long)]
    pub(crate) set: Option<i64>,
}

#[derive(Args)]
pub(crate) struct QuizUpdateArgs {
    pub(crate) id: i64,
    /// JSON object containing replacement values for the fields being edited
    #[arg(
        long,
        conflicts_with = "patch_file",
        required_unless_present = "patch_file"
    )]
    pub(crate) patch: Option<String>,
    #[arg(long)]
    pub(crate) patch_file: Option<String>,
}

#[derive(Subcommand)]
pub(crate) enum QuizSetAction {
    Add(QuizSetAddArgs),
    List(RevisionListArgs),
    Show(RevisionIdArgs),
    /// Patch title, description, subject_id or topic_ids with JSON
    Update(QuizUpdateArgs),
}

#[derive(Args)]
pub(crate) struct QuizSetAddArgs {
    #[arg(long)]
    pub(crate) title: String,
    /// Subject code
    #[arg(short, long)]
    pub(crate) subject: String,
    #[arg(long, default_value = "")]
    pub(crate) description: String,
    /// Topic ids, comma-separated; omitted or empty assigns Unsorted
    #[arg(long)]
    pub(crate) topics: Option<String>,
    /// Ordered question ids, comma-separated
    #[arg(long, value_delimiter = ',')]
    pub(crate) items: Vec<i64>,
}

#[derive(Subcommand)]
pub(crate) enum ArtifactAction {
    Save(ArtifactSaveArgs),
    /// Patch title, program, state, subject_id, source_ref or topic_ids with JSON
    Update(QuizUpdateArgs),
    List(RevisionListArgs),
    Show(RevisionIdArgs),
    /// Permanently delete a saved artifact; there is no undo
    Rm(RevisionIdArgs),
}

#[derive(Args)]
pub(crate) struct ArtifactSaveArgs {
    #[arg(long)]
    pub(crate) title: String,
    /// OpenUI Lang source file, or - for stdin
    #[arg(long)]
    pub(crate) program_file: String,
    /// Subject code
    #[arg(short, long)]
    pub(crate) subject: String,
    /// Topic ids, comma-separated; omitted or empty assigns Unsorted
    #[arg(long)]
    pub(crate) topics: Option<String>,
    /// JSON widget state; omitted means initial state
    #[arg(long)]
    pub(crate) state: Option<String>,
    #[arg(long)]
    pub(crate) source: Option<String>,
}

#[derive(Subcommand)]
pub(crate) enum TopicAction {
    /// Create a shared Revision bucket; existing names in the same scope are reused
    Add(TopicAddArgs),
    List(TopicListArgs),
    Show(RevisionIdArgs),
    /// Patch name, description or position with JSON
    Update(QuizUpdateArgs),
    /// Remove a topic; entries without another topic return to Unsorted
    Rm(RevisionIdArgs),
}

#[derive(Args)]
pub(crate) struct TopicAddArgs {
    #[arg(long)]
    pub(crate) name: String,
    /// Subject code; a topic belongs to one subject
    #[arg(short, long)]
    pub(crate) subject: String,
    #[arg(long, default_value = "")]
    pub(crate) description: String,
}

#[derive(Args)]
pub(crate) struct TopicListArgs {
    /// Subject code, with unique prefixes accepted
    #[arg(short, long)]
    pub(crate) subject: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn revision_topics_are_normal_creation_and_edit_fields() {
        for args in [
            vec![
                "topic",
                "add",
                "--name",
                "Differentiation",
                "--subject",
                "MATH",
            ],
            vec!["topic", "list", "--subject", "MATH"],
            vec![
                "topic",
                "update",
                "1",
                "--patch",
                r#"{"name":"Derivatives"}"#,
            ],
            vec!["cheat", "update", "1", "--topics", ""],
            vec!["cheat", "list", "--topic", "2"],
            vec![
                "quiz",
                "add",
                "--subject",
                "MATH",
                "--prompt",
                "Front",
                "--answer",
                "Back",
                "--topics",
                "1,2",
            ],
            vec!["quiz", "list", "--topic", "2"],
            vec![
                "quiz",
                "set",
                "add",
                "--subject",
                "MATH",
                "--title",
                "Week 1",
                "--topics",
                "2",
                "--items",
                "1,2",
            ],
            vec![
                "quiz",
                "set",
                "update",
                "1",
                "--patch",
                r#"{"topic_ids":[2]}"#,
            ],
            vec![
                "artifact",
                "update",
                "1",
                "--patch",
                r#"{"title":"Explorer","topic_ids":[2]}"#,
            ],
        ] {
            Cli::try_parse_from(std::iter::once("oculus").chain(args)).unwrap();
        }
    }

    #[test]
    fn revision_writes_name_a_subject() {
        for args in [
            vec!["cheat", "add", "--title", "Loose"],
            vec!["quiz", "add", "--prompt", "Front", "--answer", "Back"],
            vec!["quiz", "set", "add", "--title", "Week 1"],
            vec!["artifact", "save", "--title", "A", "--program-file", "-"],
            vec!["topic", "add", "--name", "Loose"],
            vec!["cheat", "list", "--general"],
            vec!["topic", "list", "--general"],
        ] {
            assert!(Cli::try_parse_from(std::iter::once("oculus").chain(args)).is_err());
        }
    }
}
