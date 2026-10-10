# math-core

The app's maths engine, in Rust: a Cargo workspace of its own, never a
dependency of `app/src-tauri` (so `tauri dev` never rebuilds it).

| Member | What it is |
| --- | --- |
| `katex/` | A vendored fork of katex-rs, a Rust port of KaTeX 0.18.5. Where it came from and what we changed: [`katex/UPSTREAM.md`](katex/UPSTREAM.md). |
| `wasm/` | `oculus-math`, the WebAssembly binding for the app: the fork's `renderToString` and `parseError`, and the edit model's `MathField` (below). |
| `edit/` | `oculus-math-edit`, the visual maths field's edit model: caret stops and slots over the formula's source, the editing commands and the shortcuts ([below](#the-edit-model)). Pure Rust; the app reaches it through `MathField` (no UI uses it yet). |
| `oracle/` | The display oracle: `src/main.rs` (the `oracle` bin) renders JSON-lines requests with the fork; `render.ts` renders the same formulas with the app's `katex` and diffs the two (`corpus.ts` the inputs, `sets.ts` the call sites' options, `engines.ts` the renderers). |

Where the fork still differs from KaTeX JS, and why that is accepted:
[`DIVERGENCES.md`](DIVERGENCES.md).

## Tests

```sh
cd app/math-core
cargo test
```

It runs the binding's boundary (`wasm/src/boundary/`) natively too.

Format our crates with `cargo fmt -p oculus-math -p oculus-math-edit -p oracle` (CI checks it);
never run `cargo fmt` over `katex/` (see `katex/UPSTREAM.md`). Lint the binding
with `cargo clippy --no-deps --target wasm32-unknown-unknown -p oculus-math`.

## The wasm build

```sh
cd app
bun run math        # node scripts/build-math.mjs
```

`cargo build --profile wasm --target wasm32-unknown-unknown -p oculus-math`
(release plus one codegen unit and `panic = "abort"`), `wasm-bindgen --target
web`, then `wasm-opt -Oz` (npm `binaryen`), into the gitignored `pkg/`:
`oculus_math.js` (the glue), `oculus_math_bg.wasm` and their `.d.ts`. It
skips while the `.wasm` is newer than every input (`katex/`, `edit/` and
`wasm/` less their tests, the Cargo files). The `.wasm` is 833 KB (348 KB
gzipped; the edit model and its binding are 148 KB of it), the glue 21 KB.
It needs the `wasm32-unknown-unknown` target and the `wasm-bindgen` CLI at
exactly the version in `Cargo.lock`. `bun run build` and `bun run test` run
it first.

The module exports, after `initSync({ module })` or `await init()`:

- `renderToString(tex, options?)` — KaTeX JS's. Options are KaTeX's names and
  defaults, and only `displayMode`, `throwOnError` (default true), `strict`
  (default `"warn"`, the warning itself dropped), `macros` (name → string) and
  `output`, plus our `sourceMap` (default false; [below](#source-mapping));
  others are ignored, a wrong type throws a `TypeError`. A parse
  error throws an `Error` named `ParseError` whose `message` is KaTeX's
  (`KaTeX parse error: …`); with `throwOnError: false` it returns the red
  `katex-error` span instead.
- `parseError(tex, options?)` — the message `renderToString` would throw
  with `throwOnError` forced on, or `undefined`. It runs the whole render, so
  build-time errors count.
- `MathField` — the edit model's `Field` ([below](#the-edit-model)). Its
  offsets are **UTF-16 units** (a JS string's indices, as `data-s`/`data-e`);
  stops and slots are ids, indices into `stops()` and `slots()`.
  `MathField.open(source, display)` has the caret at the end, or throws a
  `ParseError` when the source has no stops. It reads `source`, `display`,
  `mode` (`"math"`, `"text"`, `"command"`), `pending` (the `\command` without
  its backslash), `anchor`/`head` (stop ids), `selected` (`[from, to]`),
  `spaceFree`, `stops()` (each stop's offset by stop id, a `Uint32Array`),
  `stopSlots()` (each stop's slot id) and `slots()` (`FieldSlot`s: kind, row
  and column, bounds, `text`, interior `from`/`to`, parent). Each step
  returns a new `MathField` and leaves the receiver as it was: `caretAt(offset,
  after)`, `select(anchor, head)` (widened), `withPending(name)` and
  `run(command)`, whose result's `step` says what the command did: `changes`
  in the old source, `isolate`, a shortcut's `rewrite` in the source after
  `changes` (apply each in reverse order), `effect` (`"leaveLeft"`, …,
  `"removeMaths"`). A command is named as `Command`'s variant: `{insert:
  "x"}`, `{template: "\\frac{#0}{#?}"}`, `"backspace"`, `{left: {extend:
  true}}`, `{up: xs}` (each stop's x by stop id, `NaN` unmeasured), … (the
  `.d.ts`'s `FieldCommand`). `shortcuts()` is the shortcut table, `[keys,
  LaTeX]` pairs. `wasm/src/boundary/` converts every offset (`edit/`'s
  `utf16`) and carries commands, steps and slots as JSON; `wasm/src/field.rs`
  only moves them across.

A panic traps (`console_error_panic_hook` logs it first), and so does a stack
overflow from deep nesting (a few hundred levels; KaTeX JS goes deeper). A
trapped instance is unusable: instantiate the module again. Its `MathField`s
die with it, and any call into it traps again, the `free()` that
wasm-bindgen's finalizer makes for a collected object included. The app's
facade (`app/src/lib/maths/field.ts`) reads each field out as plain data,
opens it again on the new instance, and detaches a dead object from the
glue before it is collected.

## Source mapping

`sourceMap: true` (`Settings::source_map`) is for the edit field; display
leaves it off, and its output is then byte-identical. On, the HTML maps back
to the formula:

- Each parse node's element has `data-s`/`data-e`: the node's source range as
  **UTF-16 code-unit offsets** into `tex` (a JS string's indices, CodeMirror's
  positions), `data-s` inclusive, `data-e` exclusive. A glyph gets a span of
  its own to carry them. Ranges nest: a mapped element's range lies inside its
  nearest mapped ancestor's. A node drawn as a fragment (`\color`, a style
  switch) has no element; its unmapped children take its range.
- Glyphs of different nodes are never merged (`ab` is two spans), except a
  combining mark onto its base (Thai vowels). The split glyphs lay out as the
  merged run would: same italic corrections, no glue or line break between.
- An explicit empty group (`{}`, `\frac{}{}`'s parts, `x^{}`, `\sqrt{}`, an
  empty cell), an empty optional argument (`\sqrt[]`), `\text{}` and an
  empty `\left…\right` body draw a placeholder: `<span class="mord amsrm
  oc-placeholder">□</span>` (AMS `\square`, so layouts build around real
  metrics) with a zero-width range just inside the braces or brackets (after
  `\left`'s delimiter), where the edit model's stop for that slot is. Styling
  it is the view's. A group the parser makes up (`aligned`'s spacing `{}`)
  has no location and no placeholder.

`katex/src/source_map.rs` holds the mapping; `katex/tests/source_map.rs` and
the oracle's `--source-map` check it.

## The edit model

`edit/` (`oculus-math-edit`) says where the field's caret can be and what
each key does to the source. The field edits the formula's LaTeX itself, so
a caret is a byte offset of the source in a **slot**: an ordered run of
sibling atoms (a row, a group's content, a script, a numerator, a cell, a
`\text{}` run). `stops(source, display)` parses with source mapping on and
returns every **stop** in ←/→ order, which is source order; unparseable
source returns the parse error instead (the field edits it as TeX). The
rules:

- A stop sits before a slot's first atom and after each atom; a text slot
  has one between every character too, never inside a cluster (`วั`). An
  empty slot (`{}`, `\frac{}{}`, an empty cell) has exactly one.
- A one-token argument (`\frac ab`, `x^2`) is a `Bare` slot: typing a
  second atom into it must add braces first.
- A macro's output (`\dots`, `\iff`) maps to its invocation and is one
  atom; arguments pasted into it (`\bra{x}`, `\boxed{x}`) are a slot.
- A base's scripts (`_1^2`) are one atom after the base, holding a slot
  each; a style switch (`\color{red}`, `\displaystyle`) is an atom and its
  run stays in the slot around it.
- Stops in different slots can share an offset (`\frac ab`'s 7 ends the
  numerator and starts the denominator): `stop_at` takes an `Affinity`.
- Rows split at a top-level `\\`, each starting after the break's spaces
  (a row on a line of its own types after the newline).
  `\begin{matrix}\end{matrix}` has one empty cell, before its `\end`; an
  empty cell's stop is against the token after it. A `CD` diagram is one
  atom (edited as TeX).

`utf16` converts offsets for the DOM and CodeMirror, at the binding's
boundary. `check` holds the invariants (sorted, on char boundaries, inside
their slot, round trip through offsets, a letter typed at any maths stop
outside a bare argument still renders), which `cargo test -p
oculus-math-edit` runs over hand-written cases, the fork's source-location
formulas, `oracle/fixtures.json`, every prefix of those, and random
formulas (proptest). The corpus check runs them over the notes:

```sh
cd app
bun math-core/oracle/render.ts --stops
```

It prints counts only (formulas, stops, failures by kind, errors by kind)
and the parse time per formula (mean, p50, p99), and fails on any broken
invariant.

**Commands.** A `Field` is one formula being edited: its source, stops, a
selection (anchor and head as stop indices, so a caret names one stop where
several share an offset; `caret_at(offset, affinity)` places it after an
outside change such as undo), the pending `\command` (typed after `\`,
kept outside the source and drawn by the view) and the shortcut keys just
typed. `Field::run(&Command)` takes
typed text, a template (`#0` takes the selection, `#?` is an empty slot),
pasted LaTeX, Backspace, Delete, ⌘Backspace, ←/→ (Shift extends), ↑/↓ (the
view passes each stop's x), Home/End, select all, Tab, Shift+Tab, Enter and
Esc, and returns an `Outcome`: the change in bytes of the old source, an
`isolate` flag (that change is its own undo step), a shortcut's `rewrite`
(below), the new field and an effect for the view (`Leave(direction)`,
`RemoveMaths`). `mode()` (maths, text, command) and
`space_free()` answer the view's questions. Every command keeps the source
rendering (an edit that would break it does nothing), spaces a control word
off a following letter, braces a bare argument before a second atom (`x^2`
→ `x^{23}`) and leaves `{}` when one is emptied; Backspace right after a
typed character or template gives the source back, except where braces or
a control word's space went in with it. The rules for each key are on the
functions in `edit/src/command/`.

Matrices are typed as in MATLAB (`edit/src/command/grid/`). The grid at the caret is the cell of a
`matrix`, `pmatrix`, `bmatrix`, `Bmatrix`, `vmatrix`, `Vmatrix` or
`smallmatrix` it is directly in, or the body of a bracket group whose
brackets draw one: a `\left…\right` pair, or an opening bracket atom
(`(`, `[`, `\{`, `|`, `\|` and their control words) with its closer later
in the slot, else through the slot's end. Space after a term ends the cell
(into an empty next cell, else a new column taking what followed the
caret; at most ten); a lone binary operator or relation rejoins the cell
before (`[a + b]` is one cell); `;` goes to a new or empty next row;
Backspace in an empty cell takes its column, else its row, when all
empty; a matrix's closing key (`)`, `]`, `}`, `|` for `pmatrix`,
`bmatrix`, `Bmatrix`, `vmatrix`) trims empty trailing rows and columns
and puts the caret after it and its scripts. A group becomes its matrix
at its first new cell or row (scripts on its closer stay, `[a b]^T`); a
matrix left with one cell becomes its bracket group again, open (closed
when scripts follow it, or by the closing key). Rows are padded to the
widest first. Each edit is one change and its own undo step (`isolate`),
and splices the source: cells and separators it does not touch keep
their text, new `&`s and `\\`s copy the matrix's own, and a matrix that
is a whole display formula goes one row per line (`\begin{env}`, `row
\\` lines, `\end{env}`), as Enter's new rows do. Empty cells are stored
empty; a one-column matrix's empty last row is kept with a `\\` after it
(KaTeX drops it otherwise). `&` in any array cell is a raw `&` (a new
cell), `\&` elsewhere. `space_free()` is false where Space is a grid key.

**Shortcuts** (`edit/src/shortcut/`) expand typed keys: `sin` → `\sin`,
`->` → `\to`, `@a` → `\alpha`, `xsr` → `x^2`, `sqrt` → `\sqrt{}` with the
caret in it. The table (`table.rs`, exported as `SHORTCUTS`) holds the
inline shortcuts and the note's shorthands. Only one character typed in
maths takes part: never in text, in a pending `\command`, in a font's or
`\operatorname`'s argument, nor from an IME's string, a template or a
paste. A letter key expands only
when the whole run of single-letter atoms before the caret is the key
(`xsin` and `card` stay; `2pi` is `2\pi`, `sintheta` is `\sin\theta`); a
power (`sr`, `cb`, `rd`, `invs`) takes the letter or operand before it as
its base. A key starting with a symbol matches the keys just typed,
whatever they built on the way (`^^` is `\wedge`); `!=` after an operand
stays a factorial. A longer key expands again from the field before its
first key (`sin`, then `h`, is `\sinh`; `<=`, then `>`, `\iff`). The
typed key lands as `changes` (joining the typing run) and the expansion is
the outcome's `rewrite`: one change in bytes of the source after
`changes`, its own undo step. Esc right after an expansion puts the keys
back as typed (one change, `isolate`); the keys typed next that still lead
to a longer shortcut then stay as typed (`sin`, Esc, `h` is `sinh`). Any
other command ends the run.

`cargo test -p oculus-math-edit` runs a behaviour table per documented key
(`tests/commands/`, in a marker notation described in its `harness.rs`) and
the command invariants (`check::commands`; among them, Esc after any
expansion gives back exactly what typing its keys alone would) over a fixed
run on every test formula and 10,000 random command sequences (proptest;
`PROPTEST_CASES=100000` runs 10⁵). A pick of a run is a command, a
shortcut's keys (then Esc, for half of them) or a click. The corpus run
applies a fixed pseudo-random sequence of 60 picks to every note formula:

```sh
cd app
bun math-core/oracle/render.ts --commands
```

It prints counts only (formulas, picks, edits, insertions undone, shortcuts
reverted, failures by kind) and fails on any broken invariant or panic.

## The oracle

```sh
cd app
bun math-core/oracle/render.ts [--engine native|wasm] [--katex <KaTeX checkout>] [--out <report.json>]
bun math-core/oracle/render.ts --prefixes [--source-map]
bun math-core/oracle/render.ts --source-map [--engine native|wasm]
```

It builds `oracle` (`cargo build --release --bin oracle`), or with `--engine
wasm` the wasm build, collects every
`$$…$$` and `$…$` from the `.md` files under the app's data directory, KaTeX's
pinned fixtures (`katex/tests/fixtures/upstream.json`) and our synthetic cases
(`oracle/fixtures.json`), renders each with both engines under the app's five
call-site option sets, and prints the counts per set and per difference class.
`--katex` adds KaTeX's spec inputs from a checkout at the tracked commit;
without it, `target/katex` is used when it exists (gitignored, like all of
`target/`). To make it:

```sh
cd app/math-core
git clone https://github.com/KaTeX/KaTeX target/katex
git -C target/katex checkout 49904aa2b6c5d82ba0c5a1bc3a4d9b3353a1401c
```

With it the corpus is 4,732 formulas; without it, 4,106. The
run fails on any difference `DIVERGENCES.md` doesn't accept. The wasm engine
also fails on a thrown error that isn't KaTeX's `ParseError` shape, on
`parseError` disagreeing with `renderToString`, and on a trap.

`--prefixes` is the typing probe: the native bin renders every char-boundary
prefix of every formula with the parse gate's options and reports panics (a
wasm panic would trap, so this runs natively); `--prefixes --source-map`
probes with `sourceMap` on, as the edit field renders.

`--source-map` is the source-map property check (`sourcemap.ts`): either
engine renders every formula with the parse gate's options, `sourceMap` off
and on, and it counts the formulas failing each check, grouped by the kind of
node or character that fails, with the shortest example of each:

- a: the flag changes no error or panic, and no error's kind (its message
  without the position and context); an error that only moves is counted
  apart, not failed (with the flag on, an error on a macro body's token
  points at the invocation);
- b: every `data-s`/`data-e` pair is well-formed, within the formula, and
  inside its nearest mapped ancestor's range;
- c: every visible glyph has a mapped ancestor-or-self;
- d: every non-space source character lies in a mapped leaf (an element with
  no mapped descendants), except syntax, counted per reason: braces, `^`
  `_` `&` `$`, `\\` and its `[size]`, `\begin{name}`/`\end{name}`, the
  command of a structure (an element with mapped children), `\left`/
  `\right`/`\big` with the delimiter it draws, a structure's parameter (a
  group in it with no mapped range inside: a size, a colour, a column spec,
  `\smash[t]`), a structure's bare-token parameters (`\genfrac ( ]`'s
  delimiters, bar size and style; `\above1.0pt`'s dimension), a
  structure's optional-argument brackets, CD arrow syntax,
  macro definitions, commands that draw nothing of their own (style and size
  switches, `\color{…}`, `\phantom`, `\nonumber`), and a font or colour
  command drawn as a fragment of its argument (`\mathbf{\hbox{…}}`,
  `\blue{x}`), which has no element;
- e: the flag-on HTML without its ranges and placeholders is the flag-off
  HTML, merged glyph runs against their split glyphs aside; formulas with a
  placeholder are counted apart (it has height).

The report (`--out`, default `$TMPDIR/katex-oracle-report.json`) holds the
user's formulas: keep it outside the repo. A difference found there becomes a
synthetic case in `oracle/fixtures.json`, never a copied formula.

The bin's protocol, one JSON object per line:

```text
in:  {"id": 1, "tex": "x^2", "display": false, "options": {"throwOnError": true, "strict": "ignore", "macros": {"\\foo": "x"}, "output": "htmlAndMathml"}}
out: {"id": 1, "html": "<span class=\"katex\">…"}   or   {"id": 1, "error": "KaTeX parse error: …"}   or   {"id": 1, "panic": "…"}
out with --prefixes: {"id": 1, "prefixes": 3, "panics": [{"len": 2, "panic": "…"}]}
out with --stops:    {"id": 1, "stops": 4, "slots": 2, "between": 0, "failures": [{"kind": "…", "offset": 3}], "parseNs": 5600}
                  or {"id": 1, "error": "KaTeX parse error: …", "parseNs": 900}
out with --commands: {"id": 1, "steps": 60, "edits": 25, "restores": 9, "reverts": 2, "failures": [{"kind": "…", "step": 12}]}
                  or {"id": 1, "error": "KaTeX parse error: …"}
```

Options take KaTeX JS's names and defaults (`strict` defaults to `"warn"`),
plus `sourceMap`.
