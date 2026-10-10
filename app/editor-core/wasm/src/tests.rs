//! `Snapshot` natively: seeding from CodeMirror's JSON, edits, history
//! commands and the tree, and refusals instead of panics. Parity with
//! CodeMirror itself is the oracles' job (`oracle/history.ts` restores
//! histories from the same JSON).

use crate::{Command, Snapshot, node_names};

const CURSOR: &str = r#"{"ranges":[{"anchor":0,"head":0}],"main":0}"#;

fn cursor(pos: usize) -> String {
    format!(r#"{{"ranges":[{{"anchor":{pos},"head":{pos}}}],"main":0}}"#)
}

/// Types `text` at `at`, one transaction per char, `gap` ms apart.
fn type_at(mut s: Snapshot, at: usize, text: &str, time: &mut f64, gap: f64) -> Snapshot {
    let mut pos = at;
    for c in text.chars() {
        let (keep, rest) = (pos, s.len() - pos);
        let insert = serde_json::json!([0, c.to_string()]);
        // As `toJSON` writes it: no empty kept runs.
        let changes = serde_json::json!([keep, insert, rest]);
        let changes = changes
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p.as_u64() != Some(0))
            .collect::<Vec<_>>();
        let changes = serde_json::to_string(&changes).unwrap();
        pos += c.len_utf16();
        s = s
            .apply(
                &changes,
                Some(&cursor(pos)),
                Some("input.type"),
                true,
                None,
                *time,
            )
            .unwrap();
        *time += gap;
    }
    s
}

#[test]
fn edits_undo_and_redo() {
    let seed = Snapshot::seed("# Hi\n", CURSOR, None).unwrap();
    let mut time = 1000.0;
    let typed = type_at(seed.clone(), 5, "a *b*", &mut time, 10.0);
    assert_eq!(typed.text(), "# Hi\na *b*");
    assert_eq!((typed.undo_depth(), typed.redo_depth()), (1, 0));
    assert_eq!(typed.selection_json(), cursor(10));
    // The receiver is left as it was.
    assert_eq!(seed.text(), "# Hi\n");
    assert_eq!(seed.changes_json(), None);

    let undone = typed.pop(Command::Undo, time).unwrap().unwrap();
    assert_eq!(undone.text(), "# Hi\n");
    assert_eq!(undone.changes_json().as_deref(), Some("[5,[5]]"));
    assert_eq!(undone.changed_ranges(), [5, 10, 5, 5]);
    assert_eq!(undone.tree(), seed.tree());
    assert_eq!(
        undone.pop(Command::Undo, time).unwrap().map(|s| s.text()),
        None
    );
    let redone = undone.pop(Command::Redo, time).unwrap().unwrap();
    assert_eq!(redone.text(), typed.text());
    assert_eq!(redone.tree(), typed.tree());
}

#[test]
fn tree_names_its_types() {
    let s = Snapshot::seed("a *b*", CURSOR, None).unwrap();
    let names = node_names();
    let nodes: Vec<String> = s
        .tree()
        .chunks(3)
        .map(|t| format!("{} {} {}", names[t[0] as usize], t[1], t[2]))
        .collect();
    assert_eq!(
        nodes,
        [
            "Document 0 5",
            "Paragraph 0 5",
            "Emphasis 2 5",
            "EmphasisMark 2 3",
            "EmphasisMark 4 5"
        ]
    );
}

