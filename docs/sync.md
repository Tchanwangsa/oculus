# Sync — the scrape engine

One Rust engine scrapes three services. It runs identically inside the app
(on a plain thread, reporting through Tauri events) and in the `oculus` CLI
(reporting to stdout).

Windows uses the same engine and `/`-separated library paths. Native filesystem
paths are constructed only when opening an artifact. Office conversion in
`app/src-tauri/src/sync.rs` discovers LibreOffice from `OCULUS_SOFFICE`, PATH,
or the standard Program Files locations and launches it without a console
window. LibreOffice is an optional external installation; a missing converter
is reported by the existing conversion error path.

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
| Chronological term ranking | `app/src-tauri/src/terms.rs` |
| Personal subject file import and deletion | `app/src-tauri/src/files.rs` |
| Agent docs written into the library | `app/src-tauri/src/agents.rs` |
| Canvas calendar (class times, due dates) | `app/src-tauri/src/calendar.rs` |
| Frontend sync page / runner | `app/src/pages/SyncPage.tsx`, `app/src/lib/syncRunner.ts` |

## How it connects

- **The current term is ranked, not compared as text.** `list_courses` marks
  the newest term that still has available courses as current, and that flag
  is what `oculus run` syncs by default, what `oculus list` marks, and what a
  search with no named subject falls back to. Canvas term names do not sort
  chronologically — `"2026 Summer Term"` beats `"2026 Semester 2"` as a
  string while starting six months earlier — so `app/src-tauri/src/terms.rs`
  ranks the term within its year (summer, semester 1, winter, semester 2).
  A term Canvas names after its month rather than a semester — `"2026 June"`
  for a winter intensive — takes the rank of the term it falls inside, so it
  cannot rank as unknown and outrank the semester being studied.
  Before that, one summer enrolment marked a whole year of real subjects as
  past, and a default CLI sync fetched the summer subject alone.
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
- **Office documents are stored as themselves plus a derived PDF.** Everything
  downstream — the parsers, the page-image embedder, the viewer — is
  PDF-shaped, so `.pptx/.docx/.xlsx/.ppt/.doc/.xls` are downloaded intact and
  LibreOffice headless writes `deck.pptx.pdf` beside the original
  (`office_to_pdf` in `app/src-tauri/src/sync.rs`). The derived PDF is never
  announced and never gets a `files` row — the original name is the library
  row, which is what `paths::doc_pdf_rel` resolves for every consumer.
  Migration 10 in `app/src-tauri/src/lib.rs` exists because those PDFs once
  did get rows. **With LibreOffice absent the original is still stored**, the
  run logs a warning, and the file stays out of `file-manifest.json` so the
  next sync retries the conversion rather than skipping it. `office_to_pdf`
  and `office_ext_of` are `pub(crate)` for one caller outside this engine:
  the student's own uploads go through the very same conversion
  (`app/src-tauri/src/files.rs`, [frontend.md](./frontend.md)), so a dropped
  `.docx` is searchable on exactly the terms a scraped one is.
- **A spreadsheet is exported as one page per sheet**, not with Calc's default
  pagination (`convert_target` in `app/src-tauri/src/sync.rs`). Calc slices a
  wide sheet into page-width column bands and gives the later bands no
  headers: a 300-row × 25-column marks sheet exported as 54 pages, of which
  only the first band carried the ID and name columns — the rest were bare
  grids of numbers, which is both a meaningless page image and the markdown a
  citation would hydrate from. `SinglePageSheets` keeps every row with its
  headers; `dpi_for_page` in `app/src-tauri/src/embed/raster.rs` bounds
  oversized page renders before they reach the embedding backend.
- **An untyped upload is judged by its extension.** Canvas reports whatever
  content type the uploading browser claimed, so the same deck arrives typed
  on one course and `application/octet-stream` on another. A generic type
  falls back to the filename (`office_ext_of`), which is still an allowlist —
  only the extensions the converter handles, plus `.pdf`.
- **Changed bytes invalidate the parse.** An `updated` write purges the
  parse/embed artifacts (`.md`, `.pages.json`, `.emb.json` and the page-image folder — see
  `paths::purge_parse_artifacts`), and the app clears the file's stored
  pages and parse/embed statuses, so the pipeline re-runs instead of the
  pipeline's artifact checks pinning stale markdown and vectors.
