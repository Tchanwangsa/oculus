//! Unit and property tests against a naive model. Most run at `Small` sizes
//! (32-byte leaves, fan-out 6) so a few hundred bytes make a deep tree; the
//! edit proptest and a multi-MB stress run also cover `Release` sizes.

use std::collections::HashSet;

use proptest::prelude::*;

use super::tree::{self, Release, Sizes, Small};
use super::*;

type SmallText = Text<Small>;

/// ASCII, Thai (with combining vowels and tone marks), a Latin combining
/// accent, astral characters (two UTF-16 units) and every line break.
const PIECES: &[&str] = &[
    "a",
    "z",
    " ",
    "word",
    "ก",
    "ไ",
    "ที่",
    "\u{0E31}",
    "e\u{301}",
    "é",
    "😀",
    "𝒳",
    "👩\u{200D}👧",
    "中",
    "\n",
    "\r\n",
    "\r",
    "$x$",
    "#",
];

fn source(max: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(PIECES), 0..max).prop_map(|p| p.concat())
}

fn normalise(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// Every UTF-16 position that is a code-point boundary.
fn boundaries(s: &str) -> Vec<usize> {
    let mut out = vec![0];
    let mut pos = 0;
    for c in s.chars() {
        pos += c.len_utf16();
        out.push(pos);
    }
    out
}

fn byte_of(s: &str, pos: usize) -> usize {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units == pos {
            return i;
        }
        units += c.len_utf16();
    }
    assert_eq!(units, pos);
    s.len()
}

fn model_line(s: &str, number: usize) -> Line {
    let mut from = 0;
    for (i, text) in s.split('\n').enumerate() {
        let len = utf16_len(text);
        if i + 1 == number {
            return Line {
                number,
                from,
                to: from + len,
                text: text.to_owned(),
            };
        }
        from += len + 1;
    }
    panic!("no line {number}");
}

fn model_line_at(s: &str, pos: usize) -> Line {
    model_line(s, s[..byte_of(s, pos)].matches('\n').count() + 1)
}

/// Checks `text` against `model` everywhere: invariants, content, every line,
/// `line_at` at every boundary, refusals inside surrogates and past the end.
fn check_against<S: Sizes>(text: &Text<S>, model: &str) {
    tree::check::<S>(&text.root);
    assert_eq!(text.to_string(), model);
    let len = text.len();
    assert_eq!(len, utf16_len(model));
    let lines = model.matches('\n').count() + 1;
    assert_eq!(text.lines(), lines);
    for n in 1..=lines {
        assert_eq!(text.line(n), Some(model_line(model, n)));
    }
    assert_eq!(text.line(0), None);
    assert_eq!(text.line(lines + 1), None);
    let bounds = boundaries(model);
    for &pos in &bounds {
        assert_eq!(
            text.line_at(pos),
            Ok(model_line_at(model, pos)),
            "line_at({pos})"
        );
    }
    for pos in 0..len {
        if bounds.binary_search(&pos).is_err() {
            let inside = Some(PosError::InsideSurrogate { pos });
            assert_eq!(text.line_at(pos).err(), inside);
            assert_eq!(text.slice_string(0, pos).err(), inside);
            assert_eq!(text.slice_string(pos, len).err(), inside);
            assert_eq!(text.slice(0, pos).err(), inside);
            assert_eq!(text.slice(pos, len).err(), inside);
            assert_eq!(text.iter_range(0, pos).err(), inside);
            assert_eq!(text.iter_range(pos, 0).err(), inside);
            assert_eq!(text.iter_range(len, pos).err(), inside);
            assert_eq!(text.replace(pos, len, &Text::empty_in()).err(), inside);
        }
    }
    let past = Some(PosError::OutOfRange { pos: len + 1, len });
    assert_eq!(text.line_at(len + 1).err(), past);
    assert_eq!(text.slice(0, len + 1).err(), past);
    assert_eq!(text.iter_range(len + 1, 0).err(), past);
}

fn pick(bounds: &[usize], f: f64) -> usize {
    bounds[((bounds.len() - 1) as f64 * f).round() as usize]
}

#[derive(Debug, Clone)]
enum Op {
    Replace(f64, f64, String),
    Append(String),
    Slice(f64, f64),
}

fn op() -> impl Strategy<Value = Op> {
    let insert = prop_oneof![4 => source(12), 1 => source(300)];
    prop_oneof![
        6 => (0.0..=1.0, 0.0..=1.0, insert.clone()).prop_map(|(a, b, s)| Op::Replace(a, b, s)),
        1 => insert.prop_map(Op::Append),
        1 => (0.0..=1.0, 0.0..=1.0).prop_map(|(a, b)| Op::Slice(a, b)),
    ]
}

