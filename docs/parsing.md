# Parsing — PDFs into per-page markdown

Every PDF in the library is read by **MinerU's cloud service**, called
in-process from Rust. There is no local parser, no fast tier and no fallback:
a PDF either has markdown or it does not, and when it does not, the UI says so
(see *The failure story* below).

This page replaces the old `sidecar.md`. The Python process it described is
gone from the app — see [architecture.md](./architecture.md) for what the two
remaining processes are.

## Where

| Piece | Location |
| --- | --- |
| The seam: trait, artifact contract, error vocabulary, version | `app/src-tauri/src/parse/mod.rs` |
| MinerU cloud protocol + the `Parser` impl | `app/src-tauri/src/parse/mineru/client.rs` |
| Content list → page records (what the markdown *says*) | `app/src-tauri/src/parse/mineru/render.rs` |
| Submission queue (batching window, in-flight cap) | `app/src-tauri/src/parse/mineru/batch.rs` |
| Daily allowance + the two rate limiters | `app/src-tauri/src/parse/mineru/ledger.rs` |
| The `parse-status` event | `app/src-tauri/src/parse/events.rs` |
| Call site, and the thread each parse parks on | `app/src-tauri/src/sync.rs` |
| Artifact paths and purging | `app/src-tauri/src/paths.rs` |
| MinerU keychain commands + the pre-store token probe | `app/src-tauri/src/mineru.rs` |
| Failure vocabulary, rendered | `app/src/lib/parseState.ts` |
| Live job state, per-file failures, the app-wide latch | `app/src/stores/parseStore.ts` |
| Background recovery sweep | `app/src/hooks/useQualitySweep.ts` |
| Settings UI (token, privacy statement) | `app/src/pages/settings/LibraryPage.tsx` |

## The seam

`parse::Parser` is a trait, not a URL. MinerU cloud implements it in-process;
a local parse server — [its own repo](./architecture.md), reached over loopback
on the port the sidecar vacated — would implement the same one. Nothing in
`parse/mod.rs` may assume the cloud: an API root, a token, an upload ceiling
all arrive as configuration or as parameters.

What the seam owns is the **contract**, not the parsing:

- the on-disk artifact layout,
- `PARSER_VERSION`, which decides whether a file is already done,
- `ParseError`, the vocabulary the failure UI branches on,
- and the order the artifacts hit the disk.

`Health { backend, parser_version, ready }` is the version handshake. A backend
whose `parser_version` differs is **refused, naming both versions** — not
warned about and used anyway. The cloud client cannot disagree with itself, so
this exists for the local server that has not shipped yet.

`parse_config()` reads the `settings` row `parse`, key `engine` (`cloud` |
`local`) with an optional `engineUrl` override. Absence means `Cloud`, which is
every install today.

**The old `backend` key is ignored entirely**, `"auto"` included. It named a
*fallback policy* over the Python sidecar — "local" meant that Python parser
specifically — so those values cannot be reinterpreted; promoting a stale
`"local"` would aim the app at a parse server nobody installed. The stale
`memoryCapMb` and `backend` keys are left in the blob rather than migrated out.

## The on-disk contract

Beside each PDF:

- `<stem>.md` — full-document markdown
- `<stem>.pages.json` — `{pdf, mode, parser_version, page_count, pages:
  [{page_no, markdown}]}`, keyed by 1-based `page_no`. **This is the join key
  retrieval rests on**, and the only evidence a parse finished.
- `<stem>_images/` — extracted images; the directory's *name* is the link
  prefix written into the markdown, which is why both are computed in
  `parse/mod.rs` rather than by each backend.

`PARSER_VERSION` is **2** and stays there. It is the only thing standing
between an already-parsed library and a full re-parse, and it moves when the
artifacts change shape — not when a backend changes and not on a release.
`mode` is always `"quality"`; the field survives because records on disk have
it, not because there is another tier.

**`.pages.json` is written last, via temp+rename.** The Python wrote `.md`
first and both non-atomically, which is what produced orphan states — an
interrupted parse leaving markdown that read as evidence of a finished one.
`parse_mode` reads the record and nothing else: `mode == "quality"` at this
version means done, and **anything else — missing, unreadable, any other mode
— means parse it**. The `.md` existence check is gone; `.md` is derived from
the record, not evidence about it.

A result that comes back with no content list writes **nothing**, as an
outright error. That *prevents* the bad state; `parse_mode` *recovers* from one
that exists anyway (a restored backup, a hand-copied file). Both, not either.

