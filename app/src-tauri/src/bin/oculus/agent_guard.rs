//! Student-owned writes are confirmed by chat cards before the app applies them.

use crate::args::*;

pub(super) fn check(command: Option<&Command>, agent: bool) -> Result<(), String> {
    if !agent {
        return Ok(());
    }
    let revision_deletion = matches!(
        command,
        Some(
            Command::Cheat {
                action: CheatAction::Rm(_)
            } | Command::Quiz {
                action: QuizAction::Rm(_)
            } | Command::Artifact {
                action: ArtifactAction::Rm(_)
            } | Command::Topic {
                action: TopicAction::Rm(_)
            }
        )
    );
    if revision_deletion {
        return Err("Revision deletion belongs to the student in Revision or their terminal; propose ordinary field changes with `EditItems`".into());
    }
    let card = match command {
        Some(Command::Auth { .. } | Command::Keyd { .. }) => return Err("authentication and credential broker commands must be run by the student in their terminal".into()),
        Some(Command::Project { action: ProjectAction::Create(_) }) => Some("NewProject"),
        Some(Command::Project { action: ProjectAction::Update(_) }) => Some("ProjectChanges"),
        Some(Command::Task { action }) => match action {
            TaskAction::Add(_) => Some("NewTasks"),
            TaskAction::Update(_) | TaskAction::Move(_) | TaskAction::Refile(_) => Some("TaskChanges"),
            TaskAction::Rm(_) => Some("DeleteTasks"),
            _ => None,
        },
        Some(Command::Tag { action }) if !matches!(action, TagAction::List(_)) => Some("TagChanges"),
        Some(Command::Cheat { action: CheatAction::Add(_) }) => Some("NewCheatNotes"),
        Some(Command::Cheat { action: CheatAction::Update(_) }
            | Command::Quiz { action: QuizAction::Update(_) | QuizAction::Set { action: QuizSetAction::Update(_) } }
            | Command::Artifact { action: ArtifactAction::Update(_) }
            | Command::Topic { action: TopicAction::Update(_) }) => Some("EditItems"),
        Some(Command::Run(_)) => Some("Sync"),
        Some(Command::Index(_)) => Some("Reparse"),
        Some(Command::Transcribe(_)) => Some("Transcribe"),
        Some(Command::Lecture { action: LectureAction::Chapters(_) }) => Some("LectureChapters"),
        Some(Command::Lecture { action: LectureAction::End(_) }) => Some("LectureEnd"),
        Some(Command::Studio { action: StudioAction::Voices { action: Some(StudioVoicesAction::Download(_)) } | StudioAction::Aligner { action: StudioAlignerAction::Download } }) => Some("Download"),
        _ => None,
    };
    match card {
        Some(card) => {
            let skill = if matches!(card, "EditItems" | "NewCheatNotes") {
                "oculus-revision"
            } else {
                "oculus-plan"
            };
            Err(format!(
                "propose this with `{card}` — see the `{skill}` skill"
            ))
        }
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn every_student_write_requires_its_card() {
        for (args, card) in [
            ("project create Test", "NewProject"),
            ("project update 1 --name Test", "ProjectChanges"),
            ("task add Test", "NewTasks"),
            ("task add --batch -", "NewTasks"),
            ("task update 1 --title Test", "TaskChanges"),
            ("task move 1 -c done", "TaskChanges"),
            ("task refile 1 --unfiled", "TaskChanges"),
            ("task rm 1", "DeleteTasks"),
            ("tag add Test", "TagChanges"),
            ("tag rename A B", "TagChanges"),
            ("tag color A indigo", "TagChanges"),
            ("tag rm A", "TagChanges"),
            ("run COMP10001", "Sync"),
            ("index", "Reparse"),
            ("transcribe a.mp4", "Transcribe"),
            ("lecture chapters a", "LectureChapters"),
            ("lecture end a", "LectureEnd"),
            ("studio voices download", "Download"),
            ("studio aligner download", "Download"),
        ] {
            let cli = Cli::try_parse_from(std::iter::once("oculus").chain(args.split_whitespace()))
                .unwrap();
            assert!(
                check(cli.command.as_ref(), true)
                    .unwrap_err()
                    .contains(card),
                "{args}"
            );
            assert!(check(cli.command.as_ref(), false).is_ok());
        }
    }

    #[test]
    fn agents_draft_questions_and_propose_ordinary_revision_edits() {
        for args in [
            vec!["quiz", "update", "1", "--patch", r#"{"status":"active"}"#],
            vec!["quiz", "update", "1", "--patch", r#"{"prompt":"Changed"}"#],
            vec!["quiz", "rm", "1"],
        ] {
            let cli = Cli::try_parse_from(std::iter::once("oculus").chain(args)).unwrap();
            let error = check(cli.command.as_ref(), true).unwrap_err();
            assert!(error.contains("EditItems") || error.contains("student"));
            assert!(check(cli.command.as_ref(), false).is_ok());
        }
        for args in [
            vec!["quiz", "add", "--subject", "MATH", "--batch", "-"],
            vec![
                "quiz",
                "set",
                "add",
                "--subject",
                "MATH",
                "--title",
                "Week 1",
                "--items",
                "1,2",
            ],
            vec!["quiz", "list", "--due"],
            vec!["quiz", "show", "1"],
            vec![
                "topic",
                "add",
                "--subject",
                "MATH",
                "--name",
                "Differentiation",
            ],
            vec!["topic", "list"],
            vec!["topic", "show", "1"],
            vec!["cheat", "list", "--topic", "1"],
            vec!["cheat", "tags"],
        ] {
            let cli = Cli::try_parse_from(std::iter::once("oculus").chain(args)).unwrap();
            assert!(check(cli.command.as_ref(), true).is_ok());
        }
    }

    #[test]
    fn revision_topic_and_content_updates_share_edit_items() {
        for args in [
            vec!["cheat", "update", "1", "--topics", "2"],
            vec!["quiz", "update", "1", "--patch", r#"{"topic_ids":[2]}"#],
            vec![
                "quiz",
                "set",
                "update",
                "1",
                "--patch",
                r#"{"topic_ids":[2]}"#,
            ],
            vec!["artifact", "update", "1", "--patch", r#"{"topic_ids":[2]}"#],
            vec![
                "topic",
                "update",
                "1",
                "--patch",
                r#"{"name":"Derivatives"}"#,
            ],
        ] {
            let cli = Cli::try_parse_from(std::iter::once("oculus").chain(args)).unwrap();
            assert!(check(cli.command.as_ref(), true)
                .unwrap_err()
                .contains("EditItems"));
            assert!(check(cli.command.as_ref(), false).is_ok());
        }
        for command in ["cheat", "quiz", "artifact", "topic"] {
            let cli = Cli::try_parse_from(["oculus", command, "rm", "1"]).unwrap();
            assert!(check(cli.command.as_ref(), true)
                .unwrap_err()
                .contains("student"));
        }
    }

    #[test]
    fn read_and_workspace_commands_remain_available() {
        for args in [
            "status",
            "project list",
            "task list",
            "tag list",
            "calendar",
            "lecture candidates a",
            "studio list",
            "studio voices",
            "docs",
            "memory reindex",
        ] {
            let cli = Cli::try_parse_from(std::iter::once("oculus").chain(args.split_whitespace()))
                .unwrap();
            assert!(check(cli.command.as_ref(), true).is_ok(), "{args}");
        }
        for args in ["auth status", "keyd status"] {
            let cli = Cli::try_parse_from(std::iter::once("oculus").chain(args.split_whitespace()))
                .unwrap();
            assert!(check(cli.command.as_ref(), true).is_err());
        }
    }
}
