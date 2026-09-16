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
| Page rasterizer (pdfium) | `app/src-tauri/src/embed/raster.rs` |
| Voyage client — the `Embedder` | `app/src-tauri/src/embed/voyage/client.rs` |
| Allowance, throttle, tier detection | `app/src-tauri/src/embed/voyage/ledger.rs` |
| Which pages travel in one request | `app/src-tauri/src/embed/voyage/batch.rs` |
| Backend selection + throwing the index away | `app/src-tauri/src/embed/commands.rs` |
| API key (keychain only) | `app/src-tauri/src/voyage.rs` |
| Ingest + brute-force cosine search | `app/src-tauri/src/retrieval.rs` |
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
- The library is ~5.96B px, **4.0% of the 150B free pixel allowance** — the
  pixel budget is not the constraint, the per-minute ceiling is.
- **`RENDER_DPI` stays 200** (`app/src-tauri/src/embed/raster.rs`) even though
  ~144 DPI is the number that lands exactly on the 2M-pixel cap. Anything
  above it is downscaled by Voyage before it is looked at, so extra DPI costs
  upload bandwidth only — not tokens, not quality. It is kept because it is the
  number every page artifact in the library was rendered at.
- `voyage-usage.json` in the data dir holds the reservations, the latched
  quota and the learned tier, atomically and across restarts — the same
  discipline as `mineru-usage.json`.

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
re-index is a re-run of `oculus index`, which is only true because
`is_embedded` already rejects the old records.

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
  — used by `oculus index` for its in-place line. The app has no listener for
  it: there is no embed stage in the pipeline table and nothing in
  `app/src/hooks/useBackendEvents.ts` subscribes to an embed event.
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

- `embed_settings` / `embed_set_engine` exist in Rust with no Settings →
  Library page consuming them yet (plan A3f).
- `embedFile`, `embedPending`, `searchPages` and `embeddingStats` in
  `app/src/lib/retrieval.ts` have no caller in the app — chat is a CLI agent
  now, and it reaches the library through `oculus search` and `oculus grep`
  rather than through the WebView. They are the tested path the CLI uses,
  reached from TypeScript, not dead code — but nothing in the UI invokes them.
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
