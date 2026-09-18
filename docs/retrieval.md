# Retrieval

Semantic search over the library, built on **page images**, not extracted
text. There is no graph and no vector index — both were considered and
dropped, on measurement.

Embedding runs in-process in Rust, behind a seam shaped exactly like the
parser's (see [parsing.md](./parsing.md)). The Python sidecar and its local
Qwen model are gone; `voyage-multimodal-3.5` took their place.

## Where

| Piece | Location |
| --- | --- |
| The embed seam (trait, `.emb.json`, errors, config) | `app/src-tauri/src/embed/mod.rs` |
| Page rasterizer (pdfium), and the one session everything shares | `app/src-tauri/src/embed/raster.rs` |
| Voyage client — the `Embedder` | `app/src-tauri/src/embed/voyage/client.rs` |
| Allowance, throttle, tier detection | `app/src-tauri/src/embed/voyage/ledger.rs` |
| Which pages travel in one request | `app/src-tauri/src/embed/voyage/batch.rs` |
| What an outstanding run will cost, before it runs | `app/src-tauri/src/embed/estimate.rs` |
| Backend selection + throwing the index away | `app/src-tauri/src/embed/commands.rs` |
| The `embed-status` event | `app/src-tauri/src/embed/events.rs` |
| The serial run queue (app side) | `app/src/stores/indexStore.ts` |
| API key (keychain only) | `app/src-tauri/src/voyage.rs` |
| Ingest + brute-force cosine search | `app/src-tauri/src/retrieval.rs` |
| Lexical index over the same pages (`PAGES_FTS_SQL` + its tests) | `app/src-tauri/src/retrieval.rs` |
| Lexical query path (frontend) | `searchPageText` in `app/src/lib/db.ts` |
| Per-page markdown source | `app/src-tauri/src/parse/mod.rs` (`.pages.json`) |
| Who writes `pages.markdown` | `app/src-tauri/src/sync.rs` (the parse path) |
| `pages` table schema | migrations in `app/src-tauri/src/lib.rs` |
| Frontend query path | `app/src/lib/retrieval.ts` |
| Terminal query path | `app/src-tauri/src/bin/oculus.rs` (`oculus search`) |
| Smoke test | `app/src-tauri/src/bin/retrieval_smoke.rs` |

## The flow

`embed::backend()` returns the embedder the settings row selects — never a
concrete client named at a call site. It rasterises each PDF page with pdfium
and sends the pixels to Voyage → the vector lands in the `pages` table beside
that page's markdown, keyed on `(file, page_no)`, **stamped with the model and
width that produced it** → a query is embedded by the same backend, on the
other side of the model's asymmetry → Rust ranks by dot product over every
stored vector *in that same space* → hits carry the markdown for the answer and
the `(file, page)` ref for the deep link. **Nothing downstream of ranking
touches a vector** — a future LLM sees only markdown and citations.

The `.emb.json` beside each PDF is the record that a file is indexed:
`{pdf, model, dim, dtype, instruction, page_count, pages: [{page_no, vector}]}`,
written temp-then-rename so it becomes visible in one step. It is the same wire
shape Python wrote, so old records still deserialise — they simply do not
*match*.

## Decisions and the numbers behind them

### Page images, still (2026-08-15, and still load-bearing)

Benchmarked on real course decks (152-page corpus, then re-run at 908 pages
with topically adjacent distractors). **This argument did not move with the
backend** — it is why `raster.rs` exists at all, since MinerU's ZIP returns
cropped figures and never page rasters.

- **Image embeddings, alone.** On ordinary questions image ties text; on
  formula/screenshot/diagram pages (where text extraction yields garbage
  like `56 = 7(( 7() 7)(`) image roughly doubles recall and text never
  recovers even at rank 3. Averaging image and text vectors scored *worse*
  than image alone — don't hybridize at the vector level.
- **512 dims.** Measured indistinguishable from the local model's native
  2048 under Matryoshka truncation, at a quarter the storage. Voyage honours
  `output_dimension: 512` directly, so the stored blob is unchanged at
  1024 bytes = 512 × f16 and the column needed no migration.
