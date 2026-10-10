# Divergences from KaTeX 0.18.5

Where the fork's output differs from KaTeX JS's (the app's `katex`, 0.18.5),
as measured by `oracle/render.ts` under the app's five call-site option sets
(`oracle/sets.ts`), both as the native `oracle` bin and as the app's wasm
build (`--engine wasm`).
Each entry has a synthetic case in `oracle/fixtures.json` and a decision. The
oracle treats the accepted ones as passing (`ACCEPTED` in `render.ts`); any
other difference fails the run.

## Status (2026-10-10)

The user's notes and parsed files (3,942 unique formulas) render byte for
byte as KaTeX JS does under all five option sets, with either engine, and
error/no-error and the error text agree everywhere. KaTeX's own spec inputs
(746) and the fixtures match except D1. Paths are under
`app/src/components/`.

| Set | Call site | Options | Equal | Accepted | Differs |
| --- | --- | --- | ---: | ---: | ---: |
| a | `documents/editor/live-preview/widgets/math.ts` | `{displayMode, throwOnError: true, macros: {"\\arraystretch": "1.2"}}` after `hugArrays` | 4,728 | 4 | 0 |
| b | `documents/editor/math/field/mathField/visual-state.ts` | `{displayMode, throwOnError: true, strict: "ignore"}` | 4,728 | 4 | 0 |
| c | `documents/editor/math/tools/mathTools/dom.ts` | `{throwOnError: false}` | 4,728 | 4 | 0 |
| d | `documents/editor/math/tools/mathTools/popover.ts` | `{displayMode, throwOnError: true}` | 4,728 | 4 | 0 |
| e | rehype-katex (`markdown/MdComponents.tsx`, `files/FileMarkdown.tsx`) | `{displayMode, throwOnError: true}`, then `{displayMode, strict: "ignore", throwOnError: false}` | 4,728 | 4 | 0 |

Before the fixes in `katex/UPSTREAM.md` ("Local changes"), set a had about 1,490
HTML differences and 12 error-text differences on the notes alone.

## D1 — `undefined` in MathML for line-segment accents

```tex
\overlinesegment{AB}
\underlinesegment{AB}
```

KaTeX JS writes `<mo stretchy="true">undefined</mo>`: its `stretchyCodePoint`
table has no entry for these labels, so the text node gets `undefined`. The
fork writes a space. Only the MathML differs (hidden; screen readers would
read KaTeX's "undefined"); copy and `mathSelection/` read the
`annotation`, not the operator. Neither command appears in the notes.

**Decision: accept.** The fork's output is the better one; the oracle
rewrites KaTeX's `undefined` to the fork's space before comparing.

## Not covered

- `output: "html"` and `"mathml"` alone: the app uses neither; the oracle
  supports them (`options.output`).
- Option combinations no call site uses (`trust`, `maxSize`, `maxExpand`,
  `leqno`, `fleqn`, `minRuleThickness`, `colorIsTextColor`, `errorColor`).
- `strict: "warn"` logs: KaTeX JS writes `console.warn`, the fork `eprintln!`
  (`Settings::report_nonstrict`). Output is the same; the wasm build drops
  the warning (`eprintln!` writes nowhere on wasm32-unknown-unknown).
- Astral-plane characters inside an error's underlined range: KaTeX JS
  underlines each UTF-16 unit (splitting surrogate pairs), the fork each
  character.
