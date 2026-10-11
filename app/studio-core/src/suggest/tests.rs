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
