use crate::library::migrate::{dry_run, run, Hooks, Outcome};
use crate::test_support::Scratch;

use super::fixture::{exists, file};

#[test]
fn a_library_already_in_the_tree_only_gets_the_marker() {
    let d = Scratch::new("migrate-fresh");
    file(&d, "courses/X/Course/Files/a.pdf", "pdf");
    std::fs::create_dir_all(d.join("courses/X/.claude/.cc-writes")).unwrap();
    assert!(dry_run(&d).unwrap().is_none());
    assert!(matches!(
        run(&d, &Hooks::default()).unwrap(),
        Outcome::NothingToMove
    ));
    assert!(exists(&d, ".library-move/done"));
    assert!(!exists(&d, ".library-move/journal.json"));
    // An agent's scratch in a new library is not the move's to delete.
    assert!(exists(&d, "courses/X/.claude/.cc-writes"));
    assert!(matches!(
        run(&d, &Hooks::default()).unwrap(),
        Outcome::AlreadyDone
    ));
}

#[test]
fn an_empty_data_dir_is_new() {
    let d = Scratch::new("migrate-empty");
    assert!(matches!(
        run(&d, &Hooks::default()).unwrap(),
        Outcome::NothingToMove
    ));
}
