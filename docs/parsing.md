# Parsing — PDFs into per-page markdown

Every PDF in the library is read by **MinerU** — either its cloud service or a
MinerU server the user runs on their own Mac, chosen in Settings → Library.
Both are HTTP calls made in-process from Rust; neither is a process Oculus
starts or supervises. There is no fast tier and **no fallback between the
two**: one engine is selected, and a PDF either has markdown or it does not.
When it does not, the UI says so (see *The failure story* below).

This page replaces the old `sidecar.md`. The Python process it described is
gone from the app — see [architecture.md](./architecture.md) for what the two
remaining processes are.

## Where

| Piece | Location |
| --- | --- |
| The seam: trait, artifact contract, error vocabulary, version | `app/src-tauri/src/parse/mod.rs` |
| MinerU cloud protocol + the `Parser` impl | `app/src-tauri/src/parse/mineru/client.rs` |
| MinerU-on-this-Mac: the `Parser` impl and the probe | `app/src-tauri/src/parse/mineru/local.rs` |
| Content list → page records (what the markdown *says*) | `app/src-tauri/src/parse/mineru/render.rs` |
| Submission queue (batching window, in-flight cap) | `app/src-tauri/src/parse/mineru/batch.rs` |
| Daily allowance + the two rate limiters | `app/src-tauri/src/parse/mineru/ledger.rs` |
| The `parse-status` event | `app/src-tauri/src/parse/events.rs` |
| Call site, and the thread each parse parks on | `app/src-tauri/src/sync.rs` |
| Artifact paths and purging | `app/src-tauri/src/paths.rs` |
| MinerU keychain commands + the pre-store token probe | `app/src-tauri/src/mineru.rs` |
| Engine selection, endpoint override, the probe command | `app/src-tauri/src/parse/commands.rs` |
| Failure vocabulary, rendered | `app/src/lib/parseState.ts` |
| Live job state, per-file failures, the app-wide latch | `app/src/stores/parseStore.ts` |
| Background recovery sweep | `app/src/hooks/useQualitySweep.ts` |
| Settings UI (engine, token, endpoint, server status) | `app/src/components/settings/ParserSection.tsx` |
| The page that composes it | `app/src/pages/settings/LibraryPage.tsx` |

## The seam

`parse::Parser` is a trait, not a URL. Both implementations live in this
process: the cloud client speaks MinerU's public API over HTTPS, and
`mineru/local.rs` speaks MinerU's *own* server over loopback. Nothing in
`parse/mod.rs` may assume the cloud: an API root, a token, an upload ceiling
all arrive as configuration or as parameters.

What the seam owns is the **contract**, not the parsing:

- the on-disk artifact layout,
- `PARSER_VERSION`, which decides whether a file is already done,
- `ParseError`, the vocabulary the failure UI branches on,
- and the order the artifacts hit the disk.

`Health { backend, parser_version, ready }` is the version handshake. A backend
whose `parser_version` differs is **refused, naming both versions** — not
warned about and used anyway. The cloud client cannot disagree with itself, and
neither can the local one: both hand the content list to the same `render`, in
this process, so the record shape is ours by construction. What the local side
*can* disagree about is the **API**, which no version negotiation could
reconcile — see *The version pin* below. `ready` is where that lands: a server
that is not answering fails the handshake as `NotReady`, which is retryable, so
a stopped server never marks a file permanently broken.

`parse_config()` reads the `settings` row `parse`, key `engine` (`cloud` |
`local`) with an optional `engineUrl` override. Absence means `Cloud`.

**The old `backend` key is ignored entirely**, `"auto"` included. It named a
*fallback policy* over the Python sidecar — "local" meant that Python parser
specifically — so those values cannot be reinterpreted; promoting a stale
`"local"` would aim the app at a parse server nobody installed. The stale
`memoryCapMb` and `backend` keys are left in the blob rather than migrated out.

## Choosing an engine

Settings → Library owns the choice, over four commands in
`app/src-tauri/src/parse/commands.rs` — `parse_settings`, `parse_set_engine`,
`parse_set_engine_url`, `parse_probe_local`. They are deliberately the same
shape as `app/src-tauri/src/embed/commands.rs`, one field over: two adjacent
settings sections that behaved differently would be worse than either behaviour
alone.

