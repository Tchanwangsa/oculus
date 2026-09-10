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
| Per-page markdown source | `sidecar/parser.py` (`.pages.json`) |
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
- Ingest is idempotent and decoupled from scraping: `run -s` embeds what it
  parsed; `oculus index` re-parses/re-embeds files already on record —
  which is also how quality markdown (finished after a scrape returned)
  reaches the database.
- Two callers rank against the same store: the app's chat page and
  `oculus search`. `search_in` takes a set of subject ids because the CLI
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
