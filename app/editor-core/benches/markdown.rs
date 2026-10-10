//! The markdown parser on `synthetic-note.md` (written by
//! `oracle/markdown-bench.ts`, which times Lezer on the same cases): a full
//! parse, and one-keystroke reparses in the middle of the note (past the
//! frontmatter window), near its start (inside the window), and in the
//! middle of a 30 KB note cut from it. Results: `BENCHMARKS.md`.

use criterion::{Criterion, criterion_group, criterion_main};
use oculus_editor_core::markdown::{Tree, parse, reparse};
use oculus_editor_core::text::{ChangeSet, ChangeSpec, Text};

const NOTE: &str = include_str!("synthetic-note.md");

/// The position after the first char of the first prose line at or after
/// line `line` (as in `oracle/markdown-bench.ts`).
fn keystroke_at(doc: &Text, mut line: usize) -> usize {
    while !doc
        .line(line)
        .unwrap()
        .text
        .starts_with(|c: char| c.is_ascii_lowercase())
    {
        line += 1;
    }
    doc.line(line).unwrap().from + 1
}

/// A document, its tree, and the next document after typing `x` at `at`.
fn keystroke(doc: &Text, at: usize) -> (Tree, Text, ChangeSet) {
    let changes = ChangeSet::of(&[ChangeSpec::insert(at, "x")], doc.len()).unwrap();
    (parse(doc), changes.apply(doc).unwrap(), changes)
}

fn bench(c: &mut Criterion) {
    let doc = Text::of(NOTE);
    c.bench_function("full parse", |b| b.iter(|| parse(&doc)));

    let middle = keystroke(&doc, keystroke_at(&doc, doc.lines() / 2));
    c.bench_function("keystroke, middle (past the window)", |b| {
        b.iter(|| reparse(&middle.0, &middle.1, &middle.2))
    });

    let near = keystroke(&doc, keystroke_at(&doc, doc.line_at(1000).unwrap().number));
    c.bench_function("keystroke, inside the frontmatter window", |b| {
        b.iter(|| reparse(&near.0, &near.1, &near.2))
    });

    // The lines before the one holding unit 30 000.
    let short = doc.slice(0, doc.line_at(30_000).unwrap().from - 1).unwrap();
    let typical = keystroke(&short, keystroke_at(&short, short.lines() / 2));
    c.bench_function("keystroke, middle of a 30 KB note", |b| {
        b.iter(|| reparse(&typical.0, &typical.1, &typical.2))
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
