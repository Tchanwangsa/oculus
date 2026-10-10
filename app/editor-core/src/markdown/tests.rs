use proptest::prelude::*;

use super::{NodeType, Tree, parse, parse_str, reparse};
use crate::text::{ChangeSet, ChangeSpec, Text};

/// `name from to` per node, in pre-order.
fn dump(tree: &Tree) -> Vec<String> {
    tree.iter()
        .map(|n| format!("{} {} {}", n.name(), n.from(), n.to()))
        .collect()
}

#[test]
fn paragraph_with_emphasis() {
    let tree = parse_str("a *b* c");
    assert_eq!(
        dump(&tree),
        [
            "Document 0 7",
            "Paragraph 0 7",
            "Emphasis 2 5",
            "EmphasisMark 2 3",
            "EmphasisMark 4 5"
        ]
    );
}

#[test]
fn utf16_positions() {
    // Thai is one unit per char, the emoji two.
    let tree = parse_str("ไทย 😀 **x**");
    assert_eq!(dump(&tree)[2], "StrongEmphasis 7 12");
}

#[test]
fn node_names_are_lezers() {
    assert_eq!(NodeType::ALL[1].name(), "Document");
    assert_eq!(NodeType::ALL.len(), 59);
    for (id, t) in NodeType::ALL.iter().enumerate() {
        assert_eq!(*t as usize, id);
    }
}

#[test]
fn resolve_inner_side_bias() {
    let tree = parse_str("a *b* c");
    assert_eq!(tree.resolve_inner(2, -1).name(), "Paragraph");
    assert_eq!(tree.resolve_inner(2, 1).name(), "EmphasisMark");
    assert_eq!(tree.resolve_inner(5, -1).name(), "EmphasisMark");
    assert_eq!(tree.resolve_inner(3, 0).name(), "Emphasis");
    let mark = tree.resolve_inner(2, 1);
    assert_eq!(mark.parent().unwrap().name(), "Emphasis");
    assert_eq!(mark.parent().unwrap().parent().unwrap().name(), "Paragraph");
}

const PIECES: &[&str] = &[
    "a",
    "word ",
    " ",
    "  ",
    "\t",
    "\n",
    "\n\n",
    "> ",
    ">",
    "- ",
    "* ",
    "1. ",
    "2) ",
    "#",
    "# ",
    "```",
    "~~~",
    "    ",
    "*",
    "**",
    "_",
    "__",
    "~~",
    "`",
    "[",
    "]",
    "(",
    ")",
    "![",
    "<",
    ">",
    "&amp;",
    "\\",
    "$",
    "$$",
    "\\(",
    "\\)",
    "\\[",
    "\\]",
    "|",
    "---",
    "===",
    "[ ] ",
    "[x] ",
    "www.a.com",
    "http://b.c/d",
    "a@b.co",
    "<div>",
    "<!--",
    "-->",
    "ไทย",
    "😀",
    "é",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// Every node lies within its parent, children are in order and don't
    /// overlap, and the root spans the document.
    #[test]
    fn tree_is_well_formed(parts in proptest::collection::vec(proptest::sample::select(PIECES), 0..40)) {
        let src: String = parts.concat();
        let tree = parse_str(&src);
        let root = tree.root();
        prop_assert_eq!(root.name(), "Document");
        prop_assert_eq!(root.from(), 0);
        prop_assert_eq!(root.to(), src.encode_utf16().count());
        for node in tree.iter() {
            prop_assert!(node.from() <= node.to());
            let mut prev_end = node.from();
            for child in node.children() {
                prop_assert_eq!(child.parent(), Some(node));
                prop_assert!(child.from() >= prev_end, "{:?} starts before its sibling ends", child);
                prop_assert!(child.to() <= node.to(), "{:?} ends after {:?}", child, node);
                prev_end = child.to();
            }
        }
    }
}

/// Parses `src` on a thread with a 2 MB stack (a browser worker's order of
/// size), so recursion per nesting level would overflow; then reparses
/// after a keystroke at the end and drops both trees there.
fn parse_on_small_stack(src: String) -> (usize, usize) {
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(move || {
            let doc = Text::of(&src);
            let tree = parse(&doc);
            let changes = ChangeSet::of(&[ChangeSpec::insert(doc.len(), "a")], doc.len()).unwrap();
            let next = changes.apply(&doc).unwrap();
            let again = reparse(&tree, &next, &changes);
            assert_eq!(again.root().to(), tree.root().to() + 1);
            (tree.len(), tree.root().to())
        })
        .unwrap()
        .join()
        .unwrap()
}