/// Applies `ops` to both the rope and a `String` model, checking invariants,
/// content and persistence after each.
fn run_edits<S: Sizes>(src: &str, ops: Vec<Op>) -> Result<(), TestCaseError> {
    let mut text = Text::<S>::of_in(src);
    let mut model = normalise(src);
    for op in ops {
        let before = (text.clone(), model.clone());
        let bounds = boundaries(&model);
        match op {
            Op::Replace(a, b, insert) => {
                let (from, to) = (pick(&bounds, a.min(b)), pick(&bounds, a.max(b)));
                let ins = normalise(&insert);
                model.replace_range(byte_of(&model, from)..byte_of(&model, to), &ins);
                text = text.replace(from, to, &Text::of_in(&insert)).unwrap();
            }
            Op::Append(insert) => {
                model.push_str(&normalise(&insert));
                text = text.append(&Text::of_in(&insert));
            }
            Op::Slice(a, b) => {
                let (from, to) = (pick(&bounds, a.min(b)), pick(&bounds, a.max(b)));
                model = model[byte_of(&model, from)..byte_of(&model, to)].to_owned();
                text = text.slice(from, to).unwrap();
            }
        }
        tree::check::<S>(&text.root);
        prop_assert_eq!(text.to_string(), model.clone());
        // Persistence: the previous version is untouched.
        prop_assert_eq!(before.0.to_string(), before.1);
    }
    check_against(&text, &model);
    prop_assert!(text == Text::of_in(&model));
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn of_matches_model(src in prop_oneof![source(40), source(600)]) {
        check_against(&SmallText::of_in(&src), &normalise(&src));
    }

    #[test]
    fn edits_match_model(
        src in prop_oneof![source(40), source(600)],
        ops in prop::collection::vec(op(), 1..25),
    ) {
        run_edits::<Small>(&src, ops)?;
    }

    #[test]
    fn edits_match_model_at_release_sizes(
        src in prop_oneof![source(40), source(3000)],
        ops in prop::collection::vec(op(), 1..25),
    ) {
        run_edits::<Release>(&src, ops)?;
    }

    #[test]
    fn slices_match_model(src in source(400), a in 0.0..=1.0f64, b in 0.0..=1.0f64) {
        let text = SmallText::of_in(&src);
        let model = normalise(&src);
        let bounds = boundaries(&model);
        let (from, to) = (pick(&bounds, a.min(b)), pick(&bounds, a.max(b)));
        let expected = &model[byte_of(&model, from)..byte_of(&model, to)];
        prop_assert_eq!(text.slice_string(from, to).unwrap(), expected);
        let slice = text.slice(from, to).unwrap();
        tree::check::<Small>(&slice.root);
        prop_assert_eq!(slice.to_string(), expected);
        if from < to {
            let reversed = Some(PosError::Reversed { from: to, to: from });
            prop_assert_eq!(text.slice_string(to, from).err(), reversed);
            prop_assert_eq!(text.slice(to, from).err(), reversed);
        }
    }

    #[test]
    fn iterators_match_model(
        src in source(400),
        a in 0.0..=1.0f64,
        b in 0.0..=1.0f64,
        la in 0.0..=1.0f64,
        lb in 0.0..=1.0f64,
    ) {
        let text = SmallText::of_in(&src);
        let model = normalise(&src);
        let runs: Vec<&str> = text.iter().collect();
        for run in &runs {
            prop_assert!(*run == "\n" || (!run.is_empty() && !run.contains('\n')), "run {:?}", run);
        }
        prop_assert_eq!(runs.concat(), model.clone());
        let mut back: Vec<&str> = text.iter_rev().collect();
        back.reverse();
        prop_assert_eq!(back.concat(), model.clone());

        let bounds = boundaries(&model);
        let (from, to) = (pick(&bounds, a), pick(&bounds, b));
        let (lo, hi) = (byte_of(&model, from.min(to)), byte_of(&model, from.max(to)));
        let mut ranged: Vec<&str> = text.iter_range(from, to).unwrap().collect();
        if from > to {
            ranged.reverse();
        }
        prop_assert_eq!(ranged.concat(), &model[lo..hi]);

        let lines = text.lines();
        let first = 1 + ((lines - 1) as f64 * la).round() as usize;
        let end = ((lines + 1) as f64 * lb).round() as usize;
        let start = model_line(&model, first).from;
        let stop = if end == lines + 1 {
            text.len()
        } else if end <= 1 {
            0
        } else {
            model_line(&model, end - 1).to
        };
        let span = &model[byte_of(&model, start)..byte_of(&model, stop.max(start))];
        let expected: Vec<&str> = span.split('\n').collect();
        let actual: Vec<String> = text.iter_lines(first, end).unwrap().map(|l| l.into_owned()).collect();
        prop_assert_eq!(actual, expected);
        prop_assert!(text.iter_lines(0, 1).is_none());
        prop_assert!(text.iter_lines(1, lines + 2).is_none());
    }

    #[test]
    fn eq_ignores_tree_shape(pieces in prop::collection::vec(source(30), 0..20)) {
        let whole = SmallText::of_in(&pieces.concat());
        let built = pieces.iter().fold(SmallText::empty_in(), |acc, p| acc.append(&Text::of_in(p)));
        // A trailing `\r` joined to a leading `\n` of the next piece normalises
        // differently from the pieces apart, so compare only when they agree.
        if built.to_string() == whole.to_string() {
            prop_assert!(built == whole);
        }
        let other = whole.append(&Text::of_in("x"));
        prop_assert!(other != whole);
    }

    /// Swapping two characters keeps every summary equal, so only the content
    /// comparison can tell the documents apart.
    #[test]
    fn eq_compares_content_under_equal_summaries(
        src in source(300),
        i in any::<prop::sample::Index>(),
        j in any::<prop::sample::Index>(),
        split in any::<prop::sample::Index>(),
    ) {
        let model = normalise(&src);
        let mut chars: Vec<char> = model.chars().collect();
        prop_assume!(!chars.is_empty());
        let (i, j) = (i.index(chars.len()), j.index(chars.len()));
        chars.swap(i, j);
        let other: String = chars.iter().collect();
        let a = SmallText::of_in(&model);
        // Build `b` from two halves so its leaves cut at different places.
        let cut = byte_of(&other, boundaries(&other)[split.index(chars.len() + 1)]);
        let b = SmallText::of_in(&other[..cut]).append(&Text::of_in(&other[cut..]));
        prop_assert_eq!(a.root.summary, b.root.summary);
        prop_assert_eq!(a == b, model == other);
        prop_assert_eq!(b == a, model == other);
    }
}

