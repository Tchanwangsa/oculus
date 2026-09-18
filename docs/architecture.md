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
| In-app browser (one page WebView per tab, in the main window) | `app/src-tauri/src/browser.rs` |
| CLI-agent harness (Claude Code / Codex bridges) | `app/src-tauri/src/harness/mod.rs` |
| Projects and tasks, written headlessly | `app/src-tauri/src/projects.rs` |
| Lecture chapters: boundary detection and the naming job | `app/src-tauri/src/chapters.rs` |
| LLM provider client (keys, streaming, spend limits; dormant) | `app/src-tauri/src/llm.rs` |
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
  `.pages.json`, and `<stem>_images/` siblings the parser writes. One
  subdirectory is not the scraper's: `courses/<code>/uploads/` holds the
  student's own files, copied in by hand (`import_uploads` in
  `app/src-tauri/src/files.rs`). They are ordinary library files from there on
  — same conversion, parse, embeddings and agent access — and being the one
  place a sync never writes is what makes them the one place deleting is safe
- `lectures/<uuid>/` — downloaded Echo360 media: `source1.mp4` (the Presenter
  screen), `source2.mp4` (the room camera, when the capture has one and it has
  been asked for) and `transcript.vtt`. A `frames/` subfolder appears only when
  `oculus lecture candidates --frames` or `oculus lecture chapters` is run over
  it — a JPEG per detected topic boundary, regenerable in seconds and nothing's
  source of truth (see [chapters.md](./chapters.md))
- `agents/` — the docs a coding agent reads, the `AGENTS.md` every course
  folder symlinks, and the memory layer it writes back (`TASTE.md`,
  `memories/`); written by `oculus docs` and, for everything but the CLI
  reference, by every sync (see [cli.md](./cli.md)). Also the working
  directory — and only writable root — of every chat thread, and
  `agents/threads/<id>.ndjson`, the raw provider output per thread (see
  [harness.md](./harness.md))
- `mineru-usage.json` — persistent daily cloud reservations and quota latch
- the session cookie and auth-flag files (see [auth.md](./auth.md))

## The database

Schema lives in the tauri-plugin-sql migrations in `app/src-tauri/src/lib.rs`
— append-only and numbered, so the highest `version` in that list is the
current schema. Ownership is split deliberately:

- **In the app**, the *frontend* writes the scrape tables: it listens for
  scrape events and upserts through `app/src/lib/db.ts`.
- **Headless (CLI)**, `app/src-tauri/src/store.rs` writes the same rows with
  the same SQL, so a CLI sync shows up in the app as if the app had done it.
  It never creates the database — schema stays with the plugin's migrations,
  which is why a fresh machine must open the app once before the CLI works.

The `pages` table (markdown + embedding blob per PDF page) is the retrieval
substrate — see [retrieval.md](./retrieval.md). `lecture_chapters` (migration
29) is the other derived table: a recording's named topic spans, written by
the agent job in [chapters.md](./chapters.md) and regenerable from the file on
disk, with the job's own status on `lectures` beside it. `calendar_events` is the one
table a sync *replaces* rather than upserts into, so a cancelled class can
disappear — see [calendar.md](./calendar.md).

Not everything in here is scraped. `harness_threads` / `harness_items` are the
chat timeline ([harness.md](./harness.md)), and `projects` / `project_tasks`
(migration 27) are the student's own planning — boards, tasks and one level of
subtask ([projects.md](./projects.md)). Those two, plus `local_events`, hold
rows nothing upstream has a copy of, which is why each of them scopes a subject
with a **nullable** `subject_id` that clears rather than cascades: dropping a
course must not take the user's own work with it. They have the same two
writers as the scrape tables — `app/src/lib/projects.ts` in the app,
`app/src-tauri/src/projects.rs` headless.

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
  spawn sidecar → clean partial lecture downloads → seed WebKit's cookie jar
  with the Canvas session → verify the persisted session in a background
  thread (optimistic until proven rejected) → start the in-app keep-alive
  loop.
- **The main window holds more than one WebView.** External links open in
  in-app browser tabs (`app/src-tauri/src/browser.rs`, needs tauri's
  `unstable` feature for `Window::add_child`): one webview of remote content
  per tab, a child of the main window stacked above the app's own webview,
  shown over the slot the `/browse/:id` route leaves in the content card.
  The frontend reports that slot as insets from the window edges; Rust lays
  pages out from those and the window size, so a resize never waits on
  JavaScript. That makes the scope of
  `app/src-tauri/capabilities/default.json` load-bearing: it names
  `webviews` (`main`), not `windows`, because the two match by **OR** — a
  window-scoped capability would hand every Tauri command to whatever page
  the user browsed to. See [frontend.md](./frontend.md) for the tab strip
  and slot, and [auth.md](./auth.md) for why those pages are signed in.
- Everything Canvas-shaped was **moved out of hidden WebViews on purpose**:
  macOS suspends off-screen WKWebView content processes, which froze the old
  `scraper.js` mid-run with nothing to catch. The scrape engine is Rust
  (`app/src-tauri/src/sync.rs`); do not move background work back into a
  WebView.
- The sidecar is optional at runtime: with no `.venv` (or the port opted
  out), scraping still completes — PDFs are simply not parsed or embedded
  until `oculus index` or the app's sweep picks them up.