- **Brute-force scan, no index.** A degree of coursework is a few thousand
  pages; at 512 dims that is single-digit MB and milliseconds. Scale was
  checked: a 6.5× bigger adjacent-topic library cost one query of recall.
  Do not "optimise" this into an ANN index — there is no payoff to buy the
  machinery with.

### One pdfium session at a time (measured 2026-09-17)

`raster.rs` holds a process-wide lock across every document it opens, and
`pdfium-render`'s `thread_safe` feature — which is enabled — is **not** a
substitute. It serialises individual FFI calls; the unit that has to be atomic
is the whole session, from `load_pdf_from_file` to the last page.

Two threads sweeping the same 166-file library: **0 failures serially, 18 per
thread concurrently.** Both failure modes matter:

- `PdfiumLibraryInternalError(Unknown)` loading a page — torn document state.
- **`Encrypted`, on files that are not encrypted.** `FPDF_GetLastError()` is
  process-global and is a *separate* call from the one that failed, so a thread
  reports whichever error another thread left behind. That is the dangerous
  one: a confident, specific, wrong diagnosis. It reached Settings → Library as
  "47 files could not be measured", on a library where every file is fine.

The hold can be long — an embed's render pass is a whole document — so the
trade is explicit: a caller may wait, and no caller gets a torn answer.
`render_page` renders one page instead of sweeping the document for it, which
is both cheaper and a shorter hold on the interactive path.

The trigger was React's StrictMode firing the settings page's estimate effect
twice, so the app asked for two concurrent library-wide sweeps. That is fixed
on its own side too — `loadEstimate` runs one sweep and remembers a request
that arrives during one — but the lock is what makes the invariant hold for any
caller, including `oculus index` running beside the app.

### A page too big to send is rendered smaller, not refused (2026-09-17)

Voyage refuses any single image over 16M pixels, and a page is not something
that can be split — so one poster page failed its whole document, permanently,
and no retry could change that. Measured in the real library: `networking.pdf`
is **146 pages of 5334 x 3000 = 16,002,000 px**, two thousand pixels over the
line on every page, and `UI_Prototype.pdf` has two A0-ish pages at 23.9M in an
otherwise ordinary deck. Three files, no markdown-free reason, nothing
searchable.

`raster::dpi_for_page` clamps the DPI for exactly those pages, and the reason
it costs nothing is `BILLED_PIXEL_CAP`: Voyage downscales to 2,000,000 px
before it encodes or bills, so every pixel between 2M and 16M is already
thrown away. A clamped page is still ~8x past the point where more pixels are
looked at, bills the identical 2M, and yields a vector from the image the model
would have made anyway. **Only pages that would otherwise be refused are
touched** — every page in the library that embeds today still renders
byte-identically at 200 DPI, which is what keeps it comparable with what is
already stored.

The ceiling travels *in* from `voyage/batch.rs` rather than being a constant in
the rasterizer, which does not know which backend is asking.
`batch::refuse_oversized` stays as the floor under it: the byte ceiling and the
token ceiling are not things a DPI fixes, and a page that is still too big at
1 DPI is a genuine document error. **Skipping the page was the alternative and
is still wrong** — it would write the short record the whole module is arranged
to prevent, and page 23 would be silently unsearchable forever.

`raster::page_sizes` reports the unclamped size, and the estimator is right
anyway: both sides of the clamp are past the 2M cap, so the predicted pixels,
tokens and requests are unchanged.

### The cloud model (verified live 2026-09-17, not read off the docs)

- **`voyage-multimodal-3.5`.** Vectors come back already L2-normalised
  (measured ‖v‖ = 1.0019 on a real page), so the dot-product-is-cosine
  assumption holds with no renormalisation.
- **`input_type` is the asymmetry**: `"document"` for pages, `"query"` for
  queries. Using one for both is not an error anything detects; it just ranks
  worse. `embed::QUERY_INSTRUCTION` is the *name* of that convention — it is
  compared, never sent.
