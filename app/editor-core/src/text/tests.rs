//! Change-set laws and selection/history unit tests. Exact parity with
//! CodeMirror is the oracle's job (`oracle/changes.ts`, `oracle/history.ts`);
//! these check the algebra holds on mixed-script documents.

use proptest::prelude::*;

use super::*;

const PIECES: &[&str] = &[
    "a",
    "z",
    " ",
    "word",
    "ก",
    "ที่",
    "\u{0E31}",
    "e\u{301}",
    "😀",
    "𝒳",
    "中",
    "\n",
    "\r\n",
];

fn source(max: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(PIECES), 0..max).prop_map(|p| p.concat())
}

/// Every code-point boundary of `doc`, in UTF-16 units.
fn bounds(doc: &Text) -> Vec<usize> {
    let mut out = vec![0];
    let mut pos = 0;
    for c in doc.to_string().chars() {
        pos += c.len_utf16();
        out.push(pos);
    }
    out
}

/// Raw picks, turned into specs at `doc`'s boundaries by `specs`.
type Picks = Vec<(usize, usize, String)>;

fn picks() -> impl Strategy<Value = Picks> {
    prop::collection::vec((any::<usize>(), 0usize..6, source(4)), 0..6)
}

fn specs(doc: &Text, picks: &Picks) -> Vec<ChangeSpec> {
    let b = bounds(doc);
    picks
        .iter()
        .map(|(at, span, insert)| {
            let i = at % b.len();
            let j = (i + span).min(b.len() - 1);
            ChangeSpec::replace(b[i], b[j], insert)
        })
        .collect()
}

fn set(doc: &Text, p: &Picks) -> ChangeSet {
    ChangeSet::of(&specs(doc, p), doc.len()).unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn compose_applies_both(src in source(30), pa in picks(), pb in picks()) {
        let doc = Text::of(&src);
        let a = set(&doc, &pa);
        let mid = a.apply(&doc).unwrap();
        let b = set(&mid, &pb);
        let ab = a.compose(&b);
        prop_assert_eq!(ab.length(), doc.len());
        prop_assert_eq!(ab.new_length(), b.new_length());
        prop_assert_eq!(ab.apply(&doc).unwrap(), b.apply(&mid).unwrap());
        // The description composes the same way.
        prop_assert_eq!(a.desc().compose_desc(b.desc()), ab.desc().clone());
    }

    #[test]
    fn invert_undoes(src in source(30), pa in picks()) {
        let doc = Text::of(&src);
        let a = set(&doc, &pa);
        let after = a.apply(&doc).unwrap();
        let inv = a.invert(&doc).unwrap();
        prop_assert_eq!(inv.apply(&after).unwrap(), doc.clone());
        prop_assert_eq!(inv.desc().clone(), a.inverted_desc());
        prop_assert!(a.compose(&inv).iter_changes(false).all(|c| c.from_a == c.from_b));
        prop_assert_eq!(a.compose(&inv).apply(&doc).unwrap(), doc);
    }

    #[test]
    fn mapping_through_compose(src in source(30), pa in picks(), pb in picks()) {
        let doc = Text::of(&src);
        let a = set(&doc, &pa);
        let mid = a.apply(&doc).unwrap();
        let b = set(&mid, &pb);
        let ab = a.compose(&b);
        // Away from changes, mapping through the composition equals mapping
        // through each step. (Touching a change it need not: an insertion in
        // `b` where `a` deleted is one replacement in `ab`.)
        for pos in 0..=doc.len() {
            if ab.touches_range(pos, pos) != Touch::No {
                continue;
            }
            for assoc in [-1, 1] {
                let stepwise = b.map_pos(a.map_pos(pos, assoc), assoc);
                prop_assert_eq!(ab.map_pos(pos, assoc), stepwise);
            }
        }
    }

    #[test]
    fn mapped_changes_converge(src in source(30), pa in picks(), pb in picks()) {
        let doc = Text::of(&src);
        let a = set(&doc, &pa);
        let b = set(&doc, &pb);
        // a then b-over-a equals b then a-over-b, with a's insertions first
        // on both paths.
        let one = a.compose(&b.map(&a, false));
        let two = b.compose(&a.map(&b, true));
        prop_assert_eq!(one.apply(&doc).unwrap(), two.apply(&doc).unwrap());
        prop_assert_eq!(b.map(&a, false).desc().clone(), b.desc().map_desc(a.desc(), false));
    }

    #[test]
    fn iterators_agree(src in source(30), pa in picks()) {
        let doc = Text::of(&src);
        let a = set(&doc, &pa);
        // Rebuild the result from gaps and changes.
        let after = a.apply(&doc).unwrap();
        // On a tie, an insertion goes before the gap that starts there.
        let mut pieces: Vec<(usize, bool, String)> = a
            .iter_gaps()
            .into_iter()
            .map(|(pa, _, len)| (pa, true, doc.slice_string(pa, pa + len).unwrap()))
            .collect();
        pieces.extend(
            a.iter_changes(true)
                .map(|c| (c.from_a, false, c.inserted.to_string())),
        );
        pieces.sort_by_key(|p| (p.0, p.1));
        let rebuilt: String = pieces.into_iter().map(|p| p.2).collect();
        prop_assert_eq!(rebuilt, after.to_string());
        let joined: Vec<_> = a.iter_changes(false).map(|c| (c.from_a, c.to_a, c.from_b, c.to_b)).collect();
        let ranges: Vec<_> = a.iter_changed_ranges(false).map(|c| (c.from_a, c.to_a, c.from_b, c.to_b)).collect();
        prop_assert_eq!(joined, ranges);
    }
}

