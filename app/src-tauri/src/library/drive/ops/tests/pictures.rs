use super::*;
use crate::library::drive::ops::moves::move_entries_at;
use crate::library::drive::ops::pictures::linked;

#[test]
fn a_note_s_picture_links_are_found() {
    let text = "![a](.assets/p.png) and ![b](./.assets/q%20r.png)\n<img src=\".assets/s.png\"> x.assets/no.png";
    assert_eq!(
        linked(text).into_iter().collect::<Vec<_>>(),
        ["p.png", "q%20r.png", "s.png"]
    );
}

#[tokio::test]
async fn a_moved_note_keeps_its_pictures() {
    let lib = Lib::new("drive-pictures").await;
    let note = own(CODE, "Week 1/n.md");
    lib.file(&note, b"![](.assets/p.png) ![](.assets/q.png)")
        .await;
    lib.file(&own(CODE, "Week 1/stays.md"), b"![](.assets/q.png)")
        .await;
    lib.put(&own(CODE, "Week 1/.assets/p.png"), b"p");
    lib.put(&own(CODE, "Week 1/.assets/q.png"), b"q");
    lib.put(&own(CODE, "Week 2/.assets/p.png"), b"someone else's p");

    move_entries_at(&lib.dir, &lib.pool, &rels(&[&note]), &own(CODE, "Week 2"))
        .await
        .unwrap();

    assert!(
        !lib.exists(&own(CODE, "Week 1/.assets/p.png")),
        "moved: no one else links it"
    );
    assert_eq!(
        lib.read(&own(CODE, "Week 1/.assets/q.png")),
        "q",
        "copied: stays.md links it"
    );
    assert_eq!(lib.read(&own(CODE, "Week 2/.assets/q.png")), "q");
    assert_eq!(lib.read(&own(CODE, "Week 2/.assets/p-2.png")), "p");
    assert_eq!(
        lib.read(&own(CODE, "Week 2/.assets/p.png")),
        "someone else's p"
    );
    assert_eq!(
        lib.read(&own(CODE, "Week 2/n.md")),
        "![](.assets/p-2.png) ![](.assets/q.png)"
    );
}
