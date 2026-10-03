# Architecture

Two processes — a React frontend in a WebView and a Rust core — sharing one
data directory. PDF parsing and page embedding are HTTP calls made in-process
from Rust, behind the seams in `app/src-tauri/src/parse/` and
`app/src-tauri/src/embed/`.

## Where

| Piece | Location |
| --- | --- |
| App entry, startup, command registry | `app/src-tauri/src/lib.rs` |
| Schema migrations | `app/src-tauri/src/migrations.rs` |
| Data-dir and library path rules | `app/src-tauri/src/paths.rs` |
| Parse seam and its two MinerU clients | `app/src-tauri/src/parse/mod.rs`, `app/src-tauri/src/parse/mineru/` |
| Embed seam, Voyage client, page rasterizer | `app/src-tauri/src/embed/mod.rs`, `app/src-tauri/src/embed/voyage/`, `app/src-tauri/src/embed/raster.rs` |
| Ingest and search over `pages` | `app/src-tauri/src/retrieval.rs` |
| Rate limiting both cloud clients share | `app/src-tauri/src/ratelimit.rs` |
| Shared parse/embed event payload and channels | `app/src-tauri/src/pipeline_events.rs` |
| Blocking-command adapter | `app/src-tauri/src/blocking.rs` |
| Crash-safe file/JSON ledger replace, wall clock, test scaffolding | `app/src-tauri/src/atomic_write.rs`, `app/src-tauri/src/clock.rs`, `app/src-tauri/src/test_support.rs` |
| Credential storage (keychain only); provider probes | `app/src-tauri/src/credentials.rs`; `app/src-tauri/src/mineru.rs`, `app/src-tauri/src/voyage.rs`, `app/src-tauri/src/okta.rs` |
| Lecture video server | `app/src-tauri/src/media.rs` |
| In-app browser | `app/src-tauri/src/browser.rs`, `app/src-tauri/capabilities/default.json` |
| CLI-agent harness | `app/src-tauri/src/harness/mod.rs` |
| Headless DB writes | `app/src-tauri/src/store.rs`, `app/src-tauri/src/projects.rs` |
| File-pipeline event payload and bound channels | `app/src-tauri/src/pipeline_events.rs` |
| Frontend DB access and event folding | `app/src/lib/db.ts`, `app/src/hooks/useBackendEvents.ts` |
| The CLI over the same engine | `app/src-tauri/src/bin/oculus/` |

## Oculus starts no child process for parsing or embedding

Both are HTTPS calls from inside the Rust process: MinerU for parsing, Voyage
for embeddings. Parsing has a second engine, a MinerU server on
`127.0.0.1:8000` — but that is a program the user installed and started, which
Oculus never launches, supervises or ships. Same boundary as the cloud, different
hostname. Nothing listens for calls back into the app. The engine choice and
its failure rules are in [parsing.md](./parsing.md).

## The frontend and Rust talk only through commands and events

- Tauri commands go in; events come out. Scrape, parse and embed progress
  arrive as events that `app/src/hooks/useBackendEvents.ts` folds into zustand
  stores. Parse and embed share the optional-field payload in
  `pipeline_events`, while their channels keep separate success and error vocabularies.
- `parse::events` and `embed::events` bind the app handle once, first thing in
  `setup`, instead of threading it through the call path. The CLI never binds,
  so the same parse and embed code runs headless and its emits are no-ops.
- Library text reads, bulk parse-artifact scans, upload import/conversion,
  Canvas calendar sync and Echo360's HTTP, downloads and ffmpeg run through
  `blocking::run`, so synchronous I/O cannot hold Tauri's command thread.
  Upload batches serialize their name allocation.
- Credentials go keychain → in-process client. Neither key enters SQLite, the
  WebView, a health response or a progress event. The local engine needs none.

## Scraping lives in Rust because hidden WebViews freeze

macOS suspends an off-screen WKWebView's content process, which freezes
anything running in it mid-run with nothing to catch. So the scrape engine is
`app/src-tauri/src/sync.rs`, and the headless Okta sign-in
(`app/src-tauri/src/okta.rs`) runs in Rust too. Never move background work into
a WebView.

## Lecture video streams over localhost HTTP

WebKit refuses `<video>` sources on custom URL schemes: an `asset://` URL
fetches but the media element fails with error code 4 (macOS 26). So
`app/src-tauri/src/media.rs` serves lecture video on an ephemeral localhost
port with a per-launch token and Range support, scoped to `lectures/` and
`courses/`. The frontend gets URLs from `mediaSrc()` in `app/src/lib/media.ts`.

## One data directory, resolved without a Tauri handle

`paths::data_dir()` computes the directory Tauri would
(`~/Library/Application Support/com.tchan.oculus`) with no `AppHandle`, and is
the only way Rust reaches it — so the CLI and the app cannot disagree. Inside:

- `oculus.db` — SQLite, everything structured.
- `courses/<code>/…` — scraped files in Canvas's layout, with the parser's
  `.md`, `.pages.json` and `<stem>_images/` siblings and the embedder's
  `.emb.json`. Two subfolders are the student's own, never written by a sync:
  `uploads/` (copied in by `import_uploads`) and `documents/` (notes written in
  the app by `create_document`, with pasted images in `documents/assets/`),
  both in `app/src-tauri/src/files.rs`. Rename and delete commands are scoped
  to those shapes (`is_document_rel` in `app/src-tauri/src/paths.rs`), which is
  what makes deleting there safe.
- `lectures/<uuid>/` — Echo360 media (`source1.mp4`, optionally
  `source2.mp4`) and `transcript.vtt`, plus regenerable `frames/` and
  `outline.md` from the chapter jobs. Which source is the slides is measured,
  not assumed — see [chapters.md](./chapters.md).
- `agents/` — the docs, skills and memory layer a coding agent reads and
  writes ([cli.md](./cli.md#oculus-docs-writes-the-agents-folder)); the
  working directory and only writable root of every chat thread, with raw
  provider output in `agents/threads/<id>.ndjson` ([harness.md](./harness.md)).
- `mineru-usage.json`, `voyage-usage.json` — each cloud's daily reservations
  and quota latch. Voyage's also holds the learned rate-limit tier and the
  spend guard from Settings → Library, kept here rather than in `settings`
  because the reservation that enforces it already reads this file
  ([retrieval.md](./retrieval.md)).
- The session cookie, auth flag and `session-keepalive.log` ([auth.md](./auth.md)).

## The database has one schema owner and two writers

Schema is the append-only, numbered migration list in
`app/src-tauri/src/migrations.rs`; the highest `version` is the current schema.
An applied migration's SQL, comments and whitespace included, is frozen;
editing it fails `Database.load` with a checksum mismatch.
In the app the *frontend* writes the scrape tables, upserting through
`app/src/lib/db.ts` as scrape events arrive. Headless, `store.rs` writes the
same rows with the same SQL, so a CLI sync looks like an app sync. The CLI
never creates the database, so a fresh machine opens the app once first.

- `pages` (markdown + embedding per PDF page) is the retrieval substrate. Its
  `embed_model`/`embed_dim` columns filter every scan, because a dot product
  across two models is meaningless but still sorts. `pages_fts` is a local FTS5
  index over the same markdown; neither search falls back to the other
  ([retrieval.md](./retrieval.md)).
- A parse writes `pages.markdown` without waiting on the embed, so a file with
  no embeddings is still found by keyword.
- `lecture_chapters` is derived and regenerable ([chapters.md](./chapters.md)).
- `calendar_events` is the one table a sync replaces rather than upserts, so a
  cancelled class disappears ([calendar.md](./calendar.md)).
- `projects`, `project_tasks` and `local_events` hold the student's own rows,
  which nothing upstream has a copy of. Their `subject_id` is nullable and
  `ON DELETE SET NULL`, so dropping a course never takes the user's work with
  it. Written by `app/src/lib/projects.ts` in the app and
  `app/src-tauri/src/projects.rs` headless ([projects.md](./projects.md)).
- `harness_threads`/`harness_items` are the chat timeline
  ([harness.md](./harness.md)).
- Parse and embed settings are the `parse` and `embed` rows of `settings`, read
  by `parse_config` and `embed_config`. `store::edit_setting` preserves unknown
  keys when changing either object; malformed records start from defaults.

## The main window holds one webview per browser tab

External links open as in-app browser tabs: each is a child webview of remote
content stacked over the slot the `/browse/:id` route leaves in the content
card ([frontend.md](./frontend.md)). The frontend reports that slot as window
insets and Rust lays pages out from them, so a resize never waits on
JavaScript.

- `app/src-tauri/capabilities/default.json` is scoped to the `main` *webview*,
  not the window: window and webview scopes match by OR, so a window scope
  would hand every Tauri command to whatever page the user browsed to.
- Back-list state, find matches and zoom live only in the page, and
  `with_webview` dispatches to the main thread and returns nothing. So
  `browser.rs` *pushes* each answer as an event rather than returning it.
- WebKit has no public favicon API, so Rust fetches the icon beside each page
  load and emits `browser-favicon`; the frontend owns the `browser_favicons`
  rows. Rust sees events, the frontend owns rows — the same split as history.

## Gotchas

- Background work in a hidden WebView freezes silently — keep it in Rust ([above](#scraping-lives-in-rust-because-hidden-webviews-freeze)).
- Video over `convertFileSrc`/`asset://` fails with media error 4 — use `mediaSrc()`.
- A window-scoped capability exposes every command to browsed pages — keep `webviews: ["main"]`.
- Reaching the data dir any way but `paths::data_dir()` lets the CLI and app diverge.
- Comparing vectors without filtering on `embed_model`/`embed_dim` returns confident garbage.
- A `subject_id` that cascades on user-owned tables deletes the student's work with a course.