**Switching is not destructive, and that is the whole difference from the
embedding control beside it.** Markdown from a local MinerU and markdown from
MinerU's cloud are the same artifact — same `PARSER_VERSION`, same `mode`,
rendered by the same shared `render`. Nothing already parsed is re-parsed,
nothing on disk is invalidated and no index is thrown away, so the engine
select has **no confirmation in front of it, and none should be added for
symmetry** with the embedding one. `embed_set_engine` destroys an index because
vectors from two models share a table, a width and a dot product and share no
geometry at all; nothing of the sort is true of markdown. A dialog raised over
a change that costs nothing is how people learn to click through the one that
matters. If a future backend ever does change the artifacts, `PARSER_VERSION`
is the thing that moves, and it moves for every backend at once.

Two smaller shapes:

- **Both engines are always offered.** Availability here is not a reachability
  question, and making it one would be a trap: someone has to be able to select
  Local and *then* go start their server, and a server that is merely stopped
  must not read as an engine that was never chosen. What is actually listening
  is a live status line beside the endpoint field, not a gate in front of the
  choice.
- **An `engineUrl` override belongs to one engine, so a switch drops it.**
  Carried across it would silently aim the new engine at the old one's address.
  *Emptying* the field is the revert-to-default gesture; typing the default in
  would store a literal copy of a constant and pin it across a release that
  moved it.

The MinerU token row renders **only under Cloud**. A key field standing under a
backend that cannot use it is the kind of thing people paste secrets into.

## The local server

`app/src-tauri/src/parse/mineru/local.rs` is ~190 lines of non-comment code
against the cloud client's 1,800, and the ratio is the design rather than an
omission.

MinerU's *public* API has no per-file endpoint — the batching, the signed
uploads, the poll loop, the daily ledger, the two token buckets and the
keychain all exist to serve that one fact. MinerU's *own* server offers `POST
/file_parse`: one multipart request in, one result ZIP out. None of that
machinery has anything to do here, and three things are absent by construction
rather than by omission — no quota to exhaust, no token to be rejected, no
batch for one document to condemn.

What is **not** duplicated is the part that matters. `render.rs` is shared
unchanged: it turns a content list into page records and does not know which
side produced the list. The local client's form fields are set to match the
cloud client's hardcoded parameters under this endpoint's names — `pipeline`
for the model, `ch` for the language, `return_content_list=true` — so the same
PDF renders the same markdown whichever engine a user picked. **They are pins,
not settings**; changing either of the first two is a re-parse of everything.

- **Progress is a page count and then a finish, with nothing in between.**
  `/file_parse` blocks until the whole document is done and offers nothing to
  subscribe to. The Python's local tier filled that silence with an EMA over
  previous parses — a bar that moves while nothing is known — and that is the
  one behaviour from it deliberately not ported. A counter that sits still is
  true; a bar that lies is not.
- **A connect timeout and no read timeout.** A parse on this machine's CPU is
  minutes of silence and that is not a hang, which is the rule the cloud path
  lives by for the same reason. Connecting either happens at once or the server
  is not running, so "unreachable" stays a useful word.
- **The default is `http://127.0.0.1:8000`, because that is where MinerU binds
  itself.** It used to be the port the Python sidecar vacated, on the reasoning
  that one number in a firewall rule beats two — which does not survive contact
  with a server this project does not build. A default nobody's server answers
  on is a setting every user must change before the engine works at all.

## The version pin

`POST /file_parse` is a **MinerU 3.x** endpoint. **MinerU 4.0.0, released
2026-09-16, removes it**, rebuilding the HTTP service around `/v1/health`,
`/v1/tiers`, `/v1/uploads`, `/v1/parse/jobs` and `/v1/files` — verified against
its release notes and its own 4.x migration guide, not inferred. So the install
Oculus documents pins below 4:

```bash
uv tool install -U "mineru[core]>=3.4,<4"
mineru-api --host 127.0.0.1 --port 8000
```

`uv` is assumed to be present already — Oculus does not install a package
manager on someone's machine. Installing directly rather than in a container is
the Apple Silicon path and not a preference: MinerU's own docs say **not** to
use Docker on macOS, where the container cannot reach MPS or MLX, and three of
its four Docker profiles reserve an NVIDIA GPU.

**Drop the `<4` and every parse 404s** — which is the first reason the range is
written out rather than left open. The second outlives a future V1 client: 4.0
still emits Content List V1, so `render.rs` would survive being pointed at one,
but 4.0 also replaced the `pipeline` backend with quality tiers
(`flash`/`basic`/`standard`/`advanced`), so a 4.x parse would **not** match the
cloud's markdown. The pin holds the two engines to one artifact, not merely to
one URL.

