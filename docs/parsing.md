# Parsing — PDFs into per-page markdown

Every PDF (and every Office document, as its LibreOffice-converted PDF) is
read by **MinerU** — its cloud service, or a MinerU server the
user runs on their own Mac over loopback — chosen in Settings → Parsing. Both
are HTTP calls made in-process from Rust; Oculus never starts or supervises
either. MinerU is ~100× faster than docling with formula enrichment, with 1% vs
18% KaTeX render failures on the benchmark deck.

## Where

| Piece | Location |
| --- | --- |
| The seam: trait, artifact contract, `ParseError`, `PARSER_VERSION` | `app/src-tauri/src/parse/mod.rs`, `app/src-tauri/src/parse/parser.rs`, `app/src-tauri/src/parse/record.rs`, `app/src-tauri/src/parse/error.rs` |
| MinerU cloud protocol + its `Parser` | `app/src-tauri/src/parse/mineru/client/` |
| Local MinerU server: its `Parser` and the probe | `app/src-tauri/src/parse/mineru/local/` |
| Result content-list loading; page rendering (shared by both engines) | `app/src-tauri/src/parse/mineru/mod.rs`, `app/src-tauri/src/parse/mineru/render/` |
| Cloud submission queue (window, in-flight cap) | `app/src-tauri/src/parse/mineru/batch.rs` |
| Cloud daily allowance + the two rate limiters | `app/src-tauri/src/parse/mineru/ledger.rs` |
| Result-download TLS: the expired-certificate exception and its state | `app/src-tauri/src/parse/mineru/result_tls.rs`, `app/src/lib/pipeline/resultCert.ts`, `app/src/components/sync/ResultCertWarning.tsx` |
| Token bucket, semaphore, retry ladder (shared with Voyage) | `app/src-tauri/src/providers/ratelimit/` |
| The `parse-status` event and shared wire payload | `app/src-tauri/src/parse/events.rs`, `app/src-tauri/src/runtime/pipeline_events.rs` |
| Call site; one thread per PDF; LibreOffice conversion | `app/src-tauri/src/sync/parse.rs`, `app/src-tauri/src/sync/phases/output.rs`, `app/src-tauri/src/sync/office.rs` |
| Spreadsheets to text: conversion, page rows, startup reconcile | `app/src-tauri/src/pages/sheets/` |
| Skipping a file: the `parse_skip` command and its marks | `app/src-tauri/src/sync/scrape/mod.rs`, `app/src-tauri/src/parse/skips.rs` (`Skips`) |
| Artifact purging | `app/src-tauri/src/library/paths/course.rs` |
| MinerU keychain commands + the pre-store token probe | `app/src-tauri/src/providers/mineru.rs` |
| Engine selection, endpoint override, local probe | `app/src-tauri/src/parse/commands.rs` |
| Failure vocabulary, rendered | `app/src/lib/pipeline/parseState.ts` |
| Live job state, per-file failures, the app-wide latch | `app/src/stores/sync/parseStore.ts` |
| Background recovery sweep | `app/src/hooks/sync/useQualitySweep.ts` |
| Settings UI (engine, token, expired-certificate switch, endpoint, server status) | `app/src/components/settings/library/ParserSection.tsx` |

## MinerU's result CDN and its expired certificate

Results download from `cdn-mineru.openxlab.org.cn`, whose certificate has
expired. With Settings → Parsing's "Accept an expired download certificate"
on (the default), `result_tls` re-verifies that host's expired certificate as
of its last valid second: chain, signatures and host name are still checked,
only the clock is excused. Any other host or certificate error is refused, and
a renewed certificate passes the ordinary check, so the exception lapses on its
own. Each handshake records what it found; while the exception is in use the
Sync pipeline shows a warning icon (`ResultCertWarning`) and Settings says so.
Turned off, downloads fail as `Offline` with "certificate expired".

## Nothing falls back, so a failed parse must surface

The engine setting chooses *which* MinerU runs; it does not stack them, and a
parse that fails is never re-tried on the other engine. A failure means that
file has no markdown — no search, no `@`-mention, no Markdown view — and the UI
must say so. `ParseError` keeps the answers distinguishable:

| Variant | `kind()` | Retryable | Latching |
| --- | --- | --- | --- |
| `MissingCredentials` | `missing_credentials` | no | yes |
| `RejectedCredentials` | `rejected_credentials` | no | yes |
| `QuotaExhausted` | `quota_exhausted` | yes, later | yes |
| `Offline` | `offline` | yes | no |
| `TooLarge` | `too_large` | no | no |
| `Document` | `document` | no | no |
| `VersionMismatch` | `version_mismatch` | no | yes |
| `NotReady` | `not_ready` | yes | no |
| `Io` | `io` | yes | no |
| `Cancelled` | `cancelled` | no | no |

