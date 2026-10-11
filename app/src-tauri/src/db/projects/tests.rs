use super::*;

#[test]
fn accepts_the_shapes_the_library_actually_stores() {
    for good in [
        "2026-09-20",
        "2026-09-20T23:59:00Z",
        "2026-09-20T23:59Z",
        "2026-09-20 13:05:00",
        "2026-09-20T13:05:00.250+10:00",
        "2026-09-20T13:05:00+1000",
    ] {
        assert_eq!(check_iso8601(good).unwrap(), good, "{good}");
    }
}

#[test]
fn refuses_what_is_not_a_date() {
    for bad in [
        "next friday",
        "20/09/2026",
        "2026-13-01",
        "2026-09-32",
        "2026-09-20T25:00Z",
        "2026-09-20T13:05:00 AEST",
        "",
    ] {
        assert!(check_iso8601(bad).is_err(), "{bad}");
    }
}

#[test]
fn a_new_board_is_the_apps_board() {
    let columns = default_columns();
    assert_eq!(columns.len(), 4);
    assert_eq!(columns[0].id, "backlog");
    assert_eq!(columns.last().unwrap().kind, "done");
}

#[test]
fn batch_items_parse_from_the_documented_json() {
    let items: Vec<NewTask> = serde_json::from_str(
        r#"[{"title":"Read the brief","column":"todo","due":"2026-09-20T23:59:00Z"},
                {"title":"Draft the intro","parent":1,"estimate":90},
                {"title":"Cite sources","parent":"intro","body":"APA"}]"#,
    )
    .unwrap();
    assert_eq!(items.len(), 3);
    assert!(matches!(items[1].parent, Some(ParentRef::Id(1))));
    assert!(matches!(items[2].parent, Some(ParentRef::Key(ref k)) if k == "intro"));
    assert_eq!(items[1].estimate, Some(90));
    assert!(serde_json::from_str::<Vec<NewTask>>(r#"[{"title":"x","deu":"2026-01-01"}]"#).is_err());
}
