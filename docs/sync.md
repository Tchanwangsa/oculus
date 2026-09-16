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
| Agent docs written into the library | `app/src-tauri/src/agents.rs` |
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
- **A scrape scaffolds the agent docs it just made relevant.** `Engine::scrape`
  writes the central `agents/AGENTS.md` once per run and links each subject's
  course folder after scraping it (`app/src-tauri/src/agents.rs`), so a subject
  enrolled mid-semester is usable by a coding agent from the run that first
  fetches it — nobody has to remember `oculus docs`. The order matters: the
  central copy is written *before* anything points at it, because the links are
  relative and would otherwise dangle on a library where the CLI never ran. A
  subject whose scrape produced no folder is skipped rather than given an empty
  one, and a failure here is a warning, never a failed sync — the scaffold is
  an affordance, not part of the library. What the run cannot write is
  `OCULUS-CLI.md`, which is rendered from clap's command tree and so belongs to
  the binary; see [cli.md](./cli.md) for the three lifetimes.
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
  pages and parse status, so the parse re-runs instead of the existence
  check pinning stale markdown.
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
- **A capture is one lesson with up to two streams.** The Presenter screen and
  the room camera are `hd1.mp4` and `hd2.mp4` behind a single media id, so a
  "source" is a file name, not a second recording; they land side by side as
  `source1.mp4` and `source2.mp4` and are trimmed identically, which is what
  lets the player run them off one clock (see [frontend.md](./frontend.md)).
  Only source 1 is fetched by a sync — the camera roughly doubles a semester
  on disk and is downloaded per lecture, on demand, from the player.
- **Whether a camera exists is probed, not read.** `syllabus` looks for
  `secondaryFiles` anywhere under the lesson (the nesting has moved between
  Echo360 versions, so it searches for the key rather than a path), but this
  university's syllabus carries no file lists at all — so in practice every
  lecture falls back to asking the download endpoint for `hd2.mp4`. That
  answer is trustworthy: Echo360 resolves the stream before it signs anything,
  so a missing source is a 500 rather than a signed URL that would 404 later
  (measured against `hd3.mp4`, which is never real). It costs one redirect per
  lecture, and a `warn` line says when the fallback is being used — if the
  syllabus ever starts carrying file lists again, that line goes quiet and the
  requests stop.
- **Scrape and parse are decoupled.** A scrape completes even when parsing
  is unavailable; parsing the PDFs it wrote is a separate, idempotent pass.
  The frontend's two-stage view of it (`download → parse`), the `parse-status`
  event vocabulary and the background sweep that picks up what was missed are
  in [frontend.md](./frontend.md).
- **The parse is in this process, and it takes minutes.** `parse_pdf` in
  `app/src-tauri/src/sync.rs` goes through the seam in
  `app/src-tauri/src/parse/mod.rs` rather than POSTing to a sidecar port. The
  sidecar used to return in seconds — as soon as a fast pass had produced some
  markdown — and finish the real parse on its own thread. There is no fast tier
  now, so the call spans the entire cloud round trip, and there is no timeout
  here either: the client's own 60-minute poll deadline is the single limit, so
  nothing shorter can abandon work it is still doing.
- **Fire-and-forget is one detached thread per PDF**, and the bounded worker
  pool that used to be here is gone. The pool existed because every parse was
  an HTTP request and a full library meant a hundred simultaneous POSTs at ~2 GB
  each. Concurrency is the batcher's now
  (`app/src-tauri/src/parse/mineru/batch.rs` — a five-second/twenty-file window,
  eight batches in flight), and a second gate would only stop files reaching the
  window they are meant to share. Each thread spends its wait parked on a
  condvar: no socket, no request in flight.
- **Finishing a parse writes its own page records**, into `pages` via
  `store::upsert_pages`. That write used to live on the embed path, which made
  the markdown `oculus grep` searches a side effect of building the vector
  index. Hitting an *already*-parsed file folds its `.pages.json` in too, but
  only when the file has no page rows at all — the library holds files parsed
  before this write existed, and that is the repair path for them. See
  [retrieval.md](./retrieval.md).
- `app/src-tauri/src/md.rs` converts Canvas HTML bodies to markdown by
  refusing to descend into cruft nodes rather than stripping them first —
  same output as the old DOM-mutating converter, no mutable tree.