- **`kind()` is a frozen vocabulary**; `Display` is prose and may change.
  `parseState.ts` matches `/credential|token/i` on `kind` to decide whether to
  point at Settings, which is why both credential variants carry that word.
- **`latching` separates "this file is broken" from "parsing is down for
  everything"**. Both discriminants are optional — a failure from a previous
  session is only `error` in the DB — and unknown is never coerced into either.
- **The sweep reads the same discriminants**: `retryable === false` is never
  re-kicked, and a latch stands it down (one probe file after
  `LATCH_PROBE_AFTER_MS`).
- **An Office file LibreOffice could not convert is `Document`** with code
  `parse::CONVERSION_FAILED`, whose sentence names the conversion, not MinerU;
  an unreadable spreadsheet is `Document` with `parse::SHEET_UNREADABLE`.
- **`NotReady` is retryable and non-latching on purpose**: a local server that
  is stopped or still loading must not mark any file permanently broken.
- **`Offline` names its cause**: `providers::ratelimit::transport_detail` keeps ureq's
  source chain ("certificate expired", "connection refused") and drops the URL,
  and the sentence shows it. Every failure is also printed to stderr as
  `[oculus] parse failed: <path>: <sentence>`, since the DB keeps only `error`.
- **`Cancelled` is a skip, not a failure**: it reaches the UI as the terminal
  status `skipped` with no error fields, and stderr says
  `[oculus] parse skipped: <path>`. See below.

## A parse takes minutes, and only the engine bounds it

`parse_pdf` blocks for the whole round trip; every caller must be able to wait.
Nothing above the client imposes a deadline — it could only abandon work that
was still progressing.

- **Cloud**: `POLL_DEADLINE` (60 min) in `mineru/client/mod.rs` is the only one.
- **Local**: a connect timeout and no read timeout. `/file_parse` is minutes of
  silence on this machine's CPU, not a hang; connecting is instant or the
  server is not running.
- **Progress is counted, never inferred**: cloud sums per-task page counts;
  local reports a page count and then a finish, with no estimated bar between.
- **A running parse names its phase** (`parse::Phase`, the `phase` field of a
  `running` event): `upload_wait` (its batch is submitted, another file is
  uploading first; carries `bytes_total`), `uploading` (`bytes_done` and
  `bytes_total`, at most two events a second plus the final 100%), then
  `processing`. A document over several tasks uploads its file once per task,
  so its byte total is the sum. Only the cloud uploads; the local engine
  reports `processing` alone, and an absent `phase` means `processing`.

**Concurrency belongs to each engine.** `sync/phases/output.rs` spawns one detached thread
per PDF. Cloud threads park on the batcher's condvar until their window
closes. The local client holds one permit (`PARSE_GATE`) because `mineru-api`
reports `max_concurrent_requests: 1` — a second request would only queue
inside that server on a socket of ours. It is taken before the first progress
report, so a file waiting for it still reads as queued. A gate there
would keep cloud files out of the window they are meant to share.

**One parse per PDF.** A sync, the sweep and "Parse now" can ask for the same
file at once. `parse::InFlight` holds each PDF path while its parse runs; a
second caller waits (no timeout) and then takes the already-parsed path, which
emits `quality` without a second billed call.

**Every parse ends in `quality`, `error` or `skipped`.** `parse_pdf_reporting` catches a
panic on the parse thread (rendering, writing) as `Io` "the parser crashed on
this file", and a panic inside hayro while counting pages reads the same. The
cloud client counts pages on the caller's thread before submitting, so a PDF
that crashes hayro fails alone rather than through its batch.

**Pages are counted by the renderer** (`client::page_count`, used by both
engines, over `pdf_render::page_count`), the same count the embedder
rasterises against, since `page_no` is the join key. It opens a PDF encrypted
with an empty user password; a real password fails as `Document`
`encrypted-pdf`. `parse_file` reports its own refusals (not a parseable file,
not on disk) as `Io` too, so a sweep kick never vanishes.

## A skip ends a parse without failing it

`parse_skip(relativePath, subjectId, skip)` marks the PDF in `parse::Skips`,
keyed like `InFlight` (`sync::parse::parse_key`), and emits `skipped` at once, so the
row settles whether or not a parse is running. `skip: false` clears the mark
and emits nothing; the frontend then calls `parse_file`. The mark lives only in
memory; across restarts the skip is the row's `files.parse_status = 'skipped'`,
which the frontend writes, and which `oculus index` (`db::store::pdf_files`) and
the background sweep both pass over.

