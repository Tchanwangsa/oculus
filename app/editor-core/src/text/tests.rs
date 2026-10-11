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
fn verbatim_selection_keeps_ranges_as_given() {
    let r = SelectionRange::new;
    // Out of order and overlapping, as `EditorSelection.fromJSON` keeps them.
    let s = Selection::verbatim(vec![r(5, 8), r(2, 6)], 1).unwrap();
    assert_eq!(s.ranges().len(), 2);
    assert_eq!((s.main().anchor(), s.main_index()), (2, 1));
    assert!(Selection::verbatim(vec![], 0).is_err());
    assert!(Selection::verbatim(vec![r(0, 0)], 1).is_err());
}

#[test]
fn raw_range_keeps_a_mapped_from_past_to() {
    // "abcdefg" with 2..5 selected, all replaced by "xyz": the range maps to
    // from 3, to 0, which `range(3, 0)` would reorder to 0..3.
    let swallow = ChangeSet::of(&[ChangeSpec::replace(0, 7, "xyz")], 7).unwrap();
    let mapped = SelectionRange::new(2, 5).map(swallow.desc(), -1);
    assert_eq!((mapped.from(), mapped.to()), (3, 0));
    let ends = (mapped.anchor(), mapped.head());
    let raw = SelectionRange::raw(ends, (3, 0), None, None, mapped.assoc()).unwrap();
    assert_eq!(raw, mapped);
    // The two then map apart.
    let insert = ChangeSet::of(&[ChangeSpec::insert(0, "Q")], 3).unwrap();
    let next = |r: SelectionRange| {
        let m = r.map(insert.desc(), -1);
        (m.anchor(), m.head())
    };
    assert_eq!(next(raw), (4, 0));
    assert_eq!(next(SelectionRange::new(3, 0)), (4, 1));
    assert!(SelectionRange::raw((1, 2), (3, 0), None, None, 0).is_none());
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

/// A history restored mid-session (`from_events`) starts a new event on the
/// next keystroke; resumed with its previous time and user event
/// (`with_previous`) it joins the top one, as the live history did.
#[test]
fn resumed_history_groups_as_the_live_one() {
    let mut time = 1000;
    let (state, live) = type_text(
        State::new(Text::empty()),
        History::default(),
        "ab",
        &mut time,
        10,
    );
    let events = || {
        (
            live.done().cloned().collect::<Vec<_>>(),
            live.undone().cloned().collect::<Vec<_>>(),
        )
    };
    let (done, undone) = events();
    let restored = History::from_events(HistoryConfig::default(), done, undone);
    let (done, undone) = events();
    let resumed = History::from_events(HistoryConfig::default(), done, undone)
        .with_previous(live.prev_time(), live.prev_user_event());
    assert_eq!(resumed, live);
    let depth_after = |h: History| {
        let (_, h) = type_text(state.clone(), h, "c", &mut time.clone(), 10);
        h.undo_depth()
    };
    assert_eq!(depth_after(live.clone()), 1);
    assert_eq!(depth_after(resumed), 1);
    assert_eq!(depth_after(restored), 2);
    // An empty user event is none, on either side.
    let blank = live.clone().with_previous(5, Some(""));
    assert_eq!((blank.prev_time(), blank.prev_user_event()), (5, None));
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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// `from_parts`/`from_sections` read back what `parts`/`sections` wrote.
    #[test]
    fn json_forms_round_trip(src in source(30), pa in picks()) {
        let doc = Text::of(&src);
        let a = set(&doc, &pa);
        let back = ChangeSet::from_parts(&a.parts()).unwrap();
        prop_assert_eq!(&back, &a);
        prop_assert_eq!(back.apply(&doc).unwrap(), a.apply(&doc).unwrap());
        let sections: Vec<_> = a.sections().collect();
        prop_assert_eq!(ChangeDesc::from_sections(&sections).unwrap(), a.desc().clone());
    }

    /// A history rebuilt from its events checks out against the document
    /// and undoes and redoes as the original does.
    #[test]
    fn restored_history_behaves_as_the_original(
        src in source(20),
        steps in prop::collection::vec((0u8..6, picks(), any::<usize>()), 1..25),
    ) {
        let (state, history) = random_history(&Text::of(&src), &steps);
        let restored = History::from_events(
            HistoryConfig::default(),
            history.done().cloned().collect(),
            history.undone().cloned().collect(),
        );
        prop_assert!(restored.check(state.doc.len()).is_ok());
        prop_assert!(history.check(state.doc.len()).is_ok());
        prop_assert_eq!(restored.prev_time(), 0);
        prop_assert_eq!(restored.prev_user_event(), None);
        let other_len = state.doc.len() + 1;
        if history.done().any(|e| e.changes.is_some()) || history.undone().any(|e| e.changes.is_some()) {
            prop_assert!(restored.check(other_len).is_err());
        }
        // Undo everything, then redo everything, on both.
        let (mut a, mut b) = ((state.clone(), history), (state, restored));
        for redo in [false, true] {
            loop {
                let pa = if redo { a.1.redo(&a.0, 0) } else { a.1.undo(&a.0, 0) }.unwrap();
                let pb = if redo { b.1.redo(&b.0, 0) } else { b.1.undo(&b.0, 0) }.unwrap();
                match (pa, pb) {
                    (Some((ta, ha)), Some((tb, hb))) => {
                        prop_assert_eq!(&ta.changes, &tb.changes);
                        a = (a.0.apply(&ta).unwrap(), ha);
                        b = (b.0.apply(&tb).unwrap(), hb);
                        prop_assert_eq!(&a.0, &b.0);
                        prop_assert!(b.1.check(b.0.doc.len()).is_ok());
                    }
                    (None, None) => break,
                    _ => prop_assert!(false, "only one side could pop"),
                }
            }
        }
    }
}

/// A state and history after `steps`: edits (typed or not), selection
/// changes, untracked edits, undos and redos, 100 ms apart.
fn random_history(doc: &Text, steps: &[(u8, Picks, usize)]) -> (State, History) {
    let mut state = State::new(doc.clone());
    let mut history = History::default();
    for (i, (kind, p, at)) in steps.iter().enumerate() {
        let time = 1000 + 100 * i as i64;
        let popped = match kind {
            0 | 1 => {
                let changes = set(&state.doc, p);
                let end = changes.new_length();
                let tr = Transaction::new(changes, time)
                    .with_selection(Selection::single(at % (end + 1), at % (end + 1)))
                    .with_user_event(if *kind == 0 {
                        "input.type"
                    } else {
                        "input.paste"
                    });
                Some((tr, None))
            }
            2 => {
                let b = bounds(&state.doc);
                let pos = b[at % b.len()];
                let tr = Transaction::new(ChangeSet::empty(state.doc.len()), time)
                    .with_selection(Selection::single(pos, pos))
                    .with_user_event("select");
                Some((tr, None))
            }
            3 => {
                let tr = Transaction::new(set(&state.doc, p), time).with_add_to_history(false);
                Some((tr, None))
            }
            4 => history
                .undo(&state, time)
                .unwrap()
                .map(|(t, h)| (t, Some(h))),
            _ => history
                .redo(&state, time)
                .unwrap()
                .map(|(t, h)| (t, Some(h))),
        };
        if let Some((tr, next)) = popped {
            history = match next {
                Some(next) => next,
                None => history.update(&state, &tr).unwrap(),
            };
            state = state.apply(&tr).unwrap();
        }
    }
    (state, history)
}

#[test]
fn json_forms_keep_sections_as_given() {
    use change::Part;
    // Two kept runs side by side stay two, as `ChangeSet.fromJSON` keeps them.
    let set = ChangeSet::from_parts(&[
        Part::Keep(1),
        Part::Keep(2),
        Part::Replace(1, vec!["x".into(), "y".into()]),
        Part::Replace(0, vec![]),
    ])
    .unwrap();
    assert_eq!(
        set.sections().collect::<Vec<_>>(),
        [(1, -1), (2, -1), (1, 3), (0, 0)]
    );
    assert_eq!(set.apply(&Text::of("abcd")).unwrap().to_string(), "abcx\ny");
    assert!(ChangeSet::from_parts(&[Part::Replace(0, vec!["a\nb".into()])]).is_err());
    assert!(ChangeDesc::from_sections(&[(1, -2)]).is_err());
    assert_eq!(
        ChangeDesc::from_sections(&[(2, -1), (1, 0)])
            .unwrap()
            .new_length(),
        2
    );
}

#[test]
fn check_refuses_inconsistent_events() {
    let mut time = 1000;
    let (_, history) = type_text(
        State::new(Text::empty()),
        History::default(),
        "abc",
        &mut time,
        10,
    );
    let done: Vec<HistoryEvent> = history.done().cloned().collect();
    let restore =
        |done: Vec<HistoryEvent>| History::from_events(HistoryConfig::default(), done, vec![]);
    assert!(restore(done.clone()).check(3).is_ok());
    assert!(restore(done.clone()).check(2).is_err());
    // A start selection past the document before the event.
    let mut bad = done.clone();
    bad[0].start_selection = Some(Selection::single(1, 1));
    assert!(restore(bad).check(3).is_err());
    // Changes without a start selection, which undo could not restore.
    let mut bad = done.clone();
    bad[0].start_selection = None;
    assert!(restore(bad).check(3).is_err());
    // A selection-only event above a change event.
    let mut bad = done;
    bad.push(HistoryEvent {
        changes: None,
        mapped: None,
        start_selection: None,
        selections_after: vec![Selection::single(0, 0)],
    });
    assert!(restore(bad).check(3).is_err());
}

#[test]
fn unaligned_sections_are_refused_not_panicked() {
    use change::Part;
    // "ab" → "a" with a zero-length kept run left at the end, then a
    // deletion of the rest: CodeMirror's compose throws on it.
    let a =
        ChangeSet::from_parts(&[Part::Keep(1), Part::Replace(1, vec![]), Part::Keep(0)]).unwrap();
    let b = ChangeSet::from_parts(&[Part::Replace(1, vec![])]).unwrap();
    assert!(matches!(a.try_compose(&b), Err(ChangeError::Malformed(_))));
    assert!(a.desc().try_compose_desc(b.desc()).is_err());
    // Merged as CodeMirror merges its own, it composes.
    let merged = ChangeSet::from_parts(&[Part::Keep(1), Part::Replace(1, vec![])]).unwrap();
    assert_eq!(
        merged
            .try_compose(&b)
            .unwrap()
            .apply(&Text::of("ab"))
            .unwrap()
            .to_string(),
        ""
    );
}