#[test]
fn history_round_trips_through_json() {
    let mut time = 1000.0;
    let s = Snapshot::seed("", CURSOR, None).unwrap();
    let s = type_at(s, 0, "ab", &mut time, 10.0);
    time += 1000.0;
    let s = type_at(s, 2, "cd", &mut time, 10.0);
    let s = s.pop(Command::Undo, time).unwrap().unwrap();
    let json = s.history_json();
    assert_eq!(
        json,
        concat!(
            r#"{"done":[{"changes":[[2]],"startSelection":{"ranges":[{"anchor":0,"head":0}],"main":0},"selectionsAfter":[]}],"#,
            r#""undone":[{"changes":[2,[0,"cd"]],"startSelection":{"ranges":[{"anchor":4,"head":4}],"main":0},"selectionsAfter":[]}]}"#
        )
    );
    let restored = Snapshot::seed(&s.text(), &s.selection_json(), Some(&json)).unwrap();
    assert_eq!(restored.history_json(), json);
    assert_eq!((restored.undo_depth(), restored.redo_depth()), (1, 1));
    let redone = restored.pop(Command::Redo, time).unwrap().unwrap();
    assert_eq!(redone.text(), "abcd");
    assert_eq!(redone.selection_json(), cursor(4));
    // CodeMirror's JSON of an empty history.
    let empty = Snapshot::seed("x", CURSOR, Some(r#"{"done":[],"undone":[]}"#)).unwrap();
    assert_eq!(empty.undo_depth(), 0);
}

#[test]
fn selection_ranges_keep_goal_column_and_assoc() {
    // The history records a selection unless it equals the last one, goal
    // column included: here the cursor at 5 is recorded twice, once without
    // and once with a goal column, so selection undo has two steps.
    let s = Snapshot::seed("abc\ndef", &cursor(5), None).unwrap();
    let with_goal = r#"{"ranges":[{"anchor":5,"head":5,"goalColumn":12.5,"assoc":-1}],"main":0}"#;
    let s = s
        .apply("[7]", Some(with_goal), Some("select"), true, None, 1000.0)
        .unwrap();
    let mut s = s
        .apply("[7]", Some(&cursor(0)), Some("select"), true, None, 3000.0)
        .unwrap();
    let mut steps = 0;
    while let Some(back) = s.pop(Command::UndoSelection, 4000.0).unwrap() {
        assert_eq!(back.selection_json(), cursor(5));
        (s, steps) = (back, steps + 1);
    }
    assert_eq!(steps, 2);
}

#[test]
fn refusals_are_errors() {
    let s = Snapshot::seed("abc", CURSOR, None).unwrap();
    assert!(Snapshot::seed("abc", &cursor(4), None).is_err());
    assert!(Snapshot::seed("abc", "{", None).is_err());
    // A history whose top event is over another document.
    let history = r#"{"done":[{"changes":[[2]],"startSelection":{"ranges":[{"anchor":0,"head":0}],"main":0},"selectionsAfter":[]}],"undone":[]}"#;
    assert!(Snapshot::seed("abc", CURSOR, Some(history)).is_err());
    assert!(Snapshot::seed("ab", CURSOR, Some(history)).is_ok());
    // Changes for another length, a selection past the end, a bad isolate,
    // a fractional time, a split surrogate pair.
    assert!(s.apply("[4]", None, None, true, None, 0.0).is_err());
    assert!(
        s.apply("[3]", Some(&cursor(9)), None, true, None, 0.0)
            .is_err()
    );
    assert!(
        s.apply("[3]", None, None, true, Some("middle"), 0.0)
            .is_err()
    );
    assert!(s.apply("[3]", None, None, true, None, 0.5).is_err());
    assert!(s.apply("[3]", None, None, true, Some("full"), 0.0).is_ok());
    assert!(s.slice(2, 9).is_err());
    let emoji = Snapshot::seed("😀", CURSOR, None).unwrap();
    assert!(emoji.apply("[[1],1]", None, None, true, None, 0.0).is_err());
    assert!(emoji.slice(0, 1).is_err());
}

#[test]
fn unaligned_sections_are_refused_as_codemirror_throws() {
    // A kept run of zero units at the end, which `fromJSON` keeps and
    // composing then throws on when the keystroke joins the one before.
    let s = Snapshot::seed("", CURSOR, None).unwrap();
    let s = s
        .apply(
            r#"[[0,"a"]]"#,
            Some(&cursor(1)),
            Some("input.type"),
            true,
            None,
            1000.0,
        )
        .unwrap();
    let typed = |at: f64| {
        s.apply(
            r#"[1,[0,"b"],0]"#,
            Some(&cursor(2)),
            Some("input.type"),
            true,
            None,
            at,
        )
    };
    assert!(typed(1010.0).is_err());
    // Not joined, it is recorded as it is.
    assert_eq!(typed(5000.0).unwrap().text(), "ab");
}

#[test]
fn untracked_changes_rebase_the_history() {
    let mut time = 1000.0;
    let s = type_at(
        Snapshot::seed("", CURSOR, None).unwrap(),
        0,
        "hi",
        &mut time,
        10.0,
    );
    let s = s
        .apply(r#"[[0,"$x$ "],2]"#, None, None, false, None, time)
        .unwrap();
    assert_eq!(s.selection_json(), cursor(6));
    let undone = s.pop(Command::Undo, time).unwrap().unwrap();
    assert_eq!(undone.text(), "$x$ ");
}

#[test]
fn a_live_seed_resumes_grouping() {
    // Typed "ab" 10 ms apart; seeded from its JSON alone the next keystroke
    // starts a new event, with the live history's previous time and user
    // event it joins the one before, as CodeMirror's history would.
    let mut time = 1000.0;
    let typed = type_at(
        Snapshot::seed("", CURSOR, None).unwrap(),
        0,
        "ab",
        &mut time,
        10.0,
    );
    let json = typed.history_json();
    let live = format!(
        r#"{}, "prevTime": {}, "prevUserEvent": "input.type"}}"#,
        &json[..json.len() - 1],
        time - 10.0
    );
    let rich = r#"{"ranges":[{"anchor":2,"head":2,"goalColumn":null,"bidiLevel":null,"assoc":1}],"main":0}"#;
    let depth = |history: &str| {
        let s = Snapshot::seed("ab", rich, Some(history)).unwrap();
        type_at(s, 2, "c", &mut time.clone(), 10.0).undo_depth()
    };
    assert_eq!(depth(&json), 2);
    assert_eq!(depth(&live), 1);
    // The extras are read, never written.
    let s = Snapshot::seed("ab", rich, Some(&live)).unwrap();
    assert_eq!(s.history_json(), json);
    let fractional = live.replace(&format!("{}", time - 10.0), "1.5");
    assert!(Snapshot::seed("ab", rich, Some(&fractional)).is_err());
}

#[test]
fn a_range_with_from_past_to_is_kept_as_given() {
    // Mapping can leave CodeMirror holding from 3, to 0 (anchor 3, head 0);
    // with its `from` and `to` the seed keeps it, and it maps on as
    // CodeMirror's does rather than as the reordered 0..3.
    let raw = r#"{"ranges":[{"anchor":3,"head":0,"from":3,"to":0,"assoc":-1}],"main":0}"#;
    let plain = r#"{"ranges":[{"anchor":3,"head":0}],"main":0}"#;
    let mapped = |sel: &str| {
        let s = Snapshot::seed("xyz", sel, None).unwrap();
        assert_eq!(s.selection_json(), plain);
        s.apply(r#"[[0,"Q"],3]"#, None, None, false, None, 1000.0)
            .unwrap()
            .selection_json()
    };
    assert_eq!(
        mapped(raw),
        r#"{"ranges":[{"anchor":4,"head":0}],"main":0}"#
    );
    assert_eq!(
        mapped(plain),
        r#"{"ranges":[{"anchor":4,"head":1}],"main":0}"#
    );
    let neither = r#"{"ranges":[{"anchor":1,"head":2,"from":3,"to":0}],"main":0}"#;
    assert!(Snapshot::seed("xyz", neither, None).is_err());
}