Changed bytes purge the artifacts (`paths::purge_parse_artifacts`) before the
re-parse is triggered — the skip checks read records, so a stale record would
keep serving the old markdown forever.

## How a parse actually runs

The API's shape dictates it. A **batch** of documents is submitted as a list of
names, MinerU returns one signed upload URL per name, each file is `PUT` to its
URL, and one endpoint is polled until every task in the batch reports `done`
with a result ZIP. There is no per-file endpoint and no callback, so a batch is
one long blocking conversation.

- **Batching is not an optimisation, it is the API.** One submit carries up to
  50 tasks against a budget of 50 requests/minute; fifty files sent singly cost
  fifty submits and fifty poll loops. The window is **5 seconds or 20 files,
  whichever comes first**, with at most **8 batches in flight**. The window
  opens when the dispatcher wakes and finds work, so a lone file waits five
  seconds and goes.
- **Pages come from `content_list.json`, never the flat `.md`** in the ZIP —
  that file has no page boundaries and drops `header` items, which on slides
  are the titles. `chart_footnote` items carry the figure-explaining prose.
- **Progress is counted, never inferred.** Per-task page counts are summed;
  nothing is derived from page offsets or from how many tasks finished.
- **Errors carry a code, never the server's text.** MinerU's error bodies can
  quote the signed URLs it issued; those must not reach the UI, a log, or a
  pasted bug report.
- **Failures are scoped.** A malformed result, a `failed` task or a refused
  upload condemns *that document*. Only credentials, quota and a dead poll
  channel condemn the batch.
- Images are staged into a scratch directory whose name deliberately differs
  from the link prefix, so a parse that dies halfway cannot have overwritten
  prior artifacts.
- The renderer keeps the Python's 64-page boilerplate-grouping window: a
  header or footer repeated across enough of a window is template furniture,
  measured per document rather than hardcoded.

**Concurrency belongs to the batcher, not to a worker pool.** `sync.rs` spawns
one detached thread per PDF and the old bounded pool is gone — an inversion,
not a regression. The pool existed because every parse was an HTTP POST into
the sidecar and 105 decks meant 105 simultaneous POSTs at ~2 GB each; that was
the OOM. A second gate now would only stop files reaching the window they are
meant to share. What each thread does with its time is park on the batcher's
condvar: no socket, no request in flight.

**`parse_pdf` blocks for the whole round trip — minutes, not seconds.** The
sidecar returned as soon as a fast pass had produced *some* markdown. Every
caller now has to be somewhere that can wait that long.

## Progress

`parse-status` is emitted directly from Rust (`parse/events.rs`). It carries
`queued | running | quality | error`, and on `error` also `kind`, `retryable`
and `latching`.

This replaced `ipc.rs`, a loopback HTTP server that existed solely because the
sidecar was another process and its ephemeral port had to be threaded through
every call site that might cause a parse. The `AppHandle` is now set once at
startup rather than passed down, because the alternative puts a Tauri type in
the middle of code the CLI runs — and **a headless run leaves it unbound and
every emit is a no-op**, which is the honest shape of it.

`"quality"` is the terminal success. The name outlived the tier: there is one
parse now, but it is what every already-parsed row says and what the "already
done" check reads, so the string is frozen.

## MinerU's limits, and the ledger

From the [live API docs](https://mineru.net/apiManage/docs) (checked
2026-09-03): 200 MB and 200 pages per file, 50 signed-upload entries per
request, 1000 highest-priority pages/day (after which service is slower, not
refused). The public page does not confirm account submission or file-day
quotas, so Oculus keeps **50 submits/min, 1000 polls/min and 5000 files/day**
as its own conservative application budgets rather than claiming larger ones.

Oversized files are **refused before upload**, with the number in the message.
The Python physically sliced them; that was deliberately not ported — a
>200 MB coursework PDF is hypothetical and slicing was the fiddliest part.

`mineru-usage.json`, beside the database, is the local guess at what is left,
because MinerU exposes no endpoint that says. Two rules give it its shape:

- **Reservations are taken before the network call and never given back.** A
  failed POST, a dead upload, a `SIGKILL` halfway — all keep the files they
  reserved. The server may well have counted the work and we cannot ask, so an
  uncertain failure counts against us. A rollback would drift the count
  optimistic, which is the direction that becomes a wall of server-side
  rejections.
- **Server errors beat the local guess.** MinerU's own `-60018` latches the
  ledger, and nothing goes near the network until the day rolls over. The day
  boundary is assumed to be Beijing midnight; the provider's actual reset
  timezone is still unconfirmed.

## The token