#[test]
fn eq_with_equal_summaries() {
    assert_ne!(Text::of("ab"), Text::of("ba"));
    assert_ne!(Text::of("a\nb"), Text::of("b\na"));
    assert_ne!(Text::of("ก😀"), Text::of("😀ก"));
    assert_eq!(Text::of("a\r\nb"), Text::of("a\nb"));
}

#[test]
fn of_normalises_line_breaks() {
    assert_eq!(Text::of("a\r\nb\rc\nd").to_string(), "a\nb\nc\nd");
    assert_eq!(Text::of("a\r\nb\rc\nd").lines(), 4);
    assert_eq!(Text::of("\r\r\n\n").lines(), 4);
    assert_eq!(Text::of("").lines(), 1);
    assert_eq!(Text::empty(), Text::of(""));
}

#[test]
fn thai_is_one_unit_and_emoji_two() {
    let text = Text::of("ก😀");
    assert_eq!(text.len(), 3);
    assert_eq!(text.line_at(1).map(|l| l.number), Ok(1));
    assert_eq!(text.line_at(2), Err(PosError::InsideSurrogate { pos: 2 }));
    assert_eq!(
        text.line_at(4),
        Err(PosError::OutOfRange { pos: 4, len: 3 })
    );
}

#[test]
fn line_at_a_line_end_is_that_line() {
    let text = Text::of("ab\n😀c");
    assert_eq!(
        text.line(2),
        Some(Line {
            number: 2,
            from: 3,
            to: 6,
            text: "😀c".into()
        })
    );
    assert_eq!(text.line_at(2).unwrap().number, 1);
    assert_eq!(text.line_at(3).unwrap().number, 2);
}

#[test]
fn replace_refuses_bad_ranges() {
    let text = Text::of("ก😀b");
    let x = Text::of("x");
    assert_eq!(
        text.replace(2, 3, &x).err(),
        Some(PosError::InsideSurrogate { pos: 2 })
    );
    assert_eq!(
        text.replace(3, 1, &x).err(),
        Some(PosError::Reversed { from: 3, to: 1 })
    );
    assert_eq!(
        text.replace(0, 9, &x).err(),
        Some(PosError::OutOfRange { pos: 9, len: 4 })
    );
}

