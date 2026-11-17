use crate::library::migrate::db::lecture_path;
use crate::library::migrate::rules::{Rule, Rules};

fn rules(pairs: &[(&str, &str)]) -> Rules {
    let list: Vec<Rule> = pairs
        .iter()
        .map(|(old, new)| Rule {
            old: old.to_string(),
            new: new.to_string(),
        })
        .collect();
    Rules::new(&list)
}

#[test]
fn the_longest_prefix_wins_at_a_boundary_and_the_rest_carries_over() {
    let r = rules(&[
        ("courses/X/uploads", "courses/X"),
        ("courses/X/uploads/a.pdf", "courses/X/a (2).pdf"),
        ("courses/X/files", "courses/X/Course/Files"),
    ]);
    assert_eq!(
        r.map("courses/X/uploads/b.pdf").as_deref(),
        Some("courses/X/b.pdf")
    );
    assert_eq!(
        r.map("courses/X/uploads/a.pdf#page=3").as_deref(),
        Some("courses/X/a (2).pdf#page=3")
    );
    assert_eq!(
        r.map("courses/X/files/n.md:L15").as_deref(),
        Some("courses/X/Course/Files/n.md:L15")
    );
    assert_eq!(
        r.map("courses/X/files").as_deref(),
        Some("courses/X/Course/Files")
    );
    // Not at a boundary, and not in the old spelling.
    assert_eq!(r.map("courses/X/filesystem.md"), None);
    assert_eq!(
        r.map("courses/X/uploads/a.pdfx").as_deref(),
        Some("courses/X/a.pdfx")
    );
    assert_eq!(r.map("courses/X/Course/Files/a.pdf"), None);
}

#[test]
fn a_stored_lecture_path_is_recognised_by_its_tail_wherever_the_library_was() {
    let data = std::path::Path::new("/now/data");
    let folder = "courses/X/Course/Recordings/2026-03-02-mo-l101";
    assert_eq!(
        lecture_path("/old/home/lectures/L1/source1.mp4", "L1", data, folder).as_deref(),
        Some("/now/data/courses/X/Course/Recordings/2026-03-02-mo-l101/source1.mp4")
    );
    assert_eq!(
        lecture_path("lectures/L1/frames/1.jpg", "L1", data, folder).as_deref(),
        Some("/now/data/courses/X/Course/Recordings/2026-03-02-mo-l101/frames/1.jpg")
    );
    assert_eq!(
        lecture_path("/old/home/lectures/L2/source1.mp4", "L1", data, folder),
        None
    );
    assert_eq!(
        lecture_path(
            &format!("/now/data/{folder}/source1.mp4"),
            "L1",
            data,
            folder
        ),
        None
    );
}
