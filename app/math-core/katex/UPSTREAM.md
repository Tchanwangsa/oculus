# Upstream

This directory is a vendored fork of katex-rs's `katex` crate.

| | |
| --- | --- |
| Repository | https://github.com/katex-rs/katex-rs |
| Tag | `katex-rs-v0.3.0` |
| Commit | `f9d93892c39424ba24cef0be18105a1629b66357` (2026-09-05) |
| Path in upstream | `crates/katex` (+ the repo's `LICENSE`, MIT) |
| Tracks | KaTeX 0.18.5, KaTeX commit `49904aa2b6c5d82ba0c5a1bc3a4d9b3353a1401c` (the upstream `KaTeX` submodule's gitlink; `.gitmodules` still names an older `785315c`) |
| Vendored | 2026-10-10 |

The package keeps upstream's name (`katex-rs`) and library name (`katex`);
`use katex::…` works as upstream documents. Upstream formatting is kept: never
run `cargo fmt` over this directory, so a diff against upstream shows only our
changes.

## Left out

- `xtask/` — the screenshotter, the KaTeX JS build and fixture regeneration;
  it drives browsers over WebDriver and needs the KaTeX submodule.
- `crates/wasm-binding/` — the npm binding. Our own `wasm/` crate replaces it.
- The `KaTeX` submodule. Nothing here builds from it. The benches read
  `../../KaTeX/test/screenshotter/ss_data.yaml` (relative to this crate) at
  run time, so they build but need a KaTeX checkout there to run.
- CI (`.github/`), `docs/`, `.config/` (nextest, insta), `.cargo/`,
  `clippy.toml`, `rustfmt.toml`, the repo README.

`tests/` (the spec-port tests, their insta snapshots and the pinned
`fixtures/upstream.json`), `benches/`, `data/` and `build.rs` came over
unchanged; the workspace settings the crate inherits (package metadata, the
clippy lint table, the release profile) are in `../Cargo.toml`.

## Local changes

Each one makes the fork's output equal KaTeX 0.18.5's; the display oracle
(`../oracle/`) found them and `../oracle/fixtures.json` keeps a synthetic case
for each (its `class`).

- `Cargo.toml`: `publish = false`; the `readme` pointing outside the crate
  is gone.
- **Attribute order** (`attr-order`): node attributes lived in a hash map with
  a random per-map seed, so their order changed between runs. `AttrMap`
  (`src/namespace.rs`) keeps insertion order like a JS object; every
  attribute map uses it. `functions/rule.rs` sets `\rule`'s MathML attributes
  in KaTeX's order.
- **MathML classes** (`mathml-class`): written after the attributes as
  `class ="…"` (KaTeX JS's spelling); `\vcenter` sets a class, not a `class`
  attribute.
- **Spacing glue placement** (`glue-after-space`): `build_html.rs` inserts
  inter-atom glue after the last node visited, explicit spaces included, as
  KaTeX's `prev.insertAfter` does, not after the previous atom.
- **Small delimiters** (`small-delim`): the glyph takes no classes, and the
  centring shift is for the delimiter's style, not textstyle
  (`delimiter.rs`).
- **Spans built without options** (`lap-mtight`, `sqrt-root-mtight`,
  `supsub-wrapper`): `\mathllap`'s inner span, `\sqrt`'s index wrapper, the
  empty supsub base and `assembleSupSub`'s base wrapper take no options in
  KaTeX, so no `mtight` or colour.
- **`\stackrel`** (`stackrel-shift`): `suppress_base_shift: Some(false)` no
  longer suppresses the shift (`functions/op.rs`).
- **`\sqrt` of a bare symbol** (`sqrt-padding`): the symbol gets the
  `padding-left` too.
- **Style order** (`style-order`): `\kern` and `\rule` set their sizes after
  the colour from the options, as KaTeX does.
- **Empty `\mathop{}`** (`empty-mathop`): MathML `<mo></mo>`, not the
  named-operator form.
- **Negative zero** (`negative-zero`): tall delimiter paths printed `v-0`.
- **SVG sizes** (`phase-floor`, `sqrt-tall`): `\phase` floors its viewBox
  height; a tall `\sqrt` floors and adds the 80-unit pad.
- **Double-stroke paths** (`double-stroke`): KaTeX 0.18 joins the two copies
  with a space, not a newline.
- **`\xrightarrow`** (`xarrow-svg-align`): the arrow's wrapper gets
  `svg-align`.
- **Accents** (`accent-cramped`): the base is built in cramped style
  (`functions/accent.rs` had the options and base options swapped).
- **`\kern` max font size** (`glue-max-font`): none, as KaTeX's `makeGlue`.
- **Error rendering** (`error-span`): the `katex-error` span's title is
  `ParseError: …` and its style `color:#cc0000`, as in KaTeX.
- **Error messages** (`error-text`, `error-position`): quoting and case as in
  KaTeX (`'\sqrt' in text mode`, `Unexpected character: '\'`, `only one
  infix…`, the delimiter's text in `Invalid delimiter`); positions and context
  windows counted in UTF-16 units; errors point at their token where KaTeX's
  do (infix operators, `\middle`, invalid units and delimiters, `Expected & or
  \\ or \cr or \end`, and an argument's end-of-input token). `\middle` checks
  its delimiter before the `\left` depth, as KaTeX does.
- Tests: `tests/errors_spec.rs` expects the delimiter's text, as KaTeX's
  `errors-spec.ts` does; two `katex-error` snapshots carry the new title and
  style.

### Source mapping

Not in KaTeX: `Settings::source_map`, for the app's edit field. Off, the
output is unchanged (the display oracle runs with it off); on, the HTML maps
back to the source. `../oracle/render.ts --source-map` and
`tests/source_map.rs` check it.

- `src/source_map.rs` (new): `SourceInput` (the formula's input; a location
  in any other text, such as an unretargeted macro body, is not mapped),
  UTF-16 conversion, `tag`, the placeholder, `continues_cluster`.
- `Options::source_map` holds the `SourceInput` when the setting is on
  (`build_tree.rs` finds the parser's own input `Arc` in the tree).
- `build_html::build_group` tags each built node with its range (after the
  sizing wrapper): `data-s`/`data-e` attributes on spans, anchors and SVGs,
  `SymbolNode::source` on glyphs (written as a span around the glyph), and
  each unmapped child of a fragment. An element a builder passed up from its
  child takes the outer node's range.
- `build_common::push_combine_chars` merges glyphs only within one node's
  range, or a combining mark onto its base. A pair kept apart only for its
  ranges lays out as the merged run would: the earlier glyph drops its italic
  correction, and `SymbolNode::joins_prev` keeps inter-atom glue
  (`build_expression`) and line breaks (`build_html`) out from between them.
- `SymbolNode` has a hand-written `Debug` that omits the new fields when
  unset, so the spec tests' debug snapshots stand.
- `functions/ordgroup.rs` and `functions/text.rs`: an explicit empty group
  (`{}`, an empty cell) or `\text{}` draws the placeholder: AMS `\square`
  (real metrics) with class `oc-placeholder` and a zero-width range just
  inside its braces.
- Parse-tree locations, every change gated on the setting: each node's
  `loc` lies in the formula's own input and holds its children's.
  `parser/node_locs.rs` (new) adds `AnyParseNode::loc_mut`,
  `children`/`children_mut` and the widening helpers; `Parser::parse` ends
  with `cover_children` over the tree; `SourceLocation::cover` joins two
  ranges in either order.
  - A function node spans its invocation, the control word through its last
    argument (`FunctionContext::invocation`, what `context.loc()` returns);
    `\left…\right` runs through `\right`'s delimiter, `\begin…\end{…}`
    through `\end{…}` (a matrix's delimiter wrapper and its array both),
    text-mode `$…$` through the closing `$`, an infix `\over` from
    numerator to denominator.
  - Macro body tokens (text bodies and `\def`/`\newcommand` definitions)
    take the invocation's span (`MacroExpander::consumed`); pasted arguments
    and tokens a built-in passes through keep theirs, except those from the
    macro name's own body.
  - Nodes the parser built without one get a location: primes, Unicode
    scripts, SupSub (from its base, or its first script token), infix sides,
    colour/size/URL/raw arguments, `\hbox`'s wrapper, unsupported commands
    (`format_unsupported_cmd` takes the command's location;
    `functions/includegraphics.rs` uses it instead of its own copy), array
    and CD cells, CD arrows and labels, and `\tag`'s wrapper (the whole
    formula). An empty side, cell or label is the empty range where its
    content would go. `aligned`'s inserted `{}` has none.
  - A control word keeps KaTeX's span, its trailing spaces included.
  - `tests/source_locs.rs` checks nesting, the input, missing locations and
    that only locations change.