#[test]
fn map_modes() {
    // "abcdef": delete "cd", insert "X" at 5.
    let a = ChangeSet::of(&[ChangeSpec::delete(2, 4), ChangeSpec::insert(5, "X")], 6).unwrap();
    assert_eq!(a.map_pos_mode(3, -1, MapMode::TrackDel), None);
    assert_eq!(a.map_pos_mode(2, -1, MapMode::TrackDel), Some(2));
    assert_eq!(a.map_pos_mode(2, -1, MapMode::TrackAfter), None);
    assert_eq!(a.map_pos_mode(4, -1, MapMode::TrackBefore), None);
    assert_eq!(a.map_pos(5, -1), 3);
    assert_eq!(a.map_pos(5, 1), 4);
    assert_eq!(a.touches_range(3, 3), Touch::Cover);
    assert_eq!(a.touches_range(0, 1), Touch::No);
}

#[test]
fn of_composes_out_of_order_specs() {
    let doc = Text::of("abcdef");
    let a = ChangeSet::of(
        &[ChangeSpec::replace(4, 5, "E"), ChangeSpec::insert(1, "x")],
        6,
    )
    .unwrap();
    assert_eq!(a.apply(&doc).unwrap().to_string(), "axbcdEf");
    assert!(ChangeSet::of(&[ChangeSpec::delete(3, 9)], 6).is_err());
}

#[test]
fn selection_normalises_like_codemirror() {
    let r = SelectionRange::new;
    // A cursor touching the previous range's end merges; a range does not.
    let s = Selection::create(vec![r(5, 8), r(8, 8)], 1).unwrap();
    assert_eq!(s.ranges().len(), 1);
    assert_eq!(s.main_index(), 0);
    let s = Selection::create(vec![r(2, 5), r(5, 7)], 0).unwrap();
    assert_eq!(s.ranges().len(), 2);
    // The merged range points backward only if the later range did.
    let s = Selection::create(vec![r(6, 3), r(9, 4)], 0).unwrap();
    assert_eq!((s.main().anchor(), s.main().head()), (9, 3));
}

#[test]
fn replace_selection_with_two_cursors() {
    let doc = Text::of("ab cd");
    let sel = Selection::create(
        vec![
            SelectionRange::cursor(1, 0, None, None),
            SelectionRange::new(3, 5),
        ],
        1,
    )
    .unwrap();
    let state = State::with_selection(doc, sel).unwrap();
    let (changes, selection) = state.replace_selection("ไ😀").unwrap();
    let tr = Transaction::new(changes, 0).with_selection(selection);
    let next = state.apply(&tr).unwrap();
    assert_eq!(next.doc.to_string(), "aไ😀b ไ😀");
    let heads: Vec<_> = next.selection.ranges().iter().map(|r| r.head()).collect();
    assert_eq!(heads, [4, 9]);
    assert_eq!(next.selection.main_index(), 1);
}

/// Types `text` at the end, one character per transaction, `gap` ms apart.
fn type_text(
    mut state: State,
    mut history: History,
    text: &str,
    time: &mut i64,
    gap: i64,
) -> (State, History) {
    for c in text.chars() {
        let end = state.doc.len();
        let changes = state
            .changes(&[ChangeSpec::insert(end, &c.to_string())])
            .unwrap();
        let cursor = Selection::single(end + c.len_utf16(), end + c.len_utf16());
        let tr = Transaction::new(changes, *time)
            .with_selection(cursor)
            .with_user_event("input.type");
        *time += gap;
        history = history.update(&state, &tr).unwrap();
        state = state.apply(&tr).unwrap();
    }
    (state, history)
}