Every engine ends a marked file as `Cancelled`, without billed work where it
can and without artifacts always:

- **`run_parse`** returns `Cancelled` before calling the backend, and discards
  a result that lands after a skip. An already-parsed file still takes the
  already-parsed path.
- **Cloud**: a marked document leaves its batch before the ledger reservation
  and the submit, is checked again before each `PUT`, and fails its upload
  body's next read mid-`PUT`. A skip while its batch polls drops it, and its
  result is never fetched. The waiting caller re-checks every 300 ms, so the
  thread ends promptly; the document is then abandoned, so clearing the mark
  for a fresh parse cannot revive it.
- **Local**: a blocking `/file_parse` cannot be aborted cleanly, so the mark is
  checked before and after taking `PARSE_GATE` and after the request returns.

## `.pages.json` is the only evidence a parse finished

The seam owns the contract, not the parsing: nothing in the `parse/` seam may
assume the cloud. Beside each PDF:

- `<stem>.md` — full-document markdown, derived from the record.
- `<stem>.pages.json` — `{pdf, mode, parser_version, page_count, pages:
  [{page_no, markdown, blocks?}]}`, keyed by 1-based `page_no`: **the join key
  retrieval rests on** ([retrieval.md](./retrieval.md)). `blocks` is below.
- `<stem>_images/` — its *name* is the link prefix written into the markdown,
  so both are computed in `parse/record.rs`, never by a backend.

**The record is written last, via temp+rename** (a temp name unique per
write), so an interrupted parse cannot look finished. `parse_mode` reads only `mode`: `"quality"` means done,
anything else (missing, unreadable, another mode) means parse it. The string
is frozen, and there is one tier.

The `.md` is derived from the record: the already-parsed path rebuilds a
missing one (`ParseOutput::restore_markdown`) before it emits `quality`, with
no re-parse.

`oculus index` reconciles `files.parse_status` with the disk
(`db::store::reconcile_parse_status`): a record means `quality`; without one a
stale `queued`/`running`/`quality` is cleared, but `error` and `skipped` stay.

`PARSER_VERSION` moves only when the artifacts change shape. `parse_mode` does
**not** check it, so a bump never re-parses the library; it is enforced by the
`Health` handshake, which refuses a backend with another version, naming both.
Older records keep reading: every field added since is optional.

`oculus index --reparse` is the one way to bring old records up to date. A
file whose record is `quality` but whose `parser_version` is missing or below
`PARSER_VERSION` (`parse::is_outdated`, an untyped read like `parse_mode`) skips
the already-parsed short-circuit and runs the ordinary parse path; every other
file behaves as plain `index`. It spends MinerU allowance and **keeps the
embeddings**: nothing is purged, the old record stands until the new one lands,
`record_pages` rewrites `pages.markdown`, and `.emb.json` — keyed on page
images and page count — stays current, so ingest skips the file. The app's
parse path never re-parses on version.

### `blocks`: where each rendered item sits

Each page lists one block per **rendered** content-list item, in markdown
order (after the header sort): `{kind, bbox, start, end}`.

- `kind` is MinerU's raw `type` (`text`, `header`, `equation`, `image`,
  `table`, `chart`, …); a `text_level` heading stays `text`.
- `bbox` is `[x0, y0, x1, y1]` as **fractions of the page**, top-left origin,
  y down: MinerU's content list gives 0–1000 integers, divided by 1000 and
  clamped to [0, 1]. Multiply by the page size in points to place it.
- `start..end` are **UTF-16 code-unit** offsets into that page's `markdown`
  (the consumer is JS `String.slice`), covering exactly the item's text — the
  `"\n\n"` between items is outside every block. An image or table and its
  footnote are one item, so one block.
- Items the renderer drops (boilerplate, footers, size-filtered crops,
  empties) have no block; neither does a rendered item without four finite
  numbers spanning a non-empty box — never a zero box. Blank pages, pages
  `ParseOutput::new` gap-fills, spreadsheets and records written before
  version 3 have none, and the key is omitted when empty.
- Tracking blocks never changes the markdown; `render.rs`'s tests pin it.

A result with no content list writes **nothing**. Changed bytes purge the
artifacts (`library::paths::purge_parse_artifacts`) before re-parsing, or a stale
record would serve old markdown forever.

## Both engines produce the same artifact, so switching costs nothing