- **`output_encoding: "base64"`** returns a base64 NumPy array (2048 bytes =
  512 × f32 little-endian), converted to f16 in Rust. `output_dtype` is a
  different parameter and does not accept `base64`; confusing the two is the
  easy mistake here.

### Rate limits, and why they shape the code

- **No payment method on file means 3 RPM / 10K TPM**, which the API states in
  its own error body. With a card it is tier 1: 2000 RPM / 2M TPM.
- **The free grant is per *account*, not per programme, and that inverts the
  usual advice.** Voyage's pricing page gives every account the first 200M text
  tokens and 150B pixels free, and charges $0.60 per billion pixels after that;
  images bill by pixels, not tokens, capped at 2,000,000 an image. The whole
  library is a few percent of that grant (below), so **adding a payment method
  does not make it cheaper, because it is already free.** What a card buys is
  the per-minute ceiling: the same run is ~18 hours on the free programme and
  ~5 minutes on tier 1. Any UI that offers a card as a way to avoid a charge is
  saying something false; the honest pitch is the clock.
- **A single request larger than the per-minute ceiling is refused outright** —
  ~14,284 tokens against a 10K TPM account returns 429, with **no
  `Retry-After`**. So the per-request token budget is derived from the
  *learned* tier, not only from the API's 320,000 maximum. A batcher that
  always packs to 320K on the free tier cannot make progress at any pace: that
  is a livelock, not slowness.
- **The tier is detected, never declared**
  (`app/src-tauri/src/embed/voyage/ledger.rs`). A 429 is routine, not
  fatal — `EmbedError::RateLimited` is retryable and deliberately **not
  latching**, because a 429 that stopped the run would make the free programme
  unusable rather than slow and would read to a student like a broken key.
- **Voyage downscales images to ~2,000,000 px before billing**, measured: two
  copies of a 2339×1653 page (3,866,367 px each) billed 4,000,000 total. So a
  200-DPI landscape-A4 slide costs ~2M px ≈ 3,571 tokens. On the free
  programme that is **~2.8 pages a minute**; on tier 1 the same library is
  minutes.
- The library bills **5.18B px across 166 files / 2,980 pages — 3.5% of the
  150B free pixel allowance** (measured 2026-09-17 with `raster::page_sizes`,
  each page capped at 2,000,000 as Voyage caps it). The pixel budget is not the
  constraint; the per-minute ceiling is.
- **`RENDER_DPI` stays 200** (`app/src-tauri/src/embed/raster.rs`) even though
  ~144 DPI is the number that lands exactly on the 2M-pixel cap. Anything
  above it is downscaled by Voyage before it is looked at, so extra DPI costs
  upload bandwidth only — not tokens, not quality. It is kept because it is the
  number every page artifact in the library was rendered at.
- `voyage-usage.json` in the data dir holds the reservations, the latched
  quota, the learned tier **and the spend guard**, atomically and across
  restarts — the same discipline as `mineru-usage.json`.
- **The spend guard lives in that file rather than in the `settings` row**, and
  the reason is mechanical: the reservation that enforces it already reads the
  ledger on every request, so the two are one atomic read, and the
  process-global `UsageLedger::shared()` picks a change up without being
  rebuilt. It is a percentage of the free grant, defaulting to 100 — stop
  before this starts costing money — and `0` turns it off, because past the
  grant is a price, not a wall.
- **It binds a paid account too, which reverses the older rule.** The grant
  check used to be skipped when the tier read as free-plus, on the grounds that
  refusing a paid account's work against a free pool would be a limit the app
  invented. A percentage the user set is not invented, and past 150B pixels a
  paid account is the one actually being billed. `EmbedError::BudgetReached`
  is its own variant rather than a `QuotaExhausted` in disguise: an allowance
  repairs itself and is worth retrying, a setting does not and is not.
- **`embed_estimate` answers "what will this cost" without sending anything.**
  pdfium reads the page boxes (`raster::page_sizes`, no rasterising), the
  pixels are billed the way Voyage bills them, and `batch::plan` packs them at
  the ceiling in force — the same function the run uses, so the predicted
  request count is the one that happens. Over ~166 files it is a few seconds of
  file I/O, which is why it is a separate command from `embed_settings` and why
  Settings → Library draws around it.

