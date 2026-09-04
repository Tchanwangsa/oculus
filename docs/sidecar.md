# Sidecar — PDF parsing & embedding service

A Python FastAPI process (uvicorn, port `9547`) that turns PDFs into per-page
markdown and page-image embeddings. Spawned and supervised by
`app/src-tauri/src/sidecar.rs`; also runnable by hand for debugging.

## Where

| Piece | Location |
| --- | --- |
| HTTP service, global heavy-work queue, progress | `sidecar/main.py` |
| Fast parse + parse-mode logic | `sidecar/parser.py` |
| One-shot fast-parse subprocess | `sidecar/parse_worker.py` |
| Quality renderer + compatibility facade | `sidecar/mineru_render.py`, `sidecar/mineru_parser.py` |
| Local quality backend + retry ladder | `sidecar/mineru_local.py`, `sidecar/quality_client.py` |
| Formula-batch memory cap | `sidecar/mfr_memory.py` |
| Model workers + JSON-lines transport | `sidecar/quality_worker.py`, `sidecar/embed_worker.py`, `sidecar/worker_client.py`, `sidecar/model_workers.py` |
| Whole-tree memory governor | `sidecar/memory_governor.py` |
| Cloud client, limits + quota ledger | `sidecar/mineru_cloud.py` |
| Cloud batching + backend routing | `sidecar/cloud_batcher.py`, `sidecar/quality_router.py` |
| Live parse settings | `sidecar/parse_settings.py` |
| Page-image embeddings (Qwen3-VL) | `sidecar/embedder.py`, `sidecar/embedding_client.py`, `sidecar/embed_contract.py` |
| Model-init lock | `sidecar/modellock.py` |
| Deps (uv) | `sidecar/pyproject.toml`, `sidecar/uv.lock` |
| Rust supervisor + live limits | `app/src-tauri/src/sidecar.rs` |
| MinerU keychain commands | `app/src-tauri/src/mineru.rs` |
| Settings UI | `app/src/pages/settings/LibraryPage.tsx` |

Endpoints: `POST /parse-pdf`, `POST /embed-pdf`, `POST /embed-query`,
`GET /parse-progress`, `/parse-status`, `/parse-status-batch`, `/embed-info`,
`/health`, `POST /limits` (`memory_cap_mb`, `backend`), and
`POST /mineru-token-reset`.

## How it connects

The HTTP parent never imports torch or owns model weights. Each model lives in
its own lazy, long-lived child, reached over JSON lines. Killing a worker does
not take down HTTP, queued files, or progress reporting. Workers have their
own process groups so MinerU's render descendants die with their owner; idle
workers expire after 15 minutes. The Rust supervisor still owns final shutdown
of the complete sidecar tree.

Rust reads the `settings` key `parse` at spawn and passes the cap/backend as
environment variables. Frontend changes persist the same record and call
`sidecar_set_limits` → `/limits`, without restarting in-flight work. Backend
changes apply to new requests; lowering the memory cap applies to active work
too and may terminate a worker. The CLI's `--memory-cap` is a live override,
not a persisted preference.

## Two-tier parsing

- **Fast** (`parse_fast`, pymupdf4llm): ~2s per deck. Returns as soon as
  markdown exists, so a scrape is never blocked on quality. Runs in a
  throwaway subprocess (`parse_fast_isolated` → `sidecar/parse_worker.py`)
  because the parse leaks; see the pins below.
- **Local quality** (`parse_quality`, MinerU pipeline backend): ~1s/page roughly
  regardless of formula density. Runs in the background after fast returns;
  waiting files are surfaced with a queue position so they don't look stuck.
- A **single sidecar-wide heavy-work slot** serializes fast parsing, MinerU
  quality parsing, page-image embedding, and the embedding model's first load.
  Queries use the same gate: a warm embedding worker can be evicted between
  requests, so even a search may require a model reload. Cloud parses never
  hold this slot and can run alongside local work.
- Formula decoding history: pix2tex → docling enrichment → **MinerU**
  (2026-08-15). MinerU is ~100× faster than docling-with-enrichment and more
  correct (1% vs 18% KaTeX render failures on the benchmark deck). Docling
  is fully removed — no fallback path.

### Outputs, per PDF (written beside it)

- `<stem>.md` — full-document markdown
- `<stem>.pages.json` — per-page markdown keyed by 1-based `page_no`; **this
  is the join key retrieval rests on**, and it is only written when a parse
  *finishes* (an interrupted parse must not read as done)
