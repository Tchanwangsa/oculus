# Architecture

**Two processes, one data directory.** PDF parsing and page embedding both run
in Rust, behind the seams in `app/src-tauri/src/parse/` and
`app/src-tauri/src/embed/`, reaching two clouds over HTTPS — or, for parsing, a
MinerU the user runs themselves. The Python sidecar that used to own both is
gone from the app entirely.

```
┌───────────────────────────── Tauri app ─────────────────────────────┐
│  React frontend (WebView)  ⇄  Rust core (commands + events)         │
│        app/src/                  app/src-tauri/src/                 │
│                                     │                               │
│                                  parse/  ── MinerU cloud, or a      │
│                                            MinerU on loopback       │
│                             (in-process, emits parse-status)        │
│                                     │                               │
│                                  embed/  ── Voyage HTTPS API        │
│                        (in-process; pdfium rasterises the page)     │
└─────────────────────────────────────────────────────────────────────┘
```

**Oculus starts no child process on either path**, and that is the invariant
worth holding onto. It is not the same as "no loopback": the parse seam has a
second engine, and with it selected a parse is an HTTP call to
`127.0.0.1:8000` — a MinerU server the *user* installed and runs, which Oculus
neither launches, supervises nor ships. Same boundary as MinerU cloud, with a
different hostname. Nothing calls back *into* the app in either case: the
sidecar used to POST parse progress to a loopback server of ours on an
ephemeral port, and both ends of that are deleted.

## Where

| Piece | Location |
| --- | --- |
| App entry / migrations / startup | `app/src-tauri/src/lib.rs` |
| Data-dir + path resolution (no Tauri handle needed) | `app/src-tauri/src/paths.rs` |
| PDF parse seam (trait, artifacts, errors, config) | `app/src-tauri/src/parse/mod.rs` |
| MinerU cloud client (in-process, batched) | `app/src-tauri/src/parse/mineru/client.rs` |
| MinerU-on-this-Mac client (in-process, one POST over loopback) | `app/src-tauri/src/parse/mineru/local.rs` |
| `parse-status` events | `app/src-tauri/src/parse/events.rs` |
| Page embed seam (trait, artifacts, errors, config) | `app/src-tauri/src/embed/mod.rs` |
| Voyage cloud client + page rasterizer | `app/src-tauri/src/embed/voyage/client.rs`, `app/src-tauri/src/embed/raster.rs` |
| Ingest + brute-force search over `pages` | `app/src-tauri/src/retrieval.rs` |
| Media HTTP server (lecture video streaming) | `app/src-tauri/src/media.rs` |
| In-app browser (one page WebView per tab, in the main window) | `app/src-tauri/src/browser.rs` |
| CLI-agent harness (Claude Code / Codex / opencode bridges) | `app/src-tauri/src/harness/mod.rs` |
| Projects and tasks, written headlessly | `app/src-tauri/src/projects.rs` |
| Lecture chapters: boundary detection and the naming job | `app/src-tauri/src/chapters.rs` |
| MinerU token (keychain only) | `app/src-tauri/src/mineru.rs` |
| Voyage API key (keychain only) | `app/src-tauri/src/voyage.rs` |
| Frontend DB access | `app/src/lib/db.ts` |
| CLI over the same engine | `app/src-tauri/src/bin/oculus.rs` |

## How the processes talk

- **Frontend ⇄ Rust**: Tauri commands in, Tauri events out. Scrape/parse
  progress arrives as events the frontend folds into zustand stores via
  `app/src/hooks/useBackendEvents.ts`.
- **Rust → the two clouds**: MinerU for parsing and Voyage for page
  embeddings, both over plain HTTPS from inside this process, both with their
  credential read from the macOS keychain and handed straight to the client.
  Neither key enters SQLite, the WebView, a health response or a progress
  event, and neither crosses a socket on this machine. Parsing has a second
  destination — MinerU's own server on loopback — which needs no credential at
  all, so a local parse touches the keychain not at all.
- **Anything → Rust**: nothing listens. There used to be a tiny HTTP server of
  ours on an ephemeral port, first as a cookie proxy and WebView host for the
  JS scraper, then — once the scraper became Rust — for parse-status callbacks
  alone. Parsing is in-process now, so `app/src-tauri/src/parse/events.rs`
  emits `parse-status` straight to the frontend and the loopback server is
  deleted. The handle it emits through is bound once at startup rather than
  threaded through the call path, which is also what lets the CLI run the same
  parse code with nothing to emit to.
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
  `.pages.json`, and `<stem>_images/` siblings the parser writes, and the
  `.emb.json` sibling the embedder writes beside them
- `lectures/<uuid>/` — downloaded Echo360 media: `source1.mp4` and
  `source2.mp4` (the second when the capture has one and it has been asked
  for), plus `transcript.vtt`. Usually source 1 is the Presenter screen and
  source 2 the room camera, but not always — which one a job reads is measured
  rather than assumed (see [chapters.md](./chapters.md)). A `frames/`
  subfolder and an `outline.md` appear only when `oculus lecture
  candidates --frames` or `oculus lecture chapters` is run over it — a JPEG per
  detected topic boundary and the transcript merged with the slide changes,
  both regenerable in seconds and neither anything's source of truth. Each job
  owns its own folder of grabs (`frames/`, `frames/reading/`, `frames/live/`) and
  sweeps the ones a re-run will not overwrite
