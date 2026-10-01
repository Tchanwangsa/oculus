# Parsing — PDFs into per-page markdown

Every PDF is read by **MinerU** — its cloud service, or a MinerU server the
user runs on their own Mac over loopback — chosen in Settings → Library. Both
are HTTP calls made in-process from Rust; Oculus never starts or supervises
either. MinerU is ~100× faster than docling with formula enrichment, with 1% vs
18% KaTeX render failures on the benchmark deck.

## Where

| Piece | Location |
| --- | --- |
| The seam: trait, artifact contract, `ParseError`, `PARSER_VERSION` | `app/src-tauri/src/parse/mod.rs` |
| MinerU cloud protocol + its `Parser` | `app/src-tauri/src/parse/mineru/client.rs` |
| Local MinerU server: its `Parser` and the probe | `app/src-tauri/src/parse/mineru/local.rs` |
| Content list → page records (shared by both engines) | `app/src-tauri/src/parse/mineru/render.rs` |
| Cloud submission queue (window, in-flight cap) | `app/src-tauri/src/parse/mineru/batch.rs` |
| Cloud daily allowance + the two rate limiters | `app/src-tauri/src/parse/mineru/ledger.rs` |
| Token bucket, semaphore, retry ladder (shared with Voyage) | `app/src-tauri/src/ratelimit.rs` |
| The `parse-status` event | `app/src-tauri/src/parse/events.rs` |
| Call site; one thread per PDF; LibreOffice conversion | `app/src-tauri/src/sync.rs` |
| Artifact purging | `app/src-tauri/src/paths.rs` |
| MinerU keychain commands + the pre-store token probe | `app/src-tauri/src/mineru.rs` |
| Engine selection, endpoint override, local probe | `app/src-tauri/src/parse/commands.rs` |
| Failure vocabulary, rendered | `app/src/lib/parseState.ts` |
| Live job state, per-file failures, the app-wide latch | `app/src/stores/parseStore.ts` |
| Background recovery sweep | `app/src/hooks/useQualitySweep.ts` |
| Settings UI (engine, token, endpoint, server status) | `app/src/components/settings/ParserSection.tsx` |

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

- **`kind()` is a frozen vocabulary**; `Display` is prose and may change.
  `parseState.ts` matches `/credential|token/i` on `kind` to decide whether to
  point at Settings, which is why both credential variants carry that word.
- **`latching` separates "this file is broken" from "parsing is down for
  everything"**. Both discriminants are optional — a failure from a previous
  session is only `error` in the DB — and unknown is never coerced into either.
- **The sweep reads the same discriminants**: `retryable === false` is never
  re-kicked, and a latch stands it down (one probe file after
  `LATCH_PROBE_AFTER_MS`).
- **`NotReady` is retryable and non-latching on purpose**: a local server that
  is stopped or still loading must not mark any file permanently broken.

## A parse takes minutes, and only the engine bounds it

`parse_pdf` blocks for the whole round trip; every caller must be able to wait.
Nothing above the client imposes a deadline — it could only abandon work that
was still progressing.

- **Cloud**: `POLL_DEADLINE` (60 min) in `mineru/client.rs` is the only one.
- **Local**: a connect timeout and no read timeout. `/file_parse` is minutes of
  silence on this machine's CPU, not a hang; connecting is instant or the
  server is not running.
- **Progress is counted, never inferred**: cloud sums per-task page counts;
  local reports a page count and then a finish, with no estimated bar between.

**Concurrency belongs to each engine.** `sync.rs` spawns one detached thread
per PDF. Cloud threads park on the batcher's condvar until their window
closes. The local client holds one permit (`PARSE_GATE`) because `mineru-api`
reports `max_concurrent_requests: 1` — a second request would only queue
inside that server on a socket of ours. A gate in `sync.rs` would keep cloud
files out of the window they are meant to share.

## `.pages.json` is the only evidence a parse finished

The seam owns the contract, not the parsing: nothing in `parse/mod.rs` may
assume the cloud. Beside each PDF:

- `<stem>.md` — full-document markdown, derived from the record.
- `<stem>.pages.json` — `{pdf, mode, parser_version, page_count, pages:
  [{page_no, markdown}]}`, keyed by 1-based `page_no`: **the join key
  retrieval rests on** ([retrieval.md](./retrieval.md)).
- `<stem>_images/` — its *name* is the link prefix written into the markdown,
  so both are computed in `parse/mod.rs`, never by a backend.

**The record is written last, via temp+rename**, so an interrupted parse
cannot look finished. `parse_mode` reads only `mode`: `"quality"` means done,
anything else (missing, unreadable, another mode) means parse it. The string
is frozen, and there is one tier.

`PARSER_VERSION` moves only when the artifacts change shape. `parse_mode` does
**not** check it, so a bump never re-parses the library; it is enforced by the
`Health` handshake, which refuses a backend with another version, naming both.

A result with no content list writes **nothing**. Changed bytes purge the
artifacts (`paths::purge_parse_artifacts`) before re-parsing, or a stale
record would serve old markdown forever.

## Both engines produce the same artifact, so switching costs nothing

`render.rs` is shared: both engines hand it a MinerU content list. The local
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

`local::probe` returns `reachable` (`/health` said `healthy`), `unreachable`
(nothing answered, or it is still loading models) or `version_mismatch`
(`/health` 404s but `/v1/health` answers: a MinerU 4); Settings prints Rust's
sentence, which alone tells the `unreachable` causes apart.
`parse_probe_local`'s optional URL tests the endpoint field before it is saved.

The ignored test `a_real_mineru_answers_the_way_this_client_expects` in
`mineru/local.rs` is the only check that the form fields match a real server.

## The cloud engine batches because the API has no per-file endpoint

A batch is a list of names; MinerU returns one signed upload URL each, every
file is `PUT`, and one endpoint is polled until each task has a result ZIP.
The window is **5 s or 20 files**, with **8 batches in flight**.

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

## Gotchas

- Never add a fallback between engines or let a failure look like success — a PDF silently loses its markdown.
- Never wrap a parse in a timeout — it abandons parses that were progressing.
- Don't add a confirmation to the parse engine select — switching invalidates nothing.
- Don't drop the `<4` from the install line — every local parse 404s.
- Don't write `.pages.json` before the other artifacts — a crash then reads as a finished parse.
- Don't remove `PARSE_GATE` — sockets park for minutes in a single-request server's queue.
- `*.pptx.pdf` (LibreOffice-converted) has never been parsed in this library and has no fixture.
- Golden fixtures live in gitignored `data/parse-fixtures/` (`shasum -a 256 -c MANIFEST.sha256`) and stay out of the repo; `render.rs`'s differential tests are in it.
- Rust comments citing `sidecar/*.py` refer to commit `f875bb1`, where `render.rs`'s original lives.