- `<stem>_images/` — extracted images; the dir name is also the link prefix
  in the markdown

`parse_mode` in `sidecar/parser.py` decides what still needs doing
(`quality` / `fast` / `none`) from `.pages.json` + `PARSER_VERSION` — never
from the images dir. Old-parser markdown reports `none`→redo; quality output
is never invalidated (re-running MinerU across a library costs hours).

Both quality backends write `mode: "quality"` and an audit-only `backend`
(`mineru-local` or `mineru-cloud`). `PARSER_VERSION` remains 2. Images are
staged while quality runs, leaving fast artifacts intact if inference fails.
The shared renderer retains the old 64-page boilerplate grouping independently
of inference task size. Identical content lists/crops render identically;
cloud model versions can still produce different extraction results.

## Memory budget

The default is **8192 MiB for the whole sidecar process tree**, with a tunable
floor of **5120 MiB** (shown as 8 GB / 5 GB in settings). Five GB is an allowed
budget, not a promise MinerU will fit; many local quality jobs will be rejected
at that setting. Model weights are reclaimable worker state, not a permanently
shared 7.5 GB baseline.

- Admission compares the current tree with a calibrated branch estimate,
  evicts idle model owners only if needed, and waits for active work. An
  operation whose irreducible estimate cannot fit fails without starting.
- The watchdog samples about once a second. On macOS it sums
  `proc_pid_rusage(..., RUSAGE_INFO_V2).ri_phys_footprint`, including render
  children; other platforms fall back to RSS. This is a sampled protection,
  not an OS-enforced allocation ceiling: brief overshoot remains possible.
  If the process tree cannot be enumerated, local admission refuses to start
  and health reports incomplete measurement rather than a misleading total.
- At 85%, dead objects/accelerator caches are already reclaimed at local
  chunk boundaries, and the governor evicts idle Qwen. At the hard cap it
  kills the active model owner (or the one-shot fast parser), preserving the
  parent service. Local quality retries 64-page chunks/window 8/formula
  batch 2, then 16/window 4/formula batch 1. Environment overrides can only
  lower those sizes. The formula batch is the first rung to move because it,
  not page residency, is what drives MinerU's peaks.
- After both local attempts fail, cloud is eligible only with explicit
  cloud/auto opt-in. Otherwise the existing fast markdown stays usable.
  `/health.memory` exposes current footprint, cap, peak, kills and last kill.

Virtual-address limits (`RLIMIT_AS`) do not describe torch/Metal's physical
cost; Darwin's data limit does not cover mmap. Worker termination is the
reliable reclaim mechanism here.

## MinerU cloud (opt-in)

`local` is the default and never uploads. `cloud` and `auto` prefer cloud when
a token/quota is available; transient cloud failures fall back locally and
open a short failure cooldown. Exhausting a local per-minute bucket waits,
including server HTTP 429 responses, rather than falling back.

Rust alone accesses the MinerU keychain entry and injects its token into the
loopback `/parse-pdf` body. It is never persisted in SQLite or progress/log
payloads. Settings names the privacy boundary: PDFs go to MinerU and its
PRC-hosted OSS storage. Server caching is not a deletion guarantee.

### Token state

A token is checked before it is stored: `mineru_set_api_key` GETs a
non-existent task id, which costs nothing and creates nothing, and treats
HTTP 401/403 as MinerU refusing the token (`A0202` invalid, `A0211` expired).
Anything else — including the expected "task not found" — means the token
passed the gateway. An unreachable MinerU stores the token and reports
`unverified` rather than blocking the user offline.

Expiry is never polled. A mid-parse 401 raises `CloudAuthError`, which latches
cloud off in `sidecar/quality_router.py` — every remaining file would hit the
same 401 — and the file is still parsed locally. `/health.parse
.cloud_token_rejected` carries the latch to the settings page, which already
polls health. Saving or removing a token calls `POST /mineru-token-reset` to
clear it; a sidecar restart clears it too.

The client uses batch signed uploads even for one PDF, then polls and downloads
the result ZIP's content list and crops into the shared renderer. Parameters
match local pipeline inference (`ch`, formulas/tables on, forced OCR off).
The cloud queue accumulates for 5 seconds or 20 files and allows 8 batches
in flight, separately from the local FIFO. Split-task progress is summed,
not inferred from page offsets; cloud has no fake heartbeat.