## The other search over the same table

`pages` carries two indexes over the same rows, and they answer two different
questions.

- **The embeddings answer a question.** "Where is the worked example on
  Dijkstra" is a meaning, it costs a cloud round trip per query, and it is what
  Chat retrieves through.
- **`pages_fts` answers a keystroke.** An FTS5 index over `pages.markdown`
  (migration 35, SQL in `retrieval::PAGES_FTS_SQL`), read by `searchPageText`
  in `app/src/lib/db.ts` and shown in the ⌘K palette and the new-tab field
  (see [frontend.md](./frontend.md)). It is local, it is milliseconds, and it
  finds a person's *exact* words — a lecturer's turn of phrase, a term you half
  remember — which is the one thing an embedding is bad at and a title search
  cannot see at all.

Neither is a fallback for the other and they are never merged into one
ranking. They are also not the same coverage: only *parsed* documents have
page markdown, so both indexes stop where parsing does.

**External content, and the one leak in it.** The index stores terms rather
than a second copy of every page (`content='pages'`), and three triggers keep
it in step with whichever writer moved — the app through `tauri-plugin-sql`,
the CLI through `app/src-tauri/src/store.rs`. The update trigger is
`UPDATE OF markdown`, not a bare `UPDATE`, because every embed writes a blob to
these rows and re-indexing the text for that would be work for nothing. What
it does *not* catch is a page deleted by `files`' `ON DELETE CASCADE`: SQLite
runs triggers for foreign-key actions only with `recursive_triggers` on, so the
index can hold entries whose page is gone. That costs ranking, never
correctness — every read joins `pages ON pages.id = pages_fts.rowid` and an
entry with no page behind it drops out of the join.
`INSERT INTO pages_fts(pages_fts) VALUES('rebuild')` is the cure if one is ever
wanted.

**FTS5 is a hard dependency of opening the database at all**, because
migration 35 creates the virtual table. `libsqlite3-sys`' bundled build defines
`SQLITE_ENABLE_FTS5`; `retrieval.rs`'s `fts_tests` is the assertion that it
still does, and it exercises the triggers against the same SQL string the
migration runs rather than a copy of it.

## One space, or the ranking is noise

This is the constraint the whole module is arranged around.

Two embedding spaces in one `pages` table produce **nothing visible at all**.
The scan runs, every dot product returns a number, the results sort, and the
ranking is noise — a search that returns confident, well-formatted, unrelated
slides. That is strictly worse than an error, because nothing looks broken.

So:

- `embed::Health::check` **refuses** a backend whose model or dim differs from
  the app's, rather than warning. `embed::preflight` is the only way a call
  site is allowed to reach a backend.
- **Every scan in `retrieval.rs` filters on `pages.embed_model` and
  `pages.embed_dim`**, against the space the preflighted backend named. A
  vector from a retired model is never compared against a query — it is not
  deleted, it is simply not in the index.
- **`IndexStats` reports both numbers.** `pages_embedded` is what is
  searchable *now*; `pages_stored` is every blob in the table; `pages_stale`
  and `stale_models` are the difference, named. A type with only one of these
  would say something false about a library that has been embedded by a model
  the app no longer uses — which is exactly the state every install was in the
  moment the local model was retired (2,980 pages, 166 files, all
  `Qwen/Qwen3-VL-Embedding-2B`).
- **`embed::is_embedded` decides what still needs work**, and it checks model,
  dim, instruction *and page coverage* against the parse record. Coverage is
  part of the question because a cloud backend can lose one page to a rate
  limit mid-document, where the local one was all-or-nothing; without the count
  check a document that lost page 47 would read as embedded forever.
- Correspondingly, **`getUnembeddedPdfs` counts current-space page vectors
  rather than reading `files.embed_status`**. That column is a sticky flag with
  no memory of which space it was set in, so after the model change it claimed
  `'done'` for all 166 stranded files. The backlog has to follow the space.