#[test]
fn history_groups_and_undoes() {
    let mut time = 1000;
    let (state, history) = type_text(
        State::new(Text::empty()),
        History::default(),
        "ab",
        &mut time,
        10,
    );
    time += 1000;
    let (state, history) = type_text(state, history, "cd", &mut time, 10);
    assert_eq!(history.undo_depth(), 2);
    let (tr, history) = history.undo(&state, time).unwrap().unwrap();
    let state = state.apply(&tr).unwrap();
    assert_eq!(state.doc.to_string(), "ab");
    assert_eq!((history.undo_depth(), history.redo_depth()), (1, 1));
    let (tr, history) = history.redo(&state, time).unwrap().unwrap();
    let state = state.apply(&tr).unwrap();
    assert_eq!(state.doc.to_string(), "abcd");
    assert_eq!(state.selection.main().head(), 4);
    assert_eq!(history.redo_depth(), 0);
}

#[test]
fn history_rebases_over_untracked_changes() {
    let mut time = 1000;
    let (state, history) = type_text(
        State::new(Text::empty()),
        History::default(),
        "hello",
        &mut time,
        10,
    );
    // An untracked insertion before the typed text shifts the undo.
    let changes = state.changes(&[ChangeSpec::insert(0, "$x$ ")]).unwrap();
    let tr = Transaction::new(changes, time).with_add_to_history(false);
    let history = history.update(&state, &tr).unwrap();
    let state = state.apply(&tr).unwrap();
    let (tr, history) = history.undo(&state, time).unwrap().unwrap();
    let state = state.apply(&tr).unwrap();
    assert_eq!(state.doc.to_string(), "$x$ ");
    assert_eq!(history.undo_depth(), 0);
    // An untracked deletion that swallows the whole event drops it.
    let (state, history) = type_text(state, history, "ab", &mut time, 10);
    let changes = state.changes(&[ChangeSpec::delete(0, 6)]).unwrap();
    let tr = Transaction::new(changes, time).with_add_to_history(false);
    let history = history.update(&state, &tr).unwrap();
    assert_eq!(history.undo_depth(), 0);
}

#[test]
fn history_refuses_a_transaction_for_another_document() {
    let state = State::new(Text::of("abcde"));
    let other = State::new(Text::of("abc"));
    for spec in [ChangeSpec::insert(1, "x"), ChangeSpec::delete(0, 2)] {
        let tr = Transaction::new(other.changes(&[spec]).unwrap(), 0);
        assert!(History::default().update(&state, &tr).is_err());
    }
    // A change that splits a surrogate pair of the start document.
    let state = State::new(Text::of("😀x"));
    let tr = Transaction::new(ChangeSet::of(&[ChangeSpec::delete(0, 1)], 3).unwrap(), 0);
    assert!(History::default().update(&state, &tr).is_err());
}

#[test]
fn mismatched_lengths_are_refused_not_looped() {
    let a = ChangeSet::of(&[ChangeSpec::replace(0, 1, "a")], 4).unwrap();
    let b = ChangeSet::of(
        &[ChangeSpec::delete(2, 3), ChangeSpec::replace(3, 5, "a")],
        7,
    )
    .unwrap();
    assert!(a.try_map(&b, false).is_err());
    assert!(a.try_compose(&b).is_err());
    assert!(std::panic::catch_unwind(|| a.map(&b, true)).is_err());
    assert!(std::panic::catch_unwind(|| a.desc().map_desc(b.desc(), false)).is_err());
    // `b` deleting half of an emoji `a` inserted.
    let a = ChangeSet::of(&[ChangeSpec::insert(0, "😀")], 1).unwrap();
    let b = ChangeSet::of(&[ChangeSpec::delete(0, 1)], 3).unwrap();
    assert!(matches!(a.try_compose(&b), Err(ChangeError::Pos(_))));
}

#[test]
fn empty_user_event_is_none() {
    let tr = Transaction::new(ChangeSet::empty(0), 0).with_user_event("");
    assert_eq!(tr.user_event(), None);
    assert!(!tr.is_user_event(""));
    // So, as in CodeMirror, it joins a typing group like an unlabelled edit.
    let mut time = 1000;
    let (state, history) = type_text(
        State::new(Text::empty()),
        History::default(),
        "ab",
        &mut time,
        10,
    );
    let changes = state.changes(&[ChangeSpec::insert(2, "c")]).unwrap();
    let tr = Transaction::new(changes, time).with_user_event("");
    assert_eq!(history.update(&state, &tr).unwrap().undo_depth(), 1);
}

#[test]
fn history_time_extremes_do_not_overflow() {
    let (state, history) = type_text(
        State::new(Text::empty()),
        History::default(),
        "a",
        &mut i64::MAX.clone(),
        0,
    );
    let changes = state.changes(&[ChangeSpec::insert(1, "b")]).unwrap();
    let tr = Transaction::new(changes, i64::MIN).with_user_event("input.type");
    assert_eq!(history.update(&state, &tr).unwrap().undo_depth(), 1);
}
