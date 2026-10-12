use super::*;

#[test]
fn finds_typos() {
    let keys = ["padding", "paddingTop", "fill", "flexDirection"];
    assert_eq!(closest("pading", keys), Some("padding"));
    assert_eq!(closest("flexdirection", keys), Some("flexDirection"));
    assert_eq!(closest("backgroundColor", keys), None);
}

#[test]
fn distance_counts_edits() {
    assert_eq!(distance("kitten", "sitting"), 3);
    assert_eq!(distance("", "abc"), 3);
}

#[test]
fn nearest_has_no_limit() {
    let said = ["get them back", "ninety one"];
    assert_eq!(closest("fetching it back", said), None);
    assert_eq!(nearest("fetching it back", said), Some("get them back"));
    assert_eq!(nearest("x", []), None);
}