**The client detects that case by name rather than reporting silence.**
`local::probe` asks `/health`; on a 404 it asks `/v1/health`, and an answer
there means a MinerU that is running perfectly and simply speaks the other API.
Three states come back:

| State | What it means |
| --- | --- |
| `reachable` | `/health` answered `healthy` |
| `unreachable` | nothing answered — **or** a server answered and is not taking work yet, which is MinerU loading its models |
| `version_mismatch` | `/v1/health` answered: a MinerU 4 service, which has no `/file_parse` at all |

The settings page prints Rust's sentence rather than composing one from the
state, because Rust can tell those two `unreachable` causes apart and the page
cannot. `parse_probe_local` takes an **optional** URL so the endpoint field is
testable before it is saved; otherwise the only way to learn an address is
wrong is to commit to it first. On a cloud install it probes the local default
rather than MinerU's public root, which would be nonsense about a service
nobody runs here.

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

## How a cloud parse actually runs

Everything from here to *The token* is the cloud engine. The API's shape
dictates it. A **batch** of documents is submitted as a list of
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

**Concurrency belongs to the backend, not to a worker pool.** `sync.rs` spawns
one detached thread per PDF and the old bounded pool is gone — an inversion,
not a regression. The pool existed because every parse was an HTTP POST into
the sidecar and 105 decks meant 105 simultaneous POSTs at ~2 GB each; that was
the OOM. Each engine now answers for its own share of that. The cloud client
parks its threads on the batcher's condvar until their window closes — no
socket, no request in flight. The local client holds a single permit
(`PARSE_GATE` in `mineru/local.rs`), so only one multipart POST is ever open
against the user's server: that server is *this machine*, it works a document
at a time regardless, and a request left open on it has no read timeout above
it, so an ungated sync would be that same OOM again — same machine, same
memory, only in somebody else's process. A gate back in `sync.rs` would serve neither
engine: it would only keep cloud files out of the window they are meant to
share.

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

Settings states the privacy boundary under whichever engine is selected, as a
fact rather than an offer. Under Cloud: every PDF goes to MinerU and its
PRC-hosted OSS storage, and MinerU's documented 15-minute cache tolerance is
not a deletion guarantee. Under Local: nothing leaves the machine. That
difference is the one thing the two labels must never leave to be inferred.

## The failure story

Nothing catches a failed parse. The engine setting chooses *which* MinerU runs,
it does not stack them: a parse that fails on the selected engine is not
re-tried on the other one, ever. So a failure means that file has no markdown —
and with it no search, no `@`-mention and no Markdown view — until something
changes. `ParseError` exists to keep three answers distinguishable:

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
wrong, because a failure fell back to a local parser within the same run; every
parse is now one whole trip to whichever engine is selected — metered, on the
cloud — so `retryable === false` is never re-kicked and the sweep stands down
entirely under a latch, rather than marching the library through the same error
one batch at a time.

`NotReady` earns its row on the local engine: a server that is stopped, or
still loading its models, fails the handshake **retryable and non-latching**.
That pairing is deliberate. The sweep comes back once the server is up, and no
file is marked permanently broken for an engine that simply was not running
yet.

## History worth keeping

- **The fast tier is gone.** `pymupdf4llm` returned in ~2s and had to live in
  a throwaway subprocess because it leaked ~2 GB per deck. Nothing in the
  library was ever left in `fast`: checked 2026-09-16, `files.parse_status` was
  166 `quality` and 540 `NULL`, and all 161 readable records on disk said
  `"quality"`. Files passed through fast; they never rested there.
- **Formula decoding went pix2tex → docling enrichment → MinerU** (2026-08-15).
  MinerU is ~100× faster than docling-with-enrichment and more correct (1% vs
  18% KaTeX render failures on the benchmark deck).
- **The Python's own local parser is gone**, and with it the whole-tree memory
  governor, the 8 GB budget and the formula-batch cap. Parsing on this machine
  came back as MinerU's own server, which is a different arrangement entirely:
  nothing is bundled, nothing is supervised, and the models are somebody else's
  problem — so the shape those governors solved does not exist in this process.
  Those pins and their measurements are at `f875bb1`, the last commit holding
  `sidecar/`.
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