#[test]
fn deep_nesting_does_not_overflow() {
    // A rule, then (with text after it) 100k nested list items.
    let (nodes, _) = parse_on_small_stack("- ".repeat(100_000));
    assert_eq!(nodes, 2);
    let (nodes, len) = parse_on_small_stack("- ".repeat(100_000) + "a");
    assert_eq!(len, 200_001);
    assert!(nodes > 200_000);
    let (nodes, len) = parse_on_small_stack("> ".repeat(500_000));
    assert_eq!(len, 1_000_000);
    assert!(nodes > 500_000);
    // Inline nesting with a quote mark carried down into it.
    let src = format!("> {}a\n> b{}", "![".repeat(50_000), "](u)".repeat(50_000));
    parse_on_small_stack(src);
}

#[test]
fn code_text_can_start_between_surrogates() {
    // The second line's code starts at column 10, which falls between the
    // halves of 𝒳 (units 15..17); Lezer starts the CodeText at 16.
    let tree = parse_str(">-     \n>\t    *𝒳");
    assert!(dump(&tree).contains(&"CodeText 16 17".to_string()));
}

/// A ChangeSet of up to three sorted, disjoint replacements of `doc`, picked
/// by `picks` among its code-point boundaries.
/// Whole lines that open, close, continue or interrupt blocks.
const LINES: &[&str] = &[
    "a|b",
    "|-|-|",
    "-|-",
    "| x |",
    "para",
    "para *x",
    "===",
    "---",
    "- - -",
    "```",
    "~~~",
    "$$",
    "$$ x $$",
    "\\[",
    "\\]",
    "> q",
    ">",
    "- li",
    "1. x",
    "  - y",
    "    code",
    "<div>",
    "</div>",
    "<!--",
    "-->",
    "<?x",
    "?>",
    "[r]: /u",
    "[r]:",
    "'t'",
    "\"t\"",
    "",
    "",
    "# h",
    "***",
    "- [ ] t",
    "...",
    "<script>",
    "</script>",
];

/// A document of lines, each a block line or a run of inline pieces.
fn lines_doc() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            proptest::sample::select(LINES).prop_map(str::to_string),
            proptest::collection::vec(proptest::sample::select(PIECES), 0..4)
                .prop_map(|p| p.concat()),
        ],
        0..40,
    )
    .prop_map(|lines| lines.join("\n"))
}

/// Markup characters a keystroke toggles.
const TOGGLES: &[&str] = &[
    ">", "-", "|", "`", "$", "=", "#", "*", "_", "[", "]", "\n", " ", "~", "<", "\\", ":", "1.",
    "\t",
];

/// An insertion: inline pieces, a line with breaks around it, or one
/// markup character.
fn insert_text(k: usize) -> String {
    match k % 6 {
        0 => PIECES[k / 6 % PIECES.len()].to_string(),
        1 => format!("\n{}", LINES[k / 6 % LINES.len()]),
        2 => format!("{}\n", LINES[k / 6 % LINES.len()]),
        3 | 4 => TOGGLES[k / 6 % TOGGLES.len()].to_string(),
        _ => String::new(),
    }
}

/// A position biased toward the edges where block structure turns: the
/// start, the end, line starts and line ends.
fn biased_pos(s: &str, bounds: &[usize], line_starts: &[usize], pick: u16) -> usize {
    let n = pick as usize >> 3;
    match pick % 8 {
        0 => 0,
        1 => *bounds.last().unwrap(),
        2 | 3 => line_starts[n % line_starts.len()],
        4 => {
            // The end of a line: before its break, or the document's end.
            let start = line_starts[n % line_starts.len()];
            let units: Vec<u16> = s.encode_utf16().collect();
            let mut end = start;
            while end < units.len() && units[end] != b'\n' as u16 {
                end += 1;
            }
            end
        }
        _ => bounds[n % bounds.len()],
    }
}

