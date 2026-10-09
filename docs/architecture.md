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
| Credential storage (keychain only); provider probes | `app/src-tauri/src/credentials.rs`; `app/src-tauri/src/mineru.rs`, `app/src-tauri/src/voyage.rs`, `app/src-tauri/src/groq.rs`, `app/src-tauri/src/okta.rs` |
| Lecture video server | `app/src-tauri/src/media.rs` |
| Video transcription (Groq Whisper, then Apple's on-device speech, then local whisper.cpp) | `app/src-tauri/src/transcribe/`, `app/src-tauri/speech/main.swift` |
| Locating the shipped native helpers (ffmpeg, `apple-speech`, `whisper-cli`) | `app/src-tauri/src/bundled.rs` |
| PDF viewer's render, text and links | `app/src-tauri/src/pdf_view.rs` |
| In-app browser | `app/src-tauri/src/browser.rs`, `app/src-tauri/capabilities/default.json` |
| CLI-agent harness | `app/src-tauri/src/harness/mod.rs` |
| Headless DB writes | `app/src-tauri/src/store.rs`, `app/src-tauri/src/projects.rs` |
| App-usage ticker and its pinger | `app/src-tauri/src/usage.rs`, `app/src/hooks/useActivityPing.ts`, `app/src/lib/usageContext.ts` |
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
  Canvas calendar sync, Echo360's HTTP, downloads and ffmpeg, and video
  transcription run through `blocking::run`, so synchronous I/O cannot hold
  Tauri's command thread.
  Upload batches serialize their name allocation.
- Credentials go keychain → in-process client. No key enters SQLite, the
  WebView, a health response or a progress event. The local engine needs none.
  A read the keychain refuses (a denied prompt, or `oculus` inside Claude's
  sandbox, which fails right after the prompt is approved) is reported as
  unreadable, never as a missing key (`Secret::fetch`, `Secret::has`). MinerU's
  parse path is the exception: it still reads a refusal as no token
  (`Secret::read`).

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
  spend guard from Settings → Embeddings, kept here rather than in `settings`
  because the reservation that enforces it already reads this file
  ([retrieval.md](./retrieval.md)).
- The session cookie, auth flag, `session-keepalive.log`, and the sign-in
  attempt record and its `okta-sign-in.log` ([auth.md](./auth.md)).

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
- `document_versions` holds a note's checkpoints and snapshots, written only by
  `app/src/lib/documentVersions.ts` ([editor.md](./editor.md#a-notes-versions-live-in-the-database-never-on-disk)).
- `harness_threads`/`harness_items` are the chat timeline
  ([harness.md](./harness.md)).
- `usage_hours` and `usage_context_hours` are written only by Rust, as the
  next section describes.
- Parse and embed settings are the `parse` and `embed` rows of `settings`, read
  by `parse_config` and `embed_config`. `store::edit_setting` preserves unknown
  keys when changing either object; malformed records start from defaults.

## Rust counts app usage; the frontend only pings

`usage_hours` holds open and active seconds per local hour, keyed
`YYYY-MM-DD HH`. A ticker in `app/src-tauri/src/usage.rs`, started in `setup`,
reads the main window every 30 s and adds 30 to the current hour's row:

- **Open**: the window is visible and not minimized.
- **Active**: open, and either focused with an `input` ping in the last 120 s
  or a `media` ping in the last 60 s — so a playing lecture counts without
  focus or touch.

The frontend calls `usage_activity` with `input` (throttled to one per 30 s)
or `media` (every 30 s while a video plays). The command only stores the
ping's wall-clock time and its context; the ticker owns every write, so a
frozen or hidden WebView can't drop or double-count time. Pings use the wall
clock because `Instant` pauses through macOS sleep and a pre-sleep ping would
look fresh on wake.

Each ping carries a context, `{ kind, subjectId }` from `usageContext` in
`app/src/lib/usageContext.ts`: what sort of page it came from (lecture, file,
document, course, chat, browser, planning, other) and the subject it belongs
to. `useActivityPing` (`app/src/hooks/useActivityPing.ts`) sends the focused
pane's context with input, and pings at once, past the throttle, whenever
navigation, a tab switch or a focus move between main page and side panel
changes it within a few seconds of real input — a tab restoring at launch is
not use. A media beat carries the context of the pane hosting the element
that started playing (found through the pane root's `data-pane-id`), fixed
until playback stops, so a lecture playing while the user reads elsewhere is
still credited to the lecture once input goes quiet. Rust keeps the latest
context per ping kind; a ping without one leaves it alone, and an unknown kind
is refused.

An active tick adds its 30 s to `usage_context_hours` (hour, kind, subject —
0 outside one) in the same transaction as `usage_hours`. It credits the input
context when input made it active, which wins over media, else the media
context; with none reported yet, it credits `other`. Ticks before the frontend
has applied the migrations fail quietly and later ones retry.

## The main window holds one webview per browser tab

External links open as in-app browser tabs: each is a child webview of remote
content stacked over the slot the `/browse/:id` route leaves in the content
card ([viewers.md](./viewers.md#the-in-app-browser-is-a-native-page-per-tab-owned-by-rust)). The frontend reports that slot as window
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