All budgets live in `LIMITS` in `sidecar/mineru_cloud.py`. The
[live API docs](https://mineru.net/apiManage/docs), checked 2026-09-03, specify
200 MB / 200 pages, 50 signed-upload entries per request, and 1000
highest-priority pages/day (then slower service, not refusal). Larger PDFs
use page-range tasks; byte-oversized PDFs are physically sliced first.
The current public page does not confirm account submission/file-day quotas,
so Oculus conservatively keeps 50 submissions/min, 1000 polls/min and
5000 files/day as application budgets rather than claiming larger limits.

The data directory's `mineru-usage.json` atomically reserves files/pages before
submission, surviving restart and concurrent batches. Uncertain failures
conservatively count. Server daily-quota error `-60018` latches until reset.
The ledger assumes Beijing midnight; the provider's reset timezone remains
unconfirmed. The HTML counter is retained but this path submits PDFs only.

Regression tests cover balloon kill/restart, queue recovery, routing privacy,
signed PUT headers, ZIP extraction, split-page attribution and progress, and
fast-output preservation. `sidecar/benchmark_quality.py` runs a real deck on a
temporary copy without touching library artifacts.

Real regression on 2026-09-03: the 191-page `computing_abstractions_slides`
deck completed fast + local quality in 106.1 seconds on Metal, with a sampled
whole-tree physical-footprint peak of 5820 MiB at the 8192 MiB cap, zero kills,
191 page records and 19 retained images. (Before the formula-batch cap the same
deck took 132.5 seconds and peaked at 6101 MiB; formula-dense PDFs did not
complete at all.) Two files that previously exhausted both retry rungs now pass
at the same cap: a 6-page solutions PDF at 5447 MiB in 61.5s, and a 50-page
lecture deck at 5539 MiB in 85.7s. A separate sandbox/CPU run is not
comparable (Metal and descendant enumeration were unavailable there).
A real Qwen query in its separate Metal worker produced a 512-dimensional
float16 vector with 5335 MiB whole-tree footprint; embedding admission budgets
5600 MiB for that branch. The 5 GB floor therefore does not guarantee local
indexing either. Both measurements used cached weights and no cloud upload.

## Lifecycle

- `app/src-tauri/src/sidecar.rs` prefers `sidecar/.venv/bin/python3` (created
  by `uv sync`); with no venv it disables parsing and says so. It reclaims
  port 9547 from orphans before spawning, and installs its own exit handlers
  because Ctrl-C / `tauri dev` rebuilds bypass Tauri's Exit event.
- Progress flows back by the sidecar POSTing to the app's IPC port
  (`_notify_tauri` in `sidecar/main.py` → `app/src-tauri/src/ipc.rs`).
- The Qwen embedding model is loaded on demand, not at sidecar startup, so a
  quality parse that reaches the heavy-work queue first gets maximum headroom.
- Oculus receives real progress once per 64-page outer parse chunk, so between
  chunks the UI percentage is *estimated* from a seconds-per-page EMA. Inside
  each call, MinerU processes no more than 8 rendered pages at a time.

## Constraints that look like bugs (they are pins)

All in `sidecar/pyproject.toml` / `sidecar/mineru_local.py`:

- **PyMuPDF pinned `<1.28`** — 1.28 adds an OCR fallback that turns a 2s
  fast parse into 24s on image-heavy slides.
- **torch ≥ 2.13 required** — on 2.12, UniMERNet emits out-of-range token
  ids on MPS and the tokenizer raises `OverflowError`.
- **`six` declared explicitly** — MinerU's vendored pytorchocr bare-imports
  it; it used to arrive via docling's dep chain.
- **`qwen-vl-utils` is not droppable** — the model's own embedding script
  imports `process_vision_info` from it (and it hard-requires PyAV, ~47MB).
- **`mineru[pipeline]`, not `[core]`** — core drags in gradio, which pins
  starlette down.
- **MinerU's internal processing window is capped at 8 pages.** Its former
  64-page default let high-resolution lecture slides retain enough rendered
  page images to exhaust system memory. `OCULUS_MINERU_WINDOW_PAGES` can lower
  the cap for smaller machines; it and MinerU's own
  `MINERU_PROCESSING_WINDOW_SIZE` are clamped at 8. Oculus keeps a separate
  outer chunk capped at 64 pages for real progress reporting, avoiding the
  large fixed cost of restarting `do_parse` every 8 pages. The existing
  `OCULUS_MINERU_CHUNK_PAGES` override can lower that outer size. MinerU closes
  rendered images between its internal windows; Oculus releases dead objects
  and accelerator caches between outer chunks.
- **MinerU's formula batches are capped at 2 crops** (`sidecar/mfr_memory.py`,
  lowerable via `OCULUS_MINERU_MFR_BATCH`). UniMERNet's `UnimerSwinModel`
  encoder has no SDPA kernel, so MinerU must load it with eager attention;
  decoding runs to `max_new_tokens` and every crop in a batch waits for the
  longest one, so transient memory grows with batch size times the longest
  formula present. A batch of short formulas costs ~1.2 GiB; one long
  derivation in a 16-crop batch cost **+31.7 GiB** in a single step.
  MinerU cannot shrink these itself — `get_mfr_min_dynamic_batch_size` floors
  the batch at 16, exactly what Metal/low-VRAM hosts request, so its own
  adaptive path never runs, and `finalize_mfr_batch_groups` then merges the
  trailing group (the largest crops, hence the longest formulas) into its
  predecessor, planning batches of 23 where the requested size was 16. The cap
  wraps that one planning function so no downstream merge can undo it.
  Measured 2026-09-03 at window 8: a 6-page formula-dense solutions PDF went
  32.6 GiB → 5.6 GiB, a 50-page lecture deck 27.2 GiB → 5.8 GiB. Neither got
  slower and formula output is byte-identical. A cap of 4 still measured
  30.0 GiB, so 2 is a ceiling, not a starting point.