- Changing the engine in settings therefore **throws the index away in the same
  call** (`app/src-tauri/src/embed/commands.rs`): records first, then the
  table, then the setting — so a failure can leave an index to rebuild but
  never a setting that claims one space while the table holds another.

There is no migration path between spaces and there is not meant to be. A
re-index is just a re-run — `oculus index`, or **Build the index** in
Settings → Library — and that is only true because `is_embedded` already
rejects the old records. There is no reset step and no migration script to
write; do not add one.

## How it connects

- The page is the chunk. Slide-deck pages run ~90–760 chars of markdown, so
  there is no sub-chunking anywhere.
- **`pages.markdown` is the parse's write, not the embedder's.** It used to
  arrive only as a side effect of `retrieval::ingest`, which made the text
  `oculus grep` searches depend on the vector index having been built. A
  finished parse writes its own page records now (`store::upsert_pages`), and
  an `oculus index` over an already-parsed file folds its `.pages.json` in if
  nothing ever did. Ingest still upserts markdown alongside the vector, and
  both sides use the same conflict rule: an empty incoming page never
  overwrites text already stored.
- Ingest is idempotent, and in two halves. A PDF whose record is already in the
  current space is not re-embedded, but its record is still folded into
  `pages` — an artifact on disk is no promise the database can see it.
- **Embedding blocks for the whole round trip, and there is no timeout at the
  call site.** The client paces itself against the tier it detected; a second
  deadline imposed from above could only abandon work that was still
  progressing. Same rule as the parse path. What the call site owes instead is
  an honest counter, which is what `ingest_reporting`'s `ProgressSink` carries
  — `oculus index` renders it as an in-place line, and the app now emits it as
  **`embed-status`** (`app/src-tauri/src/embed/events.rs`), a deliberate copy
  of `parse-status` down to the field names. The app counted files and could
  not count pages, and that was the whole of the gap: one document is one
  blocking call, so a 200-page deck was an hour of a filename that never
  changed. `embed_file` takes a `subject_id` purely so the event can be keyed
  the way a pipeline row is, and the events are emitted *around* `ingest` in
  the command rather than inside it, because `ingest` is also the CLI's path.
- **A failure carries its discriminants out of the ingest now**, not just a
  sentence. `retrieval::IngestError` is `{message, kind, retryable, latching}`
  — the same three questions `ParseError` answers, optional because a failure
  that never reached a backend (no file on disk, a refused write) has no
  `EmbedError` behind it and unknown is its own case. The string alone was
  enough while the only caller was a terminal printing it; a row that has to
  decide whether to offer a retry cannot recover them from prose.
- **The app runs the index itself, as a queue** (`app/src/stores/indexStore.ts`,
  drawn by Settings → Library). `embedFile`, `searchPages`, `embeddingStats`
  and `getUnembeddedPdfs` in `app/src/lib/retrieval.ts` had no caller at all
  for a while — every one of them wired to Rust and reachable only from the
  CLI — which is how a library ended up parsed, unsearchable, and silent about
  it. The loop that replaced that is now **a queue with one worker**, because
  work arrives from three places and must never become two runs: the Index
  button enqueues the whole backlog, a finished parse enqueues one file, and a
  row's retry in File Activity enqueues one. Stopping is polled between files,
  never during one — a run that may last hours has to be interruptible, but
  abandoning a document mid-flight would re-pay for its pages next time. It
  also stops *itself* when a failure turns out to be account-wide:
  `embed_blocked` asks the ledger's two latches — spent allowance, reached
  spend limit — after a file fails, because those condemn every file still
  queued for the identical reason, and a loop that carried on would report one
  fact as a hundred errors.
- **Embedding follows a parse automatically, once there is a key.** The hop is
  one line in `app/src/hooks/useBackendEvents.ts`: a terminal `parse-status`
  enqueues that file, so a sync runs download → parse → embed end to end and
  the pipeline table has a third dot to show it. The gate is
  `retrieval::embedReady` — a stored key *and* a selected engine this build
  ships — and with it false nothing is queued and the stage is not drawn,
  because an app that embedded before it had somewhere to embed to would write
  one failed row per parsed file. The **backlog** is deliberately not swept up
  by this: 166 already-parsed files are hours of metered work and the estimate
  on the settings page exists to be read first.
