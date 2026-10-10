# Editor core

A Rust port of the note editor's text model, undo history and markdown parser,
held equal to CodeMirror and Lezer by oracles, and run beside every note
editor in dev builds as a check. The editor itself is [editor.md](./editor.md).

## Where

| Piece | Location |
| --- | --- |
| The crate: text, changes, selections, history, the markdown parser | `app/editor-core/src/` |
| Its WebAssembly bridge (`Shadow`) | `app/editor-core/wasm/` |
| Oracles against CodeMirror and Lezer | `app/editor-core/oracle/` |
| Where the parser parts from the spec; timings | `app/editor-core/DIVERGENCES.md`, `app/editor-core/BENCHMARKS.md` |
| Builds the wasm and its JS glue | `app/scripts/build-editor-wasm.mjs` |
| Shadow mode | `app/src/components/documents/editor/shadow/` |

## A Rust editor core is built beside the editor, not in it

`app/editor-core/` is a standalone crate (`oculus-editor-core`) that ports
CodeMirror's text model — a rope counted in UTF-16 units, `ChangeSet`,
selections, transactions and `history()` — and the note grammar's
`@lezer/markdown` parser, incremental like Lezer's fragment reuse.
`app/src-tauri` does not depend on it; the frontend runs it only as dev
builds' shadow mode ([below](#shadow-mode-checks-the-rust-core-against-every-note-editor)),
through the `oculus-editor-core-wasm` bridge crate in `app/editor-core/wasm/`
(one workspace, one `Cargo.lock`).

The target is the app's behaviour, not a spec: oracle scripts in
`app/editor-core/oracle/` drive the real `@codemirror/state`,
`@codemirror/commands` and the app's own `noteLanguage` beside the crate's
`oracle` binary with the same seeded random operations and diff the answers;
`text`, `history` and `markdown` also run every note in the data directory. Where Lezer and the
CommonMark/GFM spec disagree, Lezer wins; each case is listed in
`app/editor-core/DIVERGENCES.md`. Timings against Lezer are in
`app/editor-core/BENCHMARKS.md`.

From `app/editor-core`: `cargo test --workspace`, `cargo bench`. From `app/`:
`bun editor-core/oracle/<text|changes|history|markdown|shadow>.ts [cases]
[seed] [only]` — a failure prints its seed and case index, which replay it alone.
Run one oracle at a time: a large `history.ts` count holds gigabytes.

## Shadow mode checks the Rust core against every note editor

In a dev build, `noteExtensions()` carries `editorShadow()`
(`app/src/components/documents/editor/shadow/index.ts`): a `StateField` holding
each state's `Shadow` — the core's document, selection, history and tree, a
value one step returns a new copy of — and a view plugin. CodeMirror stays the
editor; the shadow never changes a transaction and catches its own exceptions.

1. **It seeds from the first state it sees once the wasm has loaded**: the
   doc, the selection and the history field's runtime value, not
   `historyField.toJSON`, which drops goal columns, a mapped range's
   `from > to`, and the previous time and user event, so the next keystroke
   would group differently
   (`app/src/components/documents/editor/shadow/basis.ts`).
2. **Every transaction is mirrored**, then checked: length, the text of the
   changed ranges, the selection. A transaction with the history's own user
   event (`undo`, `redo`, `select.undo`, `select.redo`) pops the shadow's
   history instead and must produce the same changes; a selection undo that a
   transaction filter moved (the maths field's) is followed outside the history.
3. **The plugin compares undo and redo depth on every update**, and a second
   after the last edit the whole text, the history JSON and, once Lezer's tree
   covers the document, the tree, nested code pruned. A tree report says
   whether the shadow matches a fresh Lezer parse (`shadowMatchesFreshLezerParse`).

**The first mismatch in a state chain logs one
`console.error("[editor-shadow] <kind> mismatch", details)`** — expected and
actual (strings cut around their first difference) and a `replay`: the seed
and the last 50 transactions as `Shadow` calls, which `runReplay`
(`app/src/components/documents/editor/shadow/replay.ts`) rebuilds. That chain
then stops. Any wasm exception logs `[editor-shadow] error` and stops it too;
a trap stops every chain. `localStorage.setItem("oculus.editorShadow", "off")`
turns shadow mode off from the next load.

**Release builds hold none of it**: everything sits behind
`import.meta.env.DEV`, and only `index.ts` touches `import.meta`, so
`app/editor-core/oracle/shadow.ts` runs the rest headless in bun against the
real `history()` and `noteLanguage`. `bun run editor-wasm` (from `app/`, and in
`predev`) builds the wasm and its glue into the shadow folder's gitignored
`pkg/`; it needs `rustup target add wasm32-unknown-unknown` and the
`wasm-bindgen` CLI at the version `app/editor-core/wasm/Cargo.toml` pins.
Without them the console says the shadow is off and why.

## Gotchas

- **A tree report with `shadowMatchesFreshLezerParse: true` is CodeMirror's**
  — its incremental tree, not the core, parted from a fresh parse, as
  frontmatter's lookahead would make it without `frontmatterFragments`
  ([editor.md](./editor.md)). The shadow oracle fails on it all the same.
- **`noEditorShadowInBuild` in `app/vite.config.ts` looks removable and isn't**
  — Rollup loads the wasm glue from the dead dev branch anyway, and the glue's
  `new URL(wasm, import.meta.url)` would ship the wasm in a release bundle.
- **A dropped `Shadow` is freed by a `FinalizationRegistry` callback** — a loop
  that never yields to the event loop (an oracle) grows wasm memory without bound.