- **Personal files live in a subject's `uploads/` folder.** `import_uploads`
  copies picked files into `courses/<code>/uploads/` and returns a result for
  each pick, so one failure does not discard the other imports. Office files
  use the same converter as a sync; if conversion fails, the original still
  gets a library row and a visible warning. Parsing, search and chat then use
  the same library paths as downloaded course material.
  Imports preserve identical files and rename different files that share a
  name. Allocation also reserves converted PDFs, markdown, embedding records
  and image-folder names, so `notes.md` cannot be overwritten by parsing a
  later `notes.pdf`. Concurrent imports are serialized and new files are
  created without overwrite. Names retain the Windows port's device-name and
  Unicode-length handling.
- **Only personal uploads can be deleted.** `delete_upload` removes the
  original, its converted PDF and parse artifacts. Its lexical check accepts
  only flat, portable upload paths; filesystem checks reject symbolic links
  and Windows junctions in the subject/upload folders or derived artifacts.
  Recursive image cleanup resolves its target inside the library first.
  Deleted names remain reserved by zero-byte markers in the internal
  `.oculus-upload-reservations/` folder: a parser already running may finish
  after deletion, and its late output must never become another file's parse.
  Deletion is safe to retry after the original is gone, so a failed database
  row deletion does not strand the entry. Every retry repeats the same path
  checks and removes any parse artifacts that arrived late.
  A downloaded Canvas file is never eligible for this command.
- **Progress leaves through a `Reporter` trait**, not a channel to the UI.
  `app/src-tauri/src/scrape.rs` implements it by emitting the same Tauri
  events the frontend already listened for; the CLI implements it by
  printing. The UI contract did not change when the scraper left the WebView.
- **CLI success requires metadata persistence.** A failed history insert stops
  the run before scraping. A failed file-row or completion write returns an
  error and records failed history where the database still permits it; bytes
  downloaded to disk are retained for the next sync. The CLI's following index
  phase includes Office originals whose converted siblings are available, as
  well as native PDFs.
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
- **A download can be cancelled, and a downloaded video deleted.**
  `echo360_cancel_download` and `echo360_delete_video` in
  `app/src-tauri/src/lectures.rs`, both keyed by media id and source like the
  progress bars. The transfer is a blocking read loop, so cancelling is an
  `AtomicBool` in `DownloadCancels` that `stream_to_file` checks between 64 KB
  chunks; it then returns `echo360::CANCELLED`, and the existing error path
  deletes the partial. The frontend tells that phase apart from a failure and
  clears its bar without reporting one. **Deleting removes only the video
  files** — the transcript, chapters and recap notes are kilobytes and cost an
  agent turn each to rebuild, while the video re-downloads unattended. A
  delete cancels an in-flight download for the same lecture first, so the
  writer cannot recreate the file just after it is removed.
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
  The frontend's view of it (`download → parse → embed`, the third stage drawn
  only when a Voyage key is stored), the `parse-status` / `embed-status` event
  vocabularies and the background sweep that picks up what was missed are in
  [frontend.md](./frontend.md). A finished parse queues that file's embedding
  behind it, so a sync run reaches all three stages without a button — see
  [retrieval.md](./retrieval.md).
- **The parse is in this process, and it takes minutes.** `parse_pdf` in
  `app/src-tauri/src/sync.rs` goes through the seam in
  `app/src-tauri/src/parse/mod.rs` and blocks until the chosen backend is done.
  The Python used to return in seconds — as soon as a fast pass had produced
  some markdown — and finish the real parse on its own thread. There is no fast
  tier now, so the call spans the whole of it, and no deadline is imposed from
  here on either engine: the cloud client owns a 60-minute poll deadline, and a
  local parse is a connect timeout followed by minutes of silence on this
  machine's own CPU. Anything shorter could only abandon work still in
  progress. A loopback POST does exist again, but it belongs to the local
  backend behind the seam — `sync.rs` no longer knows a port is involved, which
  is the part the sidecar's removal actually settled.
- **Fire-and-forget is one detached thread per PDF**, and the bounded worker
  pool that used to be here is gone. The pool existed because every parse was
  an HTTP request and a full library meant a hundred simultaneous POSTs at ~2 GB
  each. Concurrency is the backend's now, and the two answer it differently:
  the cloud batches (`app/src-tauri/src/parse/mineru/batch.rs` — a
  five-second/twenty-file window, eight batches in flight), while the local
  engine takes a single permit (`app/src-tauri/src/parse/mineru/local.rs`),
  because the server on the other end is this machine and works one document at
  a time regardless. A gate back here would serve neither: it would only keep
  cloud files out of the window they are meant to share. Each thread spends its
  wait parked on a condvar or on that permit — no socket, no request in
  flight.
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
