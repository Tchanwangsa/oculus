use crate::library::layout::link_between;

#[test]
fn a_link_climbs_only_as_far_as_it_must() {
    let cases = [
        (
            "Course/Modules/01-intro.md",
            "Course/Files/a.pdf",
            "../Files/a.pdf",
        ),
        ("Course/home.md", "Course/Images/9.png", "Images/9.png"),
        (
            "Course/Pages/p.md",
            "Course/Images/9.png",
            "../Images/9.png",
        ),
        ("Course/Modules/01.md", "Course/Modules/02.md", "02.md"),
        (
            "Course/Lessons/1-l.md",
            "Course/Lessons/1-l/03/q.py",
            "1-l/03/q.py",
        ),
        ("a/b/c.md", "x.md", "../../x.md"),
    ];
    for (from, to, want) in cases {
        assert_eq!(link_between(from, to), want, "{from} → {to}");
    }
}