`mineru/render/` is shared: both engines hand it a MinerU content list. The local
client's form fields (`backend=pipeline`, `lang_list=ch`, `formula_enable`,
`return_content_list`) match the cloud client's hardcoded parameters — **pins,
not settings**; changing the first two is a re-parse of everything.

So the engine select has **no confirmation, and none should be added** for
symmetry with the embedding select, whose engines are different vector spaces
([retrieval.md](./retrieval.md#one-space-or-the-ranking-is-noise)).
`parse_config()` reads the `parse` settings row — `engine` (`cloud` | `local`)
and an optional `engineUrl` — and anything missing means `Cloud`.

- **Both engines are always offered**: someone must be able to select Local and
  *then* start their server. Reachability is a status line, not a gate.
- **Switching engine drops the `engineUrl` override**, which would otherwise
  aim the new engine at the old one's address. Empty means the default.
- **The token row renders only under Cloud** — a key field under a backend that
  cannot use it invites pasted secrets.
- **Settings states the privacy boundary for the selected engine**: Cloud sends
  every PDF to MinerU's PRC-hosted storage (its 15-minute cache is not a
  deletion guarantee); Local sends nothing off the machine.

## The local engine is one request per PDF to a server the user runs

MinerU's own server offers `POST /file_parse`: one multipart request in, one
result ZIP out — no batching, quota, token or keychain. The default origin is
`http://127.0.0.1:8000` (`LOCAL_BASE_URL`), where MinerU binds. Users install
and start it themselves:

```bash
uv tool install -U "mineru[core]>=3.4,<4"
MINERU_API_OUTPUT_ROOT="$HOME/.cache/mineru-api" \
  mineru-api --host 127.0.0.1 --port 8000
```

- **The `<4` pin is load-bearing.** MinerU 4 removes `/file_parse` for a
  `/v1/...` API, so every parse would 404, and replaces `pipeline` with
  quality tiers whose markdown would not match the cloud's.
- **`MINERU_API_OUTPUT_ROOT` keeps parses out of the user's folders.**
  `mineru-api` writes each parse under `./output` in its working directory and
  `/file_parse` takes no `output_dir`; the server sweeps them only while it
  keeps running.
- **Install directly, not in Docker**: a macOS container cannot reach MPS/MLX.

`mineru::local::probe` returns `reachable` (`/health` said `healthy`), `unreachable`
(nothing answered, or it is still loading models) or `version_mismatch`
(`/health` 404s but `/v1/health` answers: a MinerU 4); Settings prints Rust's
sentence, which alone tells the `unreachable` causes apart.
`parse_probe_local`'s optional URL tests the endpoint field before it is saved.

The ignored test `a_real_mineru_answers_the_way_this_client_expects` in
`mineru/local/tests.rs` is the only check that the form fields match a real server.

## The cloud engine batches because the API has no per-file endpoint

A batch is a list of names; MinerU returns one signed upload URL each, every
file is `PUT`, and one endpoint is polled until each task has a result ZIP.
The window is **5 s or 20 files**, with **8 batches in flight**.

- **Polling starts only after every `PUT`**, so a slow upload holds back its
  whole batch. A file over `SOLO_UPLOAD_BYTES` (8 MiB, `mineru/batch.rs`)
  therefore never shares one: it leaves at once as a batch of one, and the
  window gathers only smaller files.
- **Within a batch, the smallest file is `PUT` first.**
- **Uploads and result downloads end on a stall, never a deadline**
  (`TRANSFER_STALL`, 120 s without a byte, in `mineru/client/mod.rs` and
  `result_tls::agent`). ureq's overall `timeout` would fail a slow but moving
  `PUT` at its response read, after every byte had gone up.

- **Pages come from `content_list.json`, never the ZIP's flat `.md`**, which
  has no page boundaries and drops `header` items (slide titles).
- **Failures are scoped**: a malformed result, a `failed` task or a refused
  upload condemns that document; only credentials, quota and a dead poll
  channel condemn the batch.
- **Errors carry a code, never the server's text** — MinerU's error bodies can
  quote the signed URLs it issued.
- Images stage in a scratch directory named unlike the link prefix, so a parse
  that dies halfway cannot overwrite prior artifacts.

**The ledger (`mineru-usage.json`) guesses what is left**, because MinerU has no
endpoint that says. Files over 200 MB or 200 pages are refused before upload;
Oculus's own budgets are 50 submits/min, 1000 polls/min and 5000 files/day.
Reservations are taken before the network call and **never given back** — a
rollback drifts the count optimistic, toward server-side rejections. MinerU's
`-60018` latches the ledger until the day rolls over (assumed UTC+8).

**The token lives only in the keychain** — never SQLite, the WebView, a payload
or a log. `mineru_set_api_key` checks it first by GETting a non-existent task:
401/403 is a refusal (`A0202` invalid, `A0211` expired), anything else passed,
and an unreachable MinerU stores it as `unverified`. The client keeps no
rejection latch; `parseStore`'s session-scoped `ParseLatch` does, and saving a
token lifts it.

## Spreadsheets are converted to text, never parsed

A workbook (`.xlsx`, `.xlsm`, `.xls`, `.ods`) is read in-process by `calamine`
(`app/src-tauri/src/pages/sheets/`) and written beside the original as
`marks.xlsx.md`: `# marks.xlsx`, then a `## <sheet>` section per worksheet
holding its filled extent as a GFM table, first row as the header (`(empty)`
for a blank sheet). No PDF, no MinerU call, no embedding: a sheet's text is
all there is to it, and a whole sheet makes no usable page image.

A `.csv` takes the same route as a one-sheet workbook named after its file,
read by `csv_rows` rather than calamine: quoted fields, `""` and line breaks
inside quotes, CRLF or LF, a UTF-8 BOM dropped, non-UTF-8 read as Latin-1,
and `;`, tab or `|` as the delimiter when the first line uses it more than `,`.
It has no formulas or merges, so it is only the table.

- **The table holds values**, each formula cell's stored result. A formula
  the file stored no result for (a workbook written by a script and never
  recalculated) shows in place as `=FORMULA`.
- **A merged block repeats its value** in every cell it covers, so each row
  reads on its own (a rubric's section label, a grouped header). xlsx and xls
  record merges; calamine reads none from ods, whose blocks stay top-left only.
- **`Formulas:` follows the table** when the sheet has any: one bullet per
  pattern, in order of its first cell. Cells whose formulas match once each
  A1 reference is made relative to its own cell (`$` parts stay absolute) —
  a run copied down or across — share a line naming the blocks they cover and
  the first cell's formula: `- D9:E14, D20: =PROPER(G9)`. A sheet lists 200
  patterns, a line 20 blocks, then `… and N more`. calamine expands an xlsx
  shared formula into each cell; an xls one it does not decode, so only cells
  with a formula of their own are listed there.

- **Each worksheet is one `pages` row**, page 1 the first sheet, so
  `oculus read --pages`, `oculus grep` and the lexical index see it.
  `pages::sheets::record` replaces the file's rows outright (`db::store::replace_pages`),
  sets `parse_status = 'quality'` and clears the embed columns.
- **It ends in the events a parse ends in** (`quality`, or `error` as a
  `Document` failure), so its File Activity row settles; nothing is queued,
  started or skippable. A failure deletes its text and pages, never falls
  back to LibreOffice.
- **Three callers**: a sync converts at download, and again for unchanged
  bytes whose `.md` is missing (`pages::sheets::needs_conversion`); `parse_file`
  converts on request (an upload's kick, a Retry); app startup runs
  `pages::sheets::reconcile` over every sheet on record.
- **A sync's conversion can beat the frontend's write of the `files` row**,
  so `pages::sheets::record` inserts a bare row when there is none and the frontend's
  upsert fills in the rest.
- **PDF-route files beside a sheet are swept**: a `.xlsx.pdf`,
  `.xlsx.pages.json`, `.xlsx.emb.json` or `.xlsx_images/` makes
  `needs_conversion` true, and the conversion deletes them
  (`library::paths::purge_parse_artifacts`) before writing its text.

## Gotchas

- Never add a fallback between engines or let a failure look like success — a PDF silently loses its markdown.
- Never wrap a parse in a timeout — it abandons parses that were progressing.
- Don't add a confirmation to the parse engine select — switching invalidates nothing.
- Don't drop the `<4` from the install line — every local parse 404s.
- Don't write `.pages.json` before the other artifacts — a crash then reads as a finished parse.
- Don't remove `PARSE_GATE` — sockets park for minutes in a single-request server's queue.
- `*.pptx.pdf` (LibreOffice-converted) has never been parsed in this library and has no fixture.
- Golden fixtures live in gitignored `data/parse-fixtures/` (`shasum -a 256 -c MANIFEST.sha256`) and stay out of the repo; `mineru/render/`'s differential tests are in it.
- Rust comments citing `sidecar/*.py` refer to commit `f875bb1`, where `mineru/render/`'s original lives.