- Quality pages are built from MinerU's `content_list.json` (has `page_idx`),
  **never** the flat `.md` — that file has no page boundaries and drops
  `header` items, which on slides are the titles. Footnote-typed items
  (`chart_footnote`) carry the figure-explaining prose.
- **The fast parse leaks ~2 GB per deck and must stay in a subprocess.**
  `pymupdf4llm.to_markdown` retains C-level allocations that nothing
  in-process reclaims — a gc sweep finds no live `Document`s and no objects
  accounting for the size, and `TOOLS.store_shrink(100)` changes nothing. It
  is the text/layout analysis, not image extraction (`write_images=False`
  costs the same). Measured over six decks in one interpreter: 2.1 GB → 5.8 GB
  and no decline. Across a 105-PDF library that curve is what exhausted a
  36 GB machine mid-sync. Process exit is the only reliable reclaim, so each
  fast parse gets its own interpreter — the same six decks leave the parent
  flat at ~115 MB, for about 1s of interpreter start per deck.
  `OCULUS_INPROCESS_PARSE=1` restores the old in-process call for profiling;
  it still leaks.
- **The quality tier does not show cumulative leakage.** Measured
  2026-08-26, 49 sequential parses across three fresh interpreters: RSS ramps
  to a plateau of ~3.0–3.4 GB and then oscillates, with repeated *negative*
  deltas. The decisive test is the same one that convicted the fast tier —
  one identical deck 20 times — and here the post-warm slope is −2.8 MB/doc.
  The model cache is keyed on `(name, lang, thresholds, device)` and
  `_run_mineru` always passes the same key, so it cannot grow per document,
  and the spawn-mode render pool is a reused singleton (exactly two
  descendants at every iteration, no orphans). Isolating this tier per file
  would reload weights every time and buy nothing.
  Two things to carry into any memory budget: the plateau is **resident for
  the worker's life** (~3.3–3.7 GB including the render child), and
  **`ps` RSS understates it** — Metal
  and IOAccelerator regions do not fully land in RSS, and sampled physical
  footprint peaked at ~6.8 GB while `ps` read ~3.2 GB. Budget against the
  transient, not the RSS. A later 191-page, high-resolution slide deck exposed
  a separate peak-memory failure in MinerU's 64-page processing window; the
  8-page hard ceiling addresses that peak independently of the leak finding.
- Keep module-scope side effects out of `sidecar/main.py`: MinerU renders in
  a spawn-mode ProcessPoolExecutor that re-imports the entry module, so a
  module-level model warm-up would load a 4GB model per worker.

## Debugging

**Stdout silence is not a hang.** `main.py` uses bare `print()` and Python
block-buffers stdout on a pipe (the Tauri spawn sets `PYTHONUNBUFFERED=1`,
manual runs may not). `GET /parse-status` is authoritative. To run the
sidecar yourself, keep Oculus off the port via the env override documented at
the top of `app/src-tauri/src/sidecar.rs`.
