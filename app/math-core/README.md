# math-core

The app's maths engine, in Rust: a Cargo workspace of its own, never a
dependency of `app/src-tauri` (so `tauri dev` never rebuilds it).

| Member | What it is |
| --- | --- |
| `katex/` | A vendored fork of katex-rs, a Rust port of KaTeX 0.18.5. Where it came from and what we changed: [`katex/UPSTREAM.md`](katex/UPSTREAM.md). |
| `oracle/` | The display oracle: `src/main.rs` (the `oracle` bin) renders JSON-lines requests with the fork; `render.ts` renders the same formulas with the app's `katex` and diffs the two. |

Where the fork still differs from KaTeX JS, and why that is accepted:
[`DIVERGENCES.md`](DIVERGENCES.md).

## Tests

```sh
cd app/math-core
cargo test
```

Format the oracle crate with `cargo fmt -p oracle`; never run `cargo fmt` over
`katex/` (see `katex/UPSTREAM.md`).

## The oracle

```sh
cd app
bun math-core/oracle/render.ts [--katex <KaTeX checkout>] [--out <report.json>]
```

It builds `oracle` (`cargo build --release --bin oracle`), collects every
`$$…$$` and `$…$` from the `.md` files under the app's data directory, KaTeX's
pinned fixtures (`katex/tests/fixtures/upstream.json`) and our synthetic cases
(`oracle/fixtures.json`), renders each with both engines under the app's five
call-site option sets, and prints the counts per set and per difference class.
`--katex` adds KaTeX's spec inputs from a checkout at the tracked commit. The
run fails on any difference `DIVERGENCES.md` doesn't accept.

The report (`--out`, default `$TMPDIR/katex-oracle-report.json`) holds the
user's formulas: keep it outside the repo. A difference found there becomes a
synthetic case in `oracle/fixtures.json`, never a copied formula.

The bin's protocol, one JSON object per line:

```text
in:  {"id": 1, "tex": "x^2", "display": false, "options": {"throwOnError": true, "strict": "ignore", "macros": {"\\foo": "x"}, "output": "htmlAndMathml"}}
out: {"id": 1, "html": "<span class=\"katex\">…"}   or   {"id": 1, "error": "KaTeX parse error: …"}   or   {"id": 1, "panic": "…"}
```

Options take KaTeX JS's names and defaults (`strict` defaults to `"warn"`).