/// A document of more than `MAX_CHILDREN²` full leaves is at least three
/// levels deep; scattered small edits and one large insert keep it valid.
fn stays_balanced<S: Sizes>() {
    let line = "ที่ 😀 word\r\n";
    let repeats = 2 * S::MAX_CHUNK * S::MAX_CHILDREN * S::MAX_CHILDREN / line.len() + 1;
    let source = line.repeat(repeats);
    let text = Text::<S>::of_in(&source);
    tree::check::<S>(&text.root);
    assert!(text.root.height >= 3, "height {}", text.root.height);
    let mut edited = text.clone();
    for i in 0..300 {
        let pos = (i * 7919) % edited.len();
        let pos = if edited.line_at(pos).is_err() {
            pos - 1
        } else {
            pos
        };
        let to = (pos + 3).min(edited.len());
        edited = edited.replace(pos, to, &Text::of_in("ก")).unwrap_or(edited);
        tree::check::<S>(&edited.root);
    }
    let big = Text::<S>::of_in(&"x".repeat(S::MAX_CHUNK * 50));
    let spliced = edited.replace(10, 10, &big).unwrap();
    tree::check::<S>(&spliced.root);
    assert_eq!(spliced.len(), edited.len() + S::MAX_CHUNK * 50);
    assert_eq!(text.to_string(), normalise(&source));
}

#[test]
fn large_documents_stay_balanced() {
    stays_balanced::<Small>();
    stays_balanced::<Release>();
}

/// xorshift64*: a deterministic rng for the stress run.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as usize % n.max(1)
    }

    /// A raw source of at least `bytes` bytes drawn from `PIECES`.
    fn source(&mut self, bytes: usize) -> String {
        let mut s = String::with_capacity(bytes + 16);
        while s.len() < bytes {
            s.push_str(PIECES[self.below(PIECES.len())]);
        }
        s
    }
}

/// `pos`, moved back off a low surrogate onto a code-point boundary.
fn boundary(units: &[u16], pos: usize) -> usize {
    let low = pos < units.len() && (0xDC00..0xE000).contains(&units[pos]);
    if low { pos - 1 } else { pos }
}

fn utf16(s: &str) -> Vec<u16> {
    normalise(s).encode_utf16().collect()
}

/// A multi-MB document at release sizes through 3000 random edits: small
/// typing, large inserts and deletes, appends and slices. The model is UTF-16
/// units, so positions index it directly. Every version stays alive, which
/// lets `check_shared` verify only the nodes each edit created.
#[test]
fn release_sizes_survive_a_long_edit_run() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let source = rng.source(3_000_000);
    let mut text = Text::of(&source);
    let mut model = utf16(&source);
    let mut seen = HashSet::new();
    tree::check_shared::<Release>(&text.root, &mut seen);
    assert!(text.root.height >= 3, "height {}", text.root.height);
    let mut versions = vec![text.clone()];
    let mut snapshots = vec![(text.clone(), model.clone())];
    for step in 0..3000 {
        let len = model.len();
        let from = boundary(&model, rng.below(len + 1));
        let (edit_at, edit_len) = match step % 200 {
            97 => {
                let size = rng.below(50_000);
                let insert = rng.source(size);
                text = text.append(&Text::of(&insert));
                let ins = utf16(&insert);
                model.extend_from_slice(&ins);
                (len, ins.len())
            }
            199 => {
                let to = boundary(&model, len - rng.below(len / 50 + 1));
                let from = boundary(&model, rng.below(len / 50 + 1)).min(to);
                text = text.slice(from, to).unwrap();
                model = model[from..to].to_vec();
                (0, 0)
            }
            _ => {
                let span = match rng.below(20) {
                    0 => rng.below(len / 20 + 1),
                    1..=4 => 0,
                    _ => rng.below(40),
                };
                let to = boundary(&model, (from + span).min(len));
                let size = match rng.below(20) {
                    0 => 20_000 + rng.below(180_000),
                    1 => 0,
                    _ => rng.below(30),
                };
                let insert = rng.source(size);
                text = text.replace(from, to, &Text::of(&insert)).unwrap();
                let ins = utf16(&insert);
                let n = ins.len();
                model.splice(from..to, ins);
                (from, n)
            }
        };
        tree::check_shared::<Release>(&text.root, &mut seen);
        assert_eq!(text.len(), model.len(), "step {step}");
        let len = model.len();
        let lo = boundary(&model, edit_at.saturating_sub(50).min(len));
        let hi = boundary(&model, (edit_at + edit_len + 50).min(len));
        let around = text.slice_string(lo, hi).unwrap();
        assert_eq!(
            around.encode_utf16().collect::<Vec<_>>(),
            model[lo..hi],
            "step {step}"
        );
        versions.push(text.clone());
        if step % 500 == 250 {
            snapshots.push((text.clone(), model.clone()));
        }
    }
    let lines = model.iter().filter(|&&u| u == u16::from(b'\n')).count() + 1;
    assert_eq!(text.lines(), lines);
    assert!(text.len() > 1_000_000, "document shrank to {}", text.len());
    let fresh = Text::of(&String::from_utf16(&model).unwrap());
    assert!(text == fresh);
    for (version, model) in &snapshots {
        assert_eq!(
            &version.to_string().encode_utf16().collect::<Vec<_>>(),
            model
        );
    }
}
