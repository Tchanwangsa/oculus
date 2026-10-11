# Markdown parser benchmarks

`cargo bench --bench markdown` against `bun editor-core/oracle/markdown-bench.ts`
(from `app/`): the app's Lezer configuration on the same note, with the same
keystrokes (typing `x` after the first character of a prose line), Lezer
reparsing through `TreeFragment` reuse as CodeMirror does.

The note is `benches/synthetic-note.md`: 200,768 UTF-16 units and 4,394 lines
of headings, prose with inline marks and maths, lists, tasks, quotes, tables,
display maths and code fences, after a YAML frontmatter block. The 30 KB note
is its lines before unit 30,000.

Measured 2026-10-10 on an Apple M5 Max with the app running. Timings vary up
to about 2× between runs on this machine, so each Rust cell gives the range
of criterion's means over two runs.

| Case | Rust | Lezer (bun) |
|---|---|---|
| Full parse, 200 K units | 3.7–5.0 ms | 14.1 ms |
| Keystroke mid-note (past the 64 KiB frontmatter window) | 44–123 µs | 221 µs |
| Keystroke at unit ~1,000 (inside the window) | 54–113 µs | 194 µs |
| Keystroke mid-note, 30 KB note | 9–19 µs | 41 µs |

A reparse in Rust copies the document text from the restart line to the end
and rebuilds the flat node array; both are linear in the document but
cheap next to the parse itself.
