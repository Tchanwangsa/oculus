use crate::library::layout::course_rel;

/// The cases the TypeScript mirror (`subjectRel`) is checked against too.
const FIXTURE: &str = include_str!("../../../../../tests/fixtures/library/subject_dirs.json");

#[test]
fn the_shared_fixture_holds() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let code = case[0].as_str().unwrap();
        assert_eq!(course_rel(code), case[1].as_str().unwrap(), "{code}");
    }
}
