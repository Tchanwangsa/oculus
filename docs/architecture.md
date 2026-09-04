# Architecture

Three application layers, one data directory. The Python layer supervises
separate killable model workers rather than retaining their weights itself.

```
┌───────────────────────────── Tauri app ─────────────────────────────┐
│  React frontend (WebView)  ⇄  Rust core (commands + events)         │
│        app/src/                  app/src-tauri/src/                 │
└───────────────┬───────────────────────────▲─────────────────────────┘
                │ HTTP :9547                │ HTTP (ephemeral IPC port)
                ▼                           │
        Python sidecar  ────────────────────┘
        sidecar/main.py   (parse progress callbacks)
          ├─ quality_worker.py ─ MinerU render children
          ├─ embed_worker.py   ─ Qwen model
          ├─ parse_worker.py   ─ one-shot fast parse
          └─ MinerU HTTPS API  ─ opt-in cloud queue
```

## Where

| Piece | Location |
| --- | --- |
| App entry / migrations / startup | `app/src-tauri/src/lib.rs` |
| Data-dir + path resolution (no Tauri handle needed) | `app/src-tauri/src/paths.rs` |
| Sidecar supervisor (spawn, port reclaim, shutdown) | `app/src-tauri/src/sidecar.rs` |
| IPC callback server (sidecar → app) | `app/src-tauri/src/ipc.rs` |
| Media HTTP server (lecture video streaming) | `app/src-tauri/src/media.rs` |
| LLM provider client (keys, streaming, spend limits) | `app/src-tauri/src/llm.rs` |
| Sidecar HTTP service | `sidecar/main.py` |
| Model-worker lifecycle + memory accounting | `sidecar/model_workers.py`, `sidecar/worker_client.py`, `sidecar/memory_governor.py` |
| MinerU token (keychain only) | `app/src-tauri/src/mineru.rs` |
| Frontend DB access | `app/src/lib/db.ts` |
| CLI over the same engine | `app/src-tauri/src/bin/oculus.rs` |

## How the processes talk

- **Frontend ⇄ Rust**: Tauri commands in, Tauri events out. Scrape/parse
  progress arrives as events the frontend folds into zustand stores via
  `app/src/hooks/useBackendEvents.ts`.
- **Rust → sidecar**: plain HTTP on a fixed port, `9547`
  (`SIDECAR_PORT` in `app/src-tauri/src/sidecar.rs`). The supervisor spawns
  `sidecar/main.py` with the project's `.venv` python, reclaims the port from
  orphans first, and installs exit handlers because Ctrl-C and `tauri dev`
  rebuild SIGTERMs bypass Tauri's Exit event.
- **Sidecar → Rust**: the sidecar POSTs parse-status updates to a tiny HTTP
  server in `app/src-tauri/src/ipc.rs`, bound on an ephemeral port passed to
  the sidecar at spawn. This server used to be a much larger surface (cookie
  proxy, WebView host) — the scraper is Rust now, so status callbacks are all
  that is left.
- **Media playback**: WebKit's media pipeline refuses `<video>` sources on
  custom URL schemes — an `asset://` URL fetches fine but the media element
  fails instantly with error code 4 (observed on macOS 26). So lecture video
  streams from a localhost HTTP server in `app/src-tauri/src/media.rs`
  (ephemeral port, per-launch token, Range support, scoped to the data dir's
  `lectures/` and `courses/`). The frontend gets URLs from `mediaSrc()` in
  `app/src/lib/media.ts`. Don't move video back to `convertFileSrc`.

## The data directory

`app/src-tauri/src/paths.rs` computes the same directory Tauri would
(`~/Library/Application Support/com.tchan.oculus` on macOS) **without** an
`AppHandle`, so the CLI and the app can never disagree about where things
live. Inside it:

- `oculus.db` — SQLite, everything structured
- `courses/<code>/…` — scraped files, mirrored to Canvas layout, plus `.md`,
  `.pages.json`, and `<stem>_images/` siblings the parser writes
- `lectures/<uuid>/` — downloaded Echo360 media
- `mineru-usage.json` — persistent daily cloud reservations and quota latch
- the session cookie and auth-flag files (see [auth.md](./auth.md))

## The database

Schema lives in the tauri-plugin-sql migrations in `app/src-tauri/src/lib.rs`
(19 versions and counting). Ownership is split deliberately:

- **In the app**, the *frontend* writes the scrape tables: it listens for
  scrape events and upserts through `app/src/lib/db.ts`.
- **Headless (CLI)**, `app/src-tauri/src/store.rs` writes the same rows with
  the same SQL, so a CLI sync shows up in the app as if the app had done it.
  It never creates the database — schema stays with the plugin's migrations,
  which is why a fresh machine must open the app once before the CLI works.

The `pages` table (markdown + embedding blob per PDF page) is the retrieval
substrate — see [retrieval.md](./retrieval.md). `calendar_events` is the one
table a sync *replaces* rather than upserts into, so a cancelled class can
disappear — see [calendar.md](./calendar.md).

## How it connects

- The sidecar's 8 GB default memory budget is shared by its **whole tree**,
  not allocated once per child. Model workers use JSON lines and their own
  process groups; killing a model also kills its render descendants without
  dropping HTTP or the queue. See [sidecar.md](./sidecar.md) for admission,
  retry and the 5 GB tunable floor.
- Parse settings are in SQLite under `parse`; Rust loads them at spawn and
  the frontend updates `/limits` live through Rust. MinerU's token follows a
  separate path: Rust keychain → loopback parse body → cloud client. It never
  enters SQLite, health, or progress events. Cloud is off by default.
- The startup sequence in `app/src-tauri/src/lib.rs` is: start IPC server →
  spawn sidecar → clean partial lecture downloads → verify the persisted
  session in a background thread (optimistic until proven rejected) → start
  the in-app keep-alive loop.
- Everything Canvas-shaped was **moved out of hidden WebViews on purpose**:
  macOS suspends off-screen WKWebView content processes, which froze the old
  `scraper.js` mid-run with nothing to catch. The scrape engine is Rust
  (`app/src-tauri/src/sync.rs`); do not move background work back into a
  WebView.
- The sidecar is optional at runtime: with no `.venv` (or the port opted
  out), scraping still completes — PDFs are simply not parsed or embedded
  until `oculus index` or the app's sweep picks them up.