- Two callers rank against the same store: `search_pages` in the app and
  `oculus search`. The ⌘K palette is **not** a third one — it matches titles
  in SQLite so it can answer every keystroke; see
  [frontend.md](./frontend.md). `search_in` takes a set of subject ids because
  the CLI accepts prefix codes, which can match the same subject in two terms;
  `search` is the single-subject wrapper the Tauri command uses. Both embed the
  query once and the subject filter is SQL, so neither pays per course.
  See [cli.md](./cli.md).
- Embedding and parsing meet **only** on `(file, page_no)` via `.pages.json`.
  If page attribution breaks in the parser, retrieval silently returns the
  wrong markdown for a correct visual hit. The Voyage client checks pdfium's
  page count against the parse record's before it bills a single pixel, for
  exactly this reason — pdfium and `lopdf` can genuinely disagree on a damaged
  xref.
- The key lives in the macOS keychain and nowhere else — not SQLite, not the
  WebView, not a health response, not a progress event. No `EmbedError` variant
  carries server response text, because an error body can echo the request,
  which for this API means an echo of the base64 page image.

## Not yet wired

Honest gaps, so nobody goes looking for them:

- **`searchPages` has no caller in the app.** Semantic search is reached from
  the CLI (`oculus search`), because chat is a CLI agent now and it reads the
  library through `oculus search` and `oculus grep` rather than through the
  WebView. There is no in-app results page for it. It is the tested path the
  CLI uses, reached from TypeScript, not dead code — but nothing in the UI
  invokes it. The rest of `app/src/lib/retrieval.ts` does have callers now:
  `embedFile` from `indexStore`'s worker, and `getUnembeddedPdfs` (and through
  it `embeddingStats`) from the Index button and the Sync page's seed.
- `Engine::Local` is a real arm of the seam pointing at a loopback server that
  ships from its own repo. It resolves to `EmbedError::NotReady` rather than
  silently falling back to the cloud, because embedding into a space the user
  did not choose is what `Health::check` exists to prevent.

## How much memory a local model can actually have

Nothing in the app measures this today — embedding is a cloud call and chat is
a CLI agent. The method is kept here because it was **measured rather than
reasoned**, and because it is what `Engine::Local` (and any local provider that
comes back) will need on its first afternoon rather than rediscover. The
deleted BYOK layer ran this preflight before every local model call, because a
local model that does not fit is not slow — it is an OOM that takes the machine
down, observed with a 17 GB Ollama model loading beside the old Qwen3-VL
embedder on a 36 GB machine.

- **Available memory is physical memory (`sysctl hw.memsize`) less wired
  pages, not free pages.** macOS keeps almost nothing free, compressing
  anonymous memory and evicting file cache on demand: measured on the 36 GB
  dev machine while it was happily serving a 17.4 GB resident model, free +
  inactive + speculative + purgeable came to 2.9 GB — a figure that refuses
  every model there is. Wired pages are the ones the kernel cannot page out,
  and on Apple silicon that is exactly where GPU-resident weights live, so
  they are the ones worth counting. Everything else is compressible or
  swappable.
- **Read the page size out of `vm_stat`'s own header.** It is 16 KB on Apple
  silicon; assuming the historical 4 KB miscounts wired memory fourfold.
- **Budget weights at 1.15×** for the KV cache and runtime — measured, a
  16.5 GB Q4_K_M 27B sits at 17.4 GB resident at a 32k context.
- **A model already resident needs nothing at all** and is never refused
  whatever the arithmetic says; what a runtime already holds loaded counts as
  available besides, because it evicts to make room.
- **An unmeasurable model skips the check rather than blocking on a guess.**
  Sizes came from Ollama's native `/api/tags`; the OpenAI-compatible
  `/v1/models` carries no size and LM Studio exposes none.
