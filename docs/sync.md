# Sync — the scrape engine

One Rust engine scrapes three services. It runs identically inside the app
(on a plain thread, reporting through Tauri events) and in the `oculus` CLI
(reporting to stdout).

## Where

| Piece | Location |
| --- | --- |
| Canvas scrape engine (modules driver) | `app/src-tauri/src/sync.rs` |
| Canvas HTTP: cookie, retries, pagination | `app/src-tauri/src/canvas.rs` |
| Ed Discussion: token, courses, threads, XML→md | `app/src-tauri/src/ed.rs` |
| Echo360 lectures (Tauri-independent core) | `app/src-tauri/src/echo360.rs` |
| Echo360 Tauri commands + session cache | `app/src-tauri/src/lectures.rs` |
| Canvas HTML → Markdown | `app/src-tauri/src/md.rs` |
| App-side entry: thread + `AppReporter` | `app/src-tauri/src/scrape.rs` |
| Headless DB writes | `app/src-tauri/src/store.rs` |
| Subject list state | `app/src-tauri/src/subjects.rs` |
| Canvas calendar (class times, due dates) | `app/src-tauri/src/calendar.rs` |
| Frontend sync page / runner | `app/src/pages/SyncPage.tsx`, `app/src/lib/syncRunner.ts` |

## How it connects

- **Modules are the driver.** The engine walks each course's modules and
  fetches pages and files through them, so nothing is downloaded twice. It
  was ported line-for-line in strategy from the old `scraper.js` (hidden
  WebView, now deleted) and keeps the same on-disk layout under
  `courses/<code>/`.
- **Every converted body feeds one link crawl.** Each phase that converts a
  Canvas HTML body (home/syllabus, announcements, assignment/quiz
  descriptions, pages) reports the course pages and files it references into
  a per-course `LinkCrawl` (`app/src-tauri/src/sync.rs`), drained depth-first
  after the content phases: fetched pages surface further links, `seen` sets
  break cycles. Anything reachable from any scraped body lands on disk, no
  matter which phase found it.
- **What a run fetches is configurable.** `SyncOptions` (announcements,
  assignments+quizzes, modules, Ed) gates the phases; the app's gear next to
  "Sync now" (`app/src/components/sync/SyncSettings.tsx`) persists the choice
  in settings and passes it to `scrape_content` per run. The CLI always
  syncs everything. A "Lectures" toggle refreshes each synced subject's
  Echo360 lecture *list* from the frontend after the scrape (metadata only —
  never downloads videos), and a "Calendar" toggle does the same for Canvas
  class times and due dates (see [calendar.md](./calendar.md)). Neither is a
  Rust scrape phase: both run after `scrape-complete`, write only to the
  database, and are ignored by the engine's `SyncOptions`.
- **Unchanged files are not re-downloaded.** `file-manifest.json` in the data
  dir maps Canvas file id → (`modified_at`, size) at last download; when the
  metadata call reports the same pair and the artifact is on disk, only that
  metadata call is spent. Bodies (pages, announcements, tasks, Ed threads)
  are always re-fetched and re-generated — the byte-compare in
  `paths::write_course_bytes` is what decides new/updated/unchanged, so e.g.
  a changed submission status still lands. "Re-download" bypasses the skip.
- **Changed bytes invalidate the parse.** An `updated` write purges the
  sidecar artifacts (`.md`, `.pages.json`, `.emb.json` — see
  `paths::purge_parse_artifacts`), and the app clears the file's stored
  pages and parse/embed statuses, so the pipeline re-runs instead of the
  sidecar's existence checks pinning stale markdown and vectors.
- **Progress leaves through a `Reporter` trait**, not a channel to the UI.
  `app/src-tauri/src/scrape.rs` implements it by emitting the same Tauri
  events the frontend already listened for; the CLI implements it by
  printing. The UI contract did not change when the scraper left the WebView.
- `app/src-tauri/src/canvas.rs` is the **entire** Canvas HTTP surface — the
  session cookie, retry policy, and Link-header pagination live only there.
  Both scraping and auth probing go through it.
- **Ed threads arrive as a custom `<document>` XML dialect**, converted to
  markdown in `app/src-tauri/src/ed.rs`. It is parsed with an HTML parser,
  which forces three renames/workarounds: `<link>` is HTML-void (renamed to
  `edlink` before parsing), `<image>` becomes `<img>`, and `<break/>`
  swallows following siblings as children — so every renderer emits its
  marker and then still recurses.
- **Ed course → Canvas subject mapping is fuzzy by necessity**: Ed course
  codes are staff-typed free text ("comp10002 2024s2"), matched by leading
  code token + year + semester from `/api/user` enrolments.
- **Echo360 access is an LTI launch, not an API key.** Canvas mints an
  OAuth-signed form on the course's external-tool page; POSTing it to
  Echo360 creates the session, and the CloudFront cookies that come back are
  what the media CDN accepts. Everything starts from the Canvas cookie.
  Videos are trimmed with the fetched ffmpeg binary; VTT captions are
  aligned to the trimmed timeline.
- **Scrape and parse are decoupled.** A scrape completes even when the
  sidecar is down; parsing/embedding of the PDFs it wrote is a separate,
  idempotent pass (see [sidecar.md](./sidecar.md) and
  [retrieval.md](./retrieval.md)).
- **Parse requests are queued, not spawned per file.** Each new PDF used to
  get its own detached thread, so a first sync of a full library fired every
  deck at the sidecar at once — which FastAPI happily ran 40-wide, at ~2 GB
  each. Two workers (`PARSE_WORKERS` in `app/src-tauri/src/sync.rs`) now drain
  a channel, matching the sidecar's own cap; the rest wait without holding a
  thread and a socket. Still fire-and-forget: the workers outlive the scrape
  and drain when the engine drops.
- `app/src-tauri/src/md.rs` converts Canvas HTML bodies to markdown by
  refusing to descend into cruft nodes rather than stripping them first —
  same output as the old DOM-mutating converter, no mutable tree.