- `agents/` — the docs a coding agent reads, the `AGENTS.md` every course
  folder symlinks, and the memory layer it writes back (`TASTE.md`,
  `memories/` across subjects and `memories/<CODE>/` for one, which each
  course folder's `agents/memories` is a symlink to — both buckets are in here
  because this is the only folder a thread may write); written by
  `oculus docs` and, for everything but the CLI
  reference, by every sync (see [cli.md](./cli.md)). Also the working
  directory — and only writable root — of every chat thread, and
  `agents/threads/<id>.ndjson`, the raw provider output per thread (see
  [harness.md](./harness.md))
- `mineru-usage.json` — persistent daily cloud reservations and quota latch
- `voyage-usage.json` — the same, for embeddings: pixel and token
  reservations, the quota latch, the rate-limit tier the client learned, and
  the spend guard Settings → Library sets (a percentage of Voyage's free pixel
  grant). The guard lives here rather than in the `settings` table because the
  reservation that enforces it already reads this file on every request; see
  [retrieval.md](./retrieval.md)
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
substrate — see [retrieval.md](./retrieval.md). `pages_fts` (migration 35) is
an FTS5 index over its markdown and the *other* search over the same rows: the
embeddings answer a question and cost a cloud round trip, the index answers a
keystroke and is local, and neither falls back to the other. Its `embed_model` /
`embed_dim` columns are load-bearing rather than bookkeeping: every scan
filters on them, because a dot product between vectors from two models is not
a worse score but a meaningless one that still sorts. `lecture_chapters` (migration
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

- Parse settings are in SQLite under `parse`; the seam reads which backend to
  use out of that row (`parse_config` in `app/src-tauri/src/parse/mod.rs`),
  where `engine` is `cloud` or `local` and an optional `engineUrl` overrides
  the chosen engine's API root. **Changing it invalidates nothing** — both
  engines write the same artifacts at the same `PARSER_VERSION` — which is the
  opposite of the embed row beside it; see [parsing.md](./parsing.md).
  Two dead keys from the sidecar — `memoryCapMb` and a `local|cloud|auto`
  `backend` — still sit in the same blob and are deliberately ignored rather
  than migrated out; only `engine` selects a parser. Embed settings
  are the row beside it, under `embed` (`embed_config` in
  `app/src-tauri/src/embed/mod.rs`) — same shape, one field over. Neither
  cloud's credential joins them: keychain → in-process client, and neither
  crosses a socket at all. Neither is in SQLite, in health, or in a progress
  event.
- **Parsing blocks for minutes, and every caller is built around that.** The
  sidecar answered as soon as a fast pass had produced *some* markdown; there
  is one tier now, so a parse spans the whole cloud round trip. Concurrency
  belongs to the batcher (`app/src-tauri/src/parse/mineru/batch.rs`: a
  five-second/twenty-file window, eight batches in flight), so a scrape hands
  each PDF to a detached thread and reports itself finished.
- **Embedding blocks for longer still**, and on an account with no payment
  method on file it is the slowest thing the app does: Voyage allows 10K tokens
  a minute there, which is under three pages a minute. Neither the CLI nor the
  commands impose a timeout — the client paces itself against the tier it
  detected and a 429 is routine, so a deadline from above could only abandon
  work that was still progressing.
- **A finished parse writes its own page records.** `pages.markdown` — what
  `oculus grep` searches — used to be a side effect of the embed path, which
  would have taken it down with the embedding layer.
- The startup sequence in `app/src-tauri/src/lib.rs` is: bind parse events →
  start the media server → clean partial lecture downloads → seed WebKit's cookie jar
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
  Three things about a page live in the page and nowhere else — whether its
  back list has anywhere to go, what a find matched, what the zoom is — and
  Tauri has an API for only the zoom, so `browser.rs` reads and drives them on
  the WKWebView through `with_webview`. That call dispatches to the main
  thread and hands nothing back, so each of them is a *push*: the answer is
  written into the tab and broadcast, or emitted on its own event, rather than
  returned to the command that asked.
- **Rust fetches favicons; the frontend keeps them.** WebKit has no public
  icon API, so `browser.rs` fetches one over plain HTTP beside each page load
  (`/favicon.ico`, then the document's `<link rel~="icon">`) and pushes it as
  `browser-favicon`, keyed by host and once per host per run. The frontend
  stores it in `browser_favicons`, which is the shape every other table has
  here: Rust sees the events, the frontend owns the rows. Browsing history is
  written the same way, from the `browser-state` snapshot — see
  [frontend.md](./frontend.md) for what is deliberately *not* written into
  it.
- Everything Canvas-shaped was **moved out of hidden WebViews on purpose**:
  macOS suspends off-screen WKWebView content processes, which froze the old
  `scraper.js` mid-run with nothing to catch. The scrape engine is Rust
  (`app/src-tauri/src/sync.rs`); do not move background work back into a
  WebView.
- **Nothing local can leave a PDF unindexed any more** — there is no venv to
  be missing and no model to fail to load. What can is a missing cloud
  credential, a spent allowance or no network, in which case `oculus index`
  picks the file up on a later run and the file row says why in the meantime
  (see [parsing.md](./parsing.md)).
