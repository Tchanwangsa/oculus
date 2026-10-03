# Retrieval

Semantic search over the library, built on **page images**, not extracted
text, embedded in-process from Rust by `voyage-multimodal-3.5` behind a seam
shaped like the parser's ([parsing.md](./parsing.md)). A lexical FTS5 index
over the same pages sits beside it.

## Where

| Piece | Location |
| --- | --- |
| The embed seam (trait, `.emb.json`, `EmbedError`, config, handshake) | `app/src-tauri/src/embed/mod.rs` |
| Page rasterizer (pdfium) and its process-wide session lock | `app/src-tauri/src/embed/raster.rs` |
| Voyage client — the `Embedder` | `app/src-tauri/src/embed/voyage/client.rs` |
| Allowance, spend guard, throttle, tier detection | `app/src-tauri/src/embed/voyage/ledger.rs` |
| Request packing and per-image ceilings | `app/src-tauri/src/embed/voyage/batch.rs` |
| Token bucket, semaphore, retry ladder (shared with MinerU) | `app/src-tauri/src/ratelimit.rs` |
| Cost of an outstanding run, before it runs | `app/src-tauri/src/embed/estimate.rs` |
| Engine selection, throwing the index away, `embed_blocked` | `app/src-tauri/src/embed/commands.rs` |
| The `embed-status` event and shared wire payload | `app/src-tauri/src/embed/events.rs`, `app/src-tauri/src/pipeline_events.rs` |
| API key (keychain only) | `app/src-tauri/src/voyage.rs` |
| Ingest, brute-force cosine search, `PAGES_FTS_SQL` + `fts_tests` | `app/src-tauri/src/retrieval.rs` |
| Who writes `pages.markdown` (`store::upsert_pages`) | `app/src-tauri/src/sync.rs`, `app/src-tauri/src/store.rs` |
| `pages` table schema | `app/src-tauri/src/migrations.rs` |
| The serial index queue (app side) | `app/src/stores/indexStore.ts` |
| Frontend embed calls, `embedReady`, backlog query | `app/src/lib/retrieval.ts` |
| Lexical query (`searchPageText`) | `app/src/lib/db.ts` |
| Parse → embed hop | `app/src/hooks/useBackendEvents.ts` |
| Terminal query path (`oculus search`) | `app/src-tauri/src/bin/oculus/query.rs` |
| Smoke test | `app/src-tauri/src/bin/retrieval_smoke.rs` |

## Page images are embedded, not text — measured

On real course decks (152 pages, re-run at 908 with topically adjacent
distractors), image ties text on ordinary questions and **roughly doubles
recall on formula/diagram/screenshot pages**, where extracted text is garbage
and never recovers even at rank 3. Averaging image and text vectors scored
*worse* than image alone. This is why `raster.rs` exists: MinerU returns
cropped figures, never page rasters.

- **512 dims**: indistinguishable from 2048 under Matryoshka truncation, at a
  quarter the storage; stored as 1024 bytes = 512 × f16.
- **Brute-force scan, no ANN index**: a degree is a few thousand pages, single-
  digit MB at 512 dims; a 6.5× bigger library cost one query of recall.
- **The page is the chunk**: slide pages run ~90–760 chars, so nothing is
  sub-chunked.

## Nothing downstream of ranking touches a vector

`embed::backend()` returns the embedder the settings row selects; call sites
never name a client. Each page is rasterised and sent → the vector lands in
`pages` beside that page's markdown, keyed on `(file, page_no)` and **stamped
with the model and dim** → a query is embedded by the same backend → Rust ranks
by dot product over vectors *in that space* → hits carry markdown and the
`(file, page)` ref.

`.emb.json` beside each PDF records that it is indexed (`{pdf, model, dim,
dtype, instruction, page_count, pages}`), written temp-then-rename.

Embedding and parsing meet **only** on `(file, page_no)` via `.pages.json`, so
the Voyage client checks pdfium's page count against the parse record's before
billing a pixel — pdfium and `lopdf` can disagree on a damaged xref.

## One space, or the ranking is noise

