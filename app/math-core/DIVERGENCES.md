# Divergences from KaTeX 0.18.5

Where the fork's output differs from KaTeX JS's (the app's `katex`, 0.18.5),
as measured by `oracle/render.ts` under the app's five call-site option sets.
Each entry has a synthetic case in `oracle/fixtures.json` and a decision. The
oracle treats the accepted ones as passing (`ACCEPTED` in `render.ts`); any
other difference fails the run.

## Status (2026-10-10)

The user's notes and parsed files (3,937 unique formulas) render byte for
byte as KaTeX JS does under all five option sets, and error/no-error agrees
everywhere. KaTeX's own spec inputs (746) and the fixtures match except D1.

| Set | Call site | Options | Equal | Accepted | Differs |
| --- | --- | --- | ---: | ---: | ---: |
| a | `widgets.ts:86` | `{displayMode, throwOnError: true, macros: {"\\arraystretch": "1.2"}}` after `hugArrays` | 4,723 | 4 | 0 |
| b | `mathField.ts:323` | `{displayMode, throwOnError: true, strict: "ignore"}` | 4,723 | 4 | 0 |
| c | `mathTools.ts:77` | `{throwOnError: false}` | 4,723 | 4 | 0 |
| d | `mathTools.ts:589` | `{displayMode, throwOnError: true}` | 4,723 | 4 | 0 |
| e | rehype-katex | `{displayMode, throwOnError: true}`, then `{displayMode, strict: "ignore", throwOnError: false}` | 4,723 | 4 | 0 |

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
read KaTeX's "undefined"); copy and `mathSelect.ts` read the
`annotation`, not the operator. Neither command appears in the notes.

**Decision: accept.** The fork's output is the better one; the oracle
rewrites KaTeX's `undefined` to the fork's space before comparing.

## Not covered

- `output: "html"` and `"mathml"` alone: the app uses neither; the oracle
  supports them (`options.output`).
- Option combinations no call site uses (`trust`, `maxSize`, `maxExpand`,
  `leqno`, `fleqn`, `minRuleThickness`, `colorIsTextColor`, `errorColor`).
- `strict: "warn"` logs: KaTeX JS writes `console.warn`, the fork `eprintln!`
  (`Settings::report_nonstrict`). Output is the same; the wasm build should
  route or drop the warnings.
- Astral-plane characters inside an error's underlined range: KaTeX JS
  underlines each UTF-16 unit (splitting surrogate pairs), the fork each
  character.
