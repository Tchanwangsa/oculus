use super::*;
use crate::*;

#[test]
fn markdown_file_input_preserves_newlines_and_clear_semantics() {
    let path = std::env::temp_dir().join(format!("oculus-planning-{}.md", std::process::id()));
    let markdown = "# About\n\n- it's $x$\n- `a\\b`\n\nLast paragraph\n";
    std::fs::write(&path, markdown).unwrap();
    let path = path.to_str().unwrap().to_string();
    for command in [
        vec![
            "oculus",
            "project",
            "create",
            "Assignment",
            "--brief-file",
            &path,
        ],
        vec!["oculus", "project", "update", "7", "--brief-file", &path],
        vec!["oculus", "task", "add", "Draft", "--body-file", &path],
        vec!["oculus", "task", "update", "12", "--body-file", &path],
    ] {
        let cli = Cli::try_parse_from(command).unwrap();
        let (inline, file, field) = match cli.command.unwrap() {
            Command::Project {
                action: ProjectAction::Create(a),
            } => (a.brief, a.brief_file, "brief"),
            Command::Project {
                action: ProjectAction::Update(a),
            } => (a.brief, a.brief_file, "brief"),
            Command::Task {
                action: TaskAction::Add(a),
            } => (a.body, a.body_file, "body"),
            Command::Task {
                action: TaskAction::Update(a),
            } => (a.body, a.body_file, "body"),
            _ => panic!("wrong command"),
        };
        assert_eq!(
            planning_text(inline.as_ref(), file.as_ref(), field)
                .unwrap()
                .as_deref(),
            Some(markdown)
        );
    }
    std::fs::write(&path, "").unwrap();
    let empty = planning_text(None, Some(&path), "body").unwrap();
    assert_eq!(nullable_text(empty.as_ref()), Some(None));
    std::fs::remove_file(&path).unwrap();
    assert!(planning_text(None, Some(&path), "body")
        .unwrap_err()
        .contains("reading"));
}

#[test]
fn conflicting_inputs_are_refused_before_writing() {
    for command in [
        vec![
            "oculus",
            "project",
            "create",
            "A",
            "--brief",
            "inline",
            "--brief-file",
            "-",
        ],
        vec![
            "oculus",
            "project",
            "update",
            "7",
            "--brief",
            "inline",
            "--brief-file",
            "-",
        ],
        vec![
            "oculus",
            "task",
            "add",
            "A",
            "--body",
            "inline",
            "--body-file",
            "-",
        ],
        vec![
            "oculus",
            "task",
            "update",
            "12",
            "--body",
            "inline",
            "--body-file",
            "-",
        ],
        vec!["oculus", "task", "add", "--batch", "-", "--body-file", "-"],
        vec![
            "oculus",
            "project",
            "update",
            "7",
            "--subject",
            "SUBJ",
            "--personal",
        ],
    ] {
        assert!(Cli::try_parse_from(command).is_err());
    }
}
