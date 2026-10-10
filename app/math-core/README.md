# math-core

The app's maths engine, in Rust: a Cargo workspace of its own, never a
dependency of `app/src-tauri` (so `tauri dev` never rebuilds it).

| Member | What it is |
| --- | --- |
| `katex/` | A vendored fork of katex-rs, a Rust port of KaTeX 0.18.5. Where it came from and what we changed: [`katex/UPSTREAM.md`](katex/UPSTREAM.md). |
| `wasm/` | `oculus-math`, the fork's WebAssembly binding for the app: `renderToString` and `parseError` (below). |
| `oracle/` | The display oracle: `src/main.rs` (the `oracle` bin) renders JSON-lines requests with the fork; `render.ts` renders the same formulas with the app's `katex` and diffs the two (`corpus.ts` the inputs, `sets.ts` the call sites' options, `engines.ts` the renderers). |

Where the fork still differs from KaTeX JS, and why that is accepted:
[`DIVERGENCES.md`](DIVERGENCES.md).

## Tests

```sh
cd app/math-core
cargo test
```

Format our crates with `cargo fmt -p oculus-math -p oracle` (CI checks it);
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
skips while the `.wasm` is newer than every input. It needs the
`wasm32-unknown-unknown` target and the `wasm-bindgen` CLI at exactly the
version in `Cargo.lock`. `bun run build` and `bun run test` run it first.

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

A panic traps (`console_error_panic_hook` logs it first), and so does a stack
overflow from deep nesting (a few hundred levels; KaTeX JS goes deeper). A
trapped instance is unusable: instantiate the module again.

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
  empty cell) and `\text{}` draw a placeholder: `<span class="mord amsrm
  oc-placeholder">□</span>` (AMS `\square`, so layouts build around real
  metrics) with a zero-width range just inside the braces. Styling it is the
  view's. A group the parser makes up (`aligned`'s spacing `{}`) has no
  location and no placeholder.

`katex/src/source_map.rs` holds the mapping; `katex/tests/source_map.rs` and
the oracle's `--source-map` check it.

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

With it the corpus is 4,732 formulas; without it, 4,101. The
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
```

Options take KaTeX JS's names and defaults (`strict` defaults to `"warn"`),
plus `sourceMap`.