Two embedding spaces in one `pages` table produce no error: every dot product
returns a number and the results are confident, unrelated slides. So:

- `embed::Health::check` **refuses** a backend whose model or dim differs;
  `embed::preflight` is the only way a call site reaches a backend.
- **Every scan in `retrieval.rs` filters on `pages.embed_model` and
  `pages.embed_dim`.** A vector from another model is not deleted, just never
  compared.
- **`IndexStats` reports `pages_embedded` (searchable now) and `pages_stored`
  (every blob)**, with `pages_stale`/`stale_models` naming the difference.
- **`embed::is_embedded` checks model, dim, instruction *and* page coverage**
  against the parse record — a cloud run can lose one page to a refusal, and
  without the count that page would never be retried.
- **`getUnembeddedPdfs` counts current-space vectors, not
  `files.embed_status`**, a sticky flag with no memory of which space set it.
- **Changing the engine throws the index away in the same call**
  (`embed_set_engine`): records first, then the table, then the setting — a
  failure can leave an index to rebuild, never a setting that names one space
  while the table holds another. Re-selecting the current engine is a no-op.

A re-index is just a re-run (`oculus index`, or **Build the index**), since
`is_embedded` rejects old records; don't write a migration between spaces.
`Engine::Local` ships no embedder in this build: Settings refuses it and it
resolves to `EmbedError::NotReady`, never to the cloud.

## Voyage's rate-limit tier is why an embed can take an hour

- **No payment method means 3 RPM / 10K TPM; with a card, tier 1 is 2000 RPM /
  2M TPM** (`FREE_*`/`TIER1_*` in `voyage/ledger.rs`). A 200-DPI slide bills
  ~2M px ≈ 3,571 tokens, so the free tier is **under three pages a minute** —
  a large deck genuinely takes an hour; tier 1 does the library in minutes.
- **Voyage downscales every image to 2,000,000 px before billing**
  (`BILLED_PIXEL_CAP`). `RENDER_DPI` stays 200 anyway: pixels past the cap cost
  bandwidth, not tokens or quality, and every stored vector was rendered at 200.
- **The tier is detected, never declared.** A 429 is routine:
  `EmbedError::RateLimited` is retryable and **not latching**, or the free tier
  would read as a broken key instead of a slow one.
- **The per-request token budget follows the learned tier**, not only the API's
  320K maximum: a request larger than the per-minute ceiling is refused with a
  429 and no `Retry-After`, so packing to 320K on the free tier is a livelock.
- **The free grant is per account** — 150B pixels, then $0.60 per billion — and
  the whole library is a few percent of it. A card buys speed, not savings; UI
  that offers a card to avoid a charge would be false.