/// A ChangeSet of up to three sorted, disjoint edits of `doc`: insertions,
/// one-char deletions or ranges, at biased positions.
fn changes_for(doc: &Text, picks: &[(u16, u16, usize)]) -> ChangeSet {
    let s = doc.to_string();
    let mut bounds = vec![0];
    let mut line_starts = vec![0];
    let mut at = 0;
    for c in s.chars() {
        at += c.len_utf16();
        bounds.push(at);
        if c == '\n' {
            line_starts.push(at);
        }
    }
    let next_bound = |p: usize| bounds[bounds.partition_point(|&b| b <= p).min(bounds.len() - 1)];
    let mut ranges: Vec<(usize, usize, usize)> = picks
        .iter()
        .map(|&(a, b, k)| {
            let from = biased_pos(&s, &bounds, &line_starts, a);
            let to = match b % 4 {
                0 | 1 => from,
                2 => next_bound(from),
                _ => from.max(bounds[(b as usize >> 2) % bounds.len()]),
            };
            (from, to, k)
        })
        .collect();
    ranges.sort_unstable();
    // Keep them disjoint: drop any that starts inside the previous one.
    let mut specs = Vec::new();
    let mut last_end = 0;
    for (i, &(from, to, k)) in ranges.iter().enumerate() {
        if i > 0 && from < last_end {
            continue;
        }
        specs.push(ChangeSpec::replace(from, to, &insert_text(k)));
        last_end = to.max(from + 1);
    }
    ChangeSet::of(&specs, doc.len()).unwrap()
}

fn edit_steps() -> impl Strategy<Value = Vec<Vec<(u16, u16, usize)>>> {
    proptest::collection::vec(
        proptest::collection::vec((any::<u16>(), any::<u16>(), any::<usize>()), 1..4),
        1..6,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10000))]

    /// The stage's invariant: reparsing after any edits equals a fresh parse.
    #[test]
    fn reparse_equals_fresh_parse(
        src in prop_oneof![
            lines_doc(),
            proptest::collection::vec(proptest::sample::select(PIECES), 0..80).prop_map(|p| p.concat()),
            lines_doc().prop_map(|d| format!("---\na: b\n{d}")),
        ],
        steps in edit_steps(),
    ) {
        let mut doc = Text::of(&src);
        let mut tree = parse(&doc);
        for picks in &steps {
            let changes = changes_for(&doc, picks);
            doc = changes.apply(&doc).unwrap();
            tree = reparse(&tree, &doc, &changes);
            let fresh = parse(&doc);
            prop_assert_eq!(dump(&tree), dump(&fresh), "after editing to {:?}", doc.to_string());
            prop_assert!(tree == fresh, "same nodes, different links, after editing to {:?}", doc.to_string());
        }
    }
}

#[test]
fn reparse_keeps_blocks_around_an_edit() {
    let src: String = (0..300)
        .map(|i| format!("para {i} *x*\n\n> q {i}\n\n"))
        .collect();
    let doc = Text::of(&src);
    let tree = parse(&doc);
    let at = src.len() / 2;
    let changes = ChangeSet::of(&[ChangeSpec::insert(at, "**")], doc.len()).unwrap();
    let new = changes.apply(&doc).unwrap();
    assert_eq!(dump(&reparse(&tree, &new, &changes)), dump(&parse(&new)));
}

/// Reparses `old` after inserting `insert` at `at` and checks it against a
/// fresh parse.
fn check_insert(old: &str, at: usize, insert: &str) {
    let doc = Text::of(old);
    let tree = parse(&doc);
    let changes = ChangeSet::of(&[ChangeSpec::insert(at, insert)], doc.len()).unwrap();
    let new = changes.apply(&doc).unwrap();
    let fresh = parse(&new);
    let tree = reparse(&tree, &new, &changes);
    assert_eq!(dump(&tree), dump(&fresh));
    assert!(tree == fresh);
}

#[test]
fn frontmatter_is_never_reused_away_from_the_start() {
    check_insert("---\na\n---\nx", 0, "p\n\n");
    check_insert("---\n---\n", 0, "\n---\n");
}

#[test]
fn reparse_inside_the_frontmatter_window() {
    let body: String = (0..400).map(|i| format!("para {i}\n\n")).collect();
    let src = format!("---\ntitle: x\n---\n{body}");
    check_insert(&src, 6, "y");
    check_insert(&src, 2000, "*");
    check_insert(&src, 0, "x");
    check_insert(&src, 3, "-");
}
