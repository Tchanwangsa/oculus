use super::super::{lint, make, Request, Select};
use super::Setup;
use studio_core::{LintLevel, Project};

#[test]
fn a_changed_pdf_is_a_warning_with_the_command_to_rerun() {
    let s = Setup::new("slide-lint");
    let video = s.video();
    std::fs::write(
        video.join("video.json"),
        r#"{ "size": { "width": 1920, "height": 1080 }, "theme": "default", "scenes": [
            { "id": "fig", "component": "Slide", "props": { "src": "slides/fig.png" }, "hold": 2 }
        ] }"#,
    )
    .unwrap();
    make(&Request {
        video: &video,
        data_dir: s.data(),
        source: s.source,
        page: Some(1),
        select: Select::Find("Figure 3".into()),
        name: Some("fig".into()),
        pad: 0.02,
        width: Some(300),
        replace: false,
        exact: false,
    })
    .unwrap();
    let project = Project::load(&video).unwrap();
    assert!(lint(&project, s.data()).is_empty());

    let mut bytes = std::fs::read(s.pdf()).unwrap();
    bytes.extend_from_slice(b"\n% edited\n");
    std::fs::write(s.pdf(), bytes).unwrap();
    let issues = lint(&project, s.data());
    assert_eq!(issues.len(), 1);
    let issue = &issues[0];
    assert_eq!(issue.level, LintLevel::Warning);
    assert_eq!((issue.scene, issue.id.as_deref()), (Some(0), Some("fig")));
    let message = issue.error.to_string();
    assert!(message.contains("has changed since"), "{message}");
    assert!(
        message.contains("--page 1 --find 'Figure 3' --pad 0.02 --name fig --width 300"),
        "{message}"
    );

    // A PDF that is not there is not checked.
    std::fs::remove_file(s.pdf()).unwrap();
    assert!(lint(&project, s.data()).is_empty());
}