**No deadline sits above the client.** `ingest` blocks for the whole round trip
and the client paces itself; a second deadline could only abandon progressing
work (same rule as [parsing.md](./parsing.md#a-parse-takes-minutes-and-only-the-engine-bounds-it)).
The call site owes an honest page counter instead: `ingest_reporting`'s
`ProgressSink`, emitted by the app as `embed-status` (the shared pipeline payload)
*around* `ingest`, because `ingest` is also the CLI's path.

## The ledger and the spend guard

`voyage-usage.json` in the data dir holds reservations, the latched quota, the
learned tier and the spend guard, atomically across restarts — the same
discipline as `mineru-usage.json`.

- **The spend guard lives in the ledger, not the `settings` row**, because the
  reservation already reads the ledger on every request and
  `UsageLedger::shared()` sees a change without a rebuild. It is a percentage
  of the free grant (default 100; `0` disables it) and binds paid accounts too.
- **`EmbedError::BudgetReached` is not `QuotaExhausted`**: an allowance heals
  and is worth retrying, a user's setting is not.
- **`embed_estimate` sends nothing.** pdfium reads page boxes
  (`raster::page_sizes`, no rasterising) and `batch::plan` packs them at the
  ceiling in force — the function the run uses — so the predicted request count
  is the real one. It takes seconds of file I/O, hence a separate command.

## Rendering has two guards that look removable

- **One pdfium session at a time.** `raster.rs` holds a process-wide lock from
  `load_pdf_from_file` to the last page; `thread_safe` only serialises single
  FFI calls. Concurrent sessions tear document state and report `Encrypted` on
  unencrypted files, because `FPDF_GetLastError()` is process-global.
- **A page over Voyage's 16M-pixel limit is rendered smaller, not refused.**
  `raster::dpi_for_page` clamps DPI only for such pages (posters, A0 sheets);
  since Voyage bills at 2M px anyway, the vector is the one the model would
  have made. Every other page renders byte-identically at 200 DPI. The ceiling
  comes in from `voyage/batch.rs`; `batch::refuse_oversized` remains the floor
  for byte and token ceilings no DPI fixes. Skipping the page instead would
  write a short record and leave it unsearchable.

## Ingest follows a parse, through one queue

- **`pages.markdown` is the parse's write, not the embedder's**
  (`store::upsert_pages`), so `oculus grep` never depends on the vector index.
  Ingest also upserts markdown; both sides never let an empty page overwrite
  stored text.
- **Ingest is idempotent in two halves**: a PDF already embedded in the current
  space is not re-embedded, but its record is still folded into `pages`.
- **`retrieval::IngestError` carries `{message, kind, retryable, latching}`**,
  optional because a failure before any backend has no `EmbedError` behind it.
- **The app's index is a queue with one worker** (`indexStore.ts`): the Index
  button enqueues the backlog, a finished parse enqueues one file, a File
  Activity retry enqueues one. Stop is polled between files, never mid-file (a
  mid-file abort re-pays its pages). After a failure it asks `embed_blocked`
  whether the ledger has latched (spent allowance or spend limit) and stops,
  rather than reporting one fact as a hundred errors.
- **A terminal `parse-status` enqueues that file** (`useBackendEvents.ts`),
  gated on `embedReady` — a stored key and an available engine. The backlog is
  never swept up automatically: it is hours of metered work, and the Settings
  estimate exists to be read first.

## Two indexes over `pages`, never merged

- **Embeddings answer a question**, at one cloud round trip per query, via
  `retrieval::search_in` (a set of subject ids, since CLI prefix codes can
  match a subject in two terms) or its wrapper `search`. The app has no
  semantic-search surface: chat is a CLI agent using `oculus search`
  ([cli.md](./cli.md)); the `search_pages` command is registered but unused.
- **`pages_fts` answers a keystroke.** FTS5 over `pages.markdown` (migration
  35, `retrieval::PAGES_FTS_SQL`), read by `searchPageText` for the ⌘K palette
  and new-tab field ([frontend.md](./frontend.md)) — local, milliseconds, and
  good at a person's exact words, which embeddings are not.

Neither is a fallback for the other. Both stop where parsing does.

`pages_fts` is external content (`content='pages'`), kept in step by triggers
whichever process writes; the update trigger is `UPDATE OF markdown` so an
embed's blob write does not re-index text, and fires only when the markdown
actually changed (migration 38), so an upsert of the same text leaves the
index untouched. A `files` cascade delete fires no trigger, so stale entries can linger — every read joins
`pages ON pages.id = pages_fts.rowid`, so they cost ranking, never
correctness.

## Gotchas

- Don't switch to text embeddings or average image and text vectors — recall on formula/diagram pages halves.
- Never wrap an embed in a timeout — the client paces itself and a 429 is routine.
- Don't make `RateLimited` latching — the free tier becomes unusable instead of slow.
- Don't use one `input_type` for pages and queries — nothing errors, ranking just gets worse.
- `output_encoding: "base64"` (f32 little-endian) is not `output_dtype`, which rejects `base64`.
- Voyage returns vectors already L2-normalised, so the dot product is the cosine — no renormalising.
- Don't drop the `embed_model`/`embed_dim` filter from a scan — mixed spaces rank as noise, silently.
- Don't remove `raster.rs`'s session lock — concurrent pdfium sessions misreport files as encrypted.
- FTS5 is required to open the database at all (migration 35); `fts_tests` asserts the bundled `libsqlite3-sys` still enables it.
- The Voyage key lives only in the keychain, and no `EmbedError` carries response text — an error body can echo the base64 page image.