Rust alone touches the keychain entry. It never enters SQLite, the WebView, a
progress payload or a log. There is no loopback body to inject it into any
more — the client reads the keychain at construction.

A token is checked **before** it is stored: `mineru_set_api_key` GETs a
non-existent task id, which costs nothing and creates nothing, and treats
401/403 as MinerU refusing it (`A0202` invalid, `A0211` expired). Anything else
— including the expected "task not found" — means it passed the gateway. An
unreachable MinerU stores the token and reports `unverified` rather than
blocking someone offline.

**There is no rejection latch to clear any more.** The sidecar held one because
a refused token is the same refusal for every queued file and it had no way to
be told the user had fixed it; the in-process client keeps no such state, so
the very next parse uses whatever is stored now. What remains is the app-wide
`ParseLatch` in `parseStore`, which is session-scoped — saving a token lifts it
explicitly from the settings page.

Settings states the privacy boundary rather than offering it as a choice: every
PDF goes to MinerU and its PRC-hosted OSS storage, and MinerU's documented
15-minute cache tolerance is not a deletion guarantee.

## The failure story

There is no tier beneath the cloud, so a failure means that file has no
markdown — and with it no search, no `@`-mention and no Markdown view — until
something changes. `ParseError` exists to keep three answers distinguishable:

| Variant | `kind()` | Retryable | Latching |
| --- | --- | --- | --- |
| `MissingCredentials` | `missing_credentials` | no | yes |
| `RejectedCredentials` | `rejected_credentials` | no | yes |
| `QuotaExhausted` | `quota_exhausted` | yes, later | yes |
| `Offline` | `offline` | yes | no |
| `TooLarge` | `too_large` | no | no |
| `Document` | `document` | no | no |
| `VersionMismatch` | `version_mismatch` | no | yes |
| `NotReady` | `not_ready` | yes | yes |
| `Io` | `io` | yes | no |

`kind()` is a **frozen vocabulary** — `Display`'s prose is for a student and
may be reworded at any time, but the UI branches on `kind`. Both credential
variants keep that word in their name because `parseState.ts` matches
`/credential|token/i` against it to decide whether a failure is worth pointing
at Settings. A spent quota heals on a clock and a version mismatch is an
update, so neither gets that button.

`latching` is the distinction the UI must never fudge: "this file is broken"
and "parsing is down for everything" call for different words and different
fixes. Both discriminants are **optional** — a failure inherited from a
previous session is only the word `error` in the DB — so unknown is its own
case and is never coerced into either extreme.

The background sweep reads the same discriminants. It used to be free to be
wrong, because a failure fell back to a local parser; every parse is a metered
cloud call now, so `retryable === false` is never re-kicked and the sweep
stands down entirely under a latch, rather than marching the library through
the same error one batch at a time.

## History worth keeping

- **The fast tier is gone.** `pymupdf4llm` returned in ~2s and had to live in
  a throwaway subprocess because it leaked ~2 GB per deck. Nothing in the
  library was ever left in `fast`: checked 2026-09-16, `files.parse_status` was
  166 `quality` and 540 `NULL`, and all 161 readable records on disk said
  `"quality"`. Files passed through fast; they never rested there.
- **Formula decoding went pix2tex → docling enrichment → MinerU** (2026-08-15).
  MinerU is ~100× faster than docling-with-enrichment and more correct (1% vs
  18% KaTeX render failures on the benchmark deck).
- **Local quality parsing is gone with the Python**, and with it the whole-tree
  memory governor, the 8 GB budget and the formula-batch cap. The shape of the
  problem they solved does not exist in this process. Those pins and their
  measurements are at `f875bb1`, the last commit holding `sidecar/`.
- **Office-derived PDFs (`*.pptx.pdf`) have never been parsed in this
  library**, so that path has no fixture and is unproven in practice.
  LibreOffice conversion is Rust already (`app/src-tauri/src/sync.rs`) and was
  not touched by any of this.

## Debugging

Golden fixtures with their source PDFs live in `data/parse-fixtures/`
(gitignored) — five shapes, with `MANIFEST.sha256` over every `.md` and
`.pages.json`:

```bash
cd data/parse-fixtures && shasum -a 256 -c MANIFEST.sha256
```

They are a local harness deliberately: neither the fixtures nor tests over them
belong in the repo. The differential tests that pinned `render.rs` against the
Python **are** in the repo (`app/src-tauri/src/parse/mineru/render.rs`), and
are now the working record of what that code did — the code itself is at
`f875bb1`, which every `sidecar/*.py` citation in Rust refers to.
