# Retrieval

Semantic search over the library, built on **page images**, not extracted
text. There is no graph and no vector index — both were considered and
dropped, on measurement.

## Where

| Piece | Location |
| --- | --- |
| Ingest + brute-force cosine search | `app/src-tauri/src/retrieval.rs` |
| Embedder (model, dims, token budget) | `sidecar/embedder.py` |
| Killable embedding worker + parent facade | `sidecar/embed_worker.py`, `sidecar/embedding_client.py`, `sidecar/embed_contract.py` |
| Per-page markdown source | `app/src-tauri/src/parse/mod.rs` (`.pages.json`) |
| Who writes `pages.markdown` | `app/src-tauri/src/sync.rs` (the parse path) |
| `pages` table schema | migrations in `app/src-tauri/src/lib.rs` |
| Frontend query path | `app/src/lib/retrieval.ts`, `app/src/pages/ChatPage.tsx` |
| Terminal query path | `app/src-tauri/src/bin/oculus.rs` (`oculus search`) |
| Smoke test | `app/src-tauri/src/bin/retrieval_smoke.rs` |

## The flow

Sidecar renders each PDF page to PNG and embeds it → the vector lands in the
`pages` table alongside that page's markdown, keyed on `(file, page_no)` → a
query is embedded by the same model (`POST /embed-query`) → Rust ranks by dot
product over every stored vector → hits carry the markdown for the answer and
the `(file, page)` ref for the deep link. **Nothing downstream of ranking
touches a vector** — a future LLM sees only markdown and citations.

## Decisions and the numbers behind them

Benchmarked 2026-08-15 on real course decks (152-page corpus, then re-run at
908 pages with topically adjacent distractors):

- **Model: `Qwen3-VL-Embedding-2B`** — the *VL* line, not the text-only
  `Qwen3-Embedding` family. The 2B is within one benchmark query of the 8B
  at a quarter of the download.
- **Image embeddings, alone.** On ordinary questions image ties text; on
  formula/screenshot/diagram pages (where text extraction yields garbage
  like `56 = 7(( 7() 7)(`) image roughly doubles recall and text never
  recovers even at rank 3. Averaging image and text vectors scored *worse*
  than image alone — don't hybridize at the vector level.
- **512 dims via Matryoshka truncation** (native 2048): measured
  indistinguishable, a quarter the storage. Slice then re-normalise — a
  truncated unit vector is no longer unit length.
- **640 vision tokens** is the knee: 2.7× faster than the model default at
  equal-or-better accuracy (~0.45 s/page real-world on MPS). Batching is a
  no-op on MPS; token count is the only speed lever.
- **Brute-force scan, no index.** A degree of coursework is a few thousand
  pages; at 512 dims that is single-digit MB and milliseconds. Scale was
  checked: a 6.5× bigger adjacent-topic library cost one query of recall.
- The shipped `Qwen3VLEmbedder` hardcodes cuda-else-cpu; `sidecar/embedder.py`
  subclasses it onto MPS.

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
- Ingest is idempotent and decoupled from scraping: `run -s` embeds what it
  parsed; `oculus index` re-embeds files already on record.
- Two callers rank against the same store: the app's chat page and
  `oculus search`. The ⌘K palette is **not** a third one — it matches titles
  in SQLite so it can answer every keystroke; see
  [frontend.md](./frontend.md). `search_in` takes a set of subject ids because the CLI
  accepts prefix codes, which can match the same subject in two terms;
  `search` is the single-subject wrapper the Tauri command uses. Both embed
  the query once and the subject filter is SQL, so neither pays per course.
  See [cli.md](./cli.md).
- Page-image and query embedding share the sidecar's single heavy-work slot
  with local parsing. The model lives in a separate lazy worker; admission
  can unload idle MinerU to make room and the governor can kill Qwen without
  dropping the service. A query may wait/reload after memory reclamation.
  Cloud quality uses a separate queue and does not delay embedding admission.
- Embedding and parsing meet **only** on `(file, page_no)` via
  `.pages.json`. If page attribution breaks in the parser, retrieval
  silently returns the wrong markdown for a correct visual hit.

## How much memory a local model can actually have

The embedder is not the only thing that wants the GPU. The deleted BYOK layer
ran a preflight before every local model call, because a local model that does
not fit is not slow — it is an OOM that takes the machine down, observed with
a 17 GB Ollama model loading beside the sidecar's Qwen3-VL embedder on a 36 GB
machine. Nothing does that check today; chat is a CLI agent and the sidecar
governs its own two workers. The method is written down here because it was
measured rather than reasoned, and re-adding a preflight when a local provider
returns should be an afternoon rather than a rediscovery.

- **Available memory is physical memory (`sysctl hw.memsize`) less wired
  pages, not free pages.** macOS keeps almost nothing free, compressing
  anonymous memory and evicting file cache on demand: measured on the 36 GB
  dev machine while it was happily serving a 17.4 GB resident model, free +
  inactive + speculative + purgeable came to 2.9 GB — a figure that refuses
  every model there is. Wired pages are the ones the kernel cannot page out,
  and on Apple silicon that is exactly where GPU-resident weights (and the
  sidecar's MPS embedder) live, so they are the ones worth counting.
  Everything else is compressible or swappable.
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
