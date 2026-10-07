# Sync — the scrape engine

One Rust engine scrapes Canvas, Ed Discussion and Echo360. It runs identically
inside the app (on a plain thread, reporting through Tauri events) and in the
`oculus` CLI (reporting to stdout).

## Where

| Piece | Location |
| --- | --- |
| Canvas scrape engine (modules driver, link crawl, Office conversion) | `app/src-tauri/src/sync.rs` |
| Canvas HTTP: cookie, retries, pagination | `app/src-tauri/src/canvas.rs` |
| Ed Discussion: token, courses, threads, XML→md | `app/src-tauri/src/ed.rs` |
| Echo360 core; its Tauri commands, session cache, downloads | `app/src-tauri/src/echo360.rs`, `app/src-tauri/src/lectures.rs` |
| Canvas HTML → Markdown | `app/src-tauri/src/md.rs` |
| App-side entry: thread + `AppReporter`; on-demand module videos | `app/src-tauri/src/scrape.rs` |
| Headless DB writes, subject list state | `app/src-tauri/src/store.rs`, `app/src-tauri/src/subjects.rs` |
| Chronological term ranking | `app/src-tauri/src/terms.rs` |
| Agent docs written into the library | `app/src-tauri/src/agents.rs` |
| Frontend sync page, runner, options | `app/src/pages/SyncPage.tsx`, `app/src/lib/syncRunner.ts`, `app/src/components/sync/SyncSettings.tsx` |

## Scraping lives in Rust, never in a WebView

The engine is plain Rust on a thread because a hidden WebView is suspended by
macOS (see [architecture.md](./architecture.md)). Progress leaves through the
`Reporter` trait in `app/src-tauri/src/sync.rs`: `app/src-tauri/src/scrape.rs`
emits Tauri events, the CLI prints. A file announced as downloading
(`scrape-file-start`) always ends in `scrape-file` or, when its download or
save fails, `scrape-file-failed` (`{ subject_id, relative_path, error }`, a
sentence that never carries the signed URL). `app/src-tauri/src/canvas.rs` is the
entire Canvas HTTP surface — cookie, retry policy and Link-header pagination
live only there, and both scraping and auth probing go through it.

## The current term is ranked, not compared as text

`list_courses` marks the newest term that still has available courses as
current; `oculus run` syncs it by default, `oculus list` marks it, and a
search with no named subject falls back to it. Canvas term names do not sort
chronologically (`"2026 Summer Term"` > `"2026 Semester 2"` as strings), so
`app/src-tauri/src/terms.rs` ranks within the year — summer, semester 1,
winter, semester 2 — and a month-named intensive (`"2026 June"`) takes the rank
of the term it falls inside.

## Modules drive the scrape, and every body feeds one link crawl

- The engine walks each course's modules and fetches pages and files through
  them, mirroring Canvas's layout under `courses/<code>/`.
- Every phase that converts an HTML body (home/syllabus, announcements,
  assignment/quiz descriptions, pages) reports the pages and files it links
  into a per-course `LinkCrawl`, drained depth-first after the content phases
  with `seen` sets breaking cycles. Anything reachable from any scraped body
  lands on disk.
- `SyncOptions` (announcements, assignments+quizzes, modules, Ed) gates the
  phases; the app persists the choice from `SyncSettings.tsx`, the CLI syncs
  everything. The "Lectures" and "Calendar" toggles are not Rust phases: the
  frontend refreshes Echo360 lecture lists and the calendar
  ([calendar.md](./calendar.md)) after `scrape-complete`, database-only.

## A scrape scaffolds the agent docs

`Engine::scrape` writes the central `agents/AGENTS.md` once per run, then links
each subject's course folder after scraping it (`app/src-tauri/src/agents.rs`),
so a subject enrolled mid-semester is usable by a coding agent from its first
sync. The central copy is written first because the links are relative. A
subject with no folder is skipped, and a failure is a warning, never a failed
sync. `OCULUS-CLI.md` is rendered from the binary's command tree, so the run
cannot write it — see [cli.md](./cli.md).

## Files are fetched once and invalidated by their bytes

- `file-manifest.json` in the data dir maps Canvas file id → (`modified_at`,
  size); when the metadata call reports the same pair and the file is on disk,
  nothing is downloaded.
- Bodies (pages, announcements, tasks, Ed threads) are always re-fetched;
  the byte-compare in `paths::write_course_bytes` decides new/updated/unchanged.
- An `updated` write purges the parse/embed artifacts
  (`paths::purge_parse_artifacts`) and the file's pages, so the parse re-runs.
- An `application/octet-stream` upload is judged by extension (`office_ext_of`, `is_video`; allowlists).
- `MAX_FILE_BYTES` (100 MB) skips any larger file except a video, which a sync never downloads.

## A sync lists module videos, and the student downloads them

Lecture videos uploaded as Canvas files (`video/*`, or an untyped
`.mp4/.mov/.m4v/.webm`) are ordinary library files fetched on request; they
have nothing to do with Echo360 lectures.

- `fetch_file` records a video without downloading it, and its module TOC line
  carries the Canvas id and the path it will land at:
  `- [title](../files/<name>.mp4) _(video <id>)_` (`file_toc_line`, parsed by
  `app/src/lib/moduleToc.ts`). Locked videos skip like any file.
- Once downloaded, a sync reports it `unchanged` while the manifest pair
  matches. A copy Canvas has since changed is left as it is: only the
  student downloads a video, through the Modules page.
- `canvas_download_video` (`app/src-tauri/src/scrape.rs`) streams it with
  `Canvas::download_to` into a `.part` sibling, renamed on success and removed
  on any failure or cancel, then reports a `scrape-file` like a synced file, so
  the frontend writes its `files` row. Progress is `canvas-video-progress`
  (`canvasFileId`, `percent`, `phase`); `canvas_cancel_video` sets the
  file's flag in `VideoCancels`, polled per chunk.
- The cookie goes only to Canvas: `download_to` sets it for a Canvas URL, and
  ureq strips `Cookie` on every redirect, so the signed file host a Canvas
  download redirects to never sees it.
- Videos never reach parse or embed: every pipeline gate is a PDF/Office
  allowlist (`paths::doc_pdf_rel`, `isPdfBacked`, `PDF_BACKED_SQL_LIST`).

## Office documents are stored as themselves plus a derived PDF

Everything downstream is PDF-shaped, so `.pptx/.docx/.xlsx/.ppt/.doc/.xls` are
kept intact and LibreOffice headless writes `deck.pptx.pdf` beside them
(`office_to_pdf`). The derived PDF never gets a `files` row; the original is
the library row, and `paths::doc_pdf_rel` resolves it for every consumer. The
student's own uploads go through the same conversion
(`app/src-tauri/src/files.rs`). A spreadsheet exports with `SinglePageSheets`
(`convert_target`), because Calc's default pagination splits wide sheets into
header-less column bands; `dpi_for_page` in `app/src-tauri/src/embed/raster.rs`
caps the resulting page's render size.

## Ed threads are a custom XML dialect

Ed returns threads as `<document>` XML, parsed with an HTML parser in
`app/src-tauri/src/ed.rs`: `<link>` is renamed `edlink` (HTML-void), `<image>`
becomes `<img>`, and `<break/>` swallows following siblings, so every renderer
emits its marker and still recurses. Ed course codes are staff-typed free text
("comp10002 2024s2"), matched to Canvas subjects by leading code + year +
semester from `/api/user`. The token is in [auth.md](./auth.md).

## Echo360 is an LTI launch with up to two streams

The app's Echo360 commands run synchronous HTTP, file I/O and ffmpeg on
blocking workers, leaving async runtime threads available for other commands.
Workers share the course session cache and source-qualified cancellation flags.

- Canvas mints an OAuth-signed form on the course's external-tool page;
  POSTing it to Echo360 creates the session, and the CloudFront cookies that
  come back are what the media CDN accepts.
- A capture is one media id with the Presenter screen at `hd1.mp4` and the room
  camera at `hd2.mp4`. They land as `source1.mp4`/`source2.mp4`, trimmed
  identically with the fetched ffmpeg so the player runs them off one clock;
  VTT captions are aligned to the trimmed timeline.
- A sync fetches only source 1; the camera downloads per lecture from the
  player. Its existence is probed — the syllabus here carries no
  `secondaryFiles`, so each lecture asks for `hd2.mp4`, which 500s if absent
  (a `warn` line says the fallback is in use).
- `echo360_cancel_download` sets an `AtomicBool` in `DownloadCancels` that the
  read loop checks between chunks, returning `echo360::CANCELLED`; the error
  path deletes the partial. `echo360_delete_video` removes only the video
  files — transcripts, chapters and recaps cost an agent turn to rebuild.

## Scrape and parse are decoupled

A scrape completes even when parsing is unavailable. Parsing is a separate,
idempotent pass — one detached thread per PDF, blocking in `parse_pdf` on the
seam in `app/src-tauri/src/parse/mod.rs` for as long as the engine takes
([parsing.md](./parsing.md)). A finished parse writes its page records via
`store::upsert_pages` so `oculus grep` never waits on the vector index, and
queues its embedding ([retrieval.md](./retrieval.md)). Hitting an
already-parsed file with no page rows folds its `.pages.json` in.

## Gotchas

- An office file whose conversion fails (no LibreOffice) is still stored but kept out of `file-manifest.json`, so the next sync retries the conversion. A derived PDF from older bytes is deleted with its parse artifacts, and the row gets a `document` parse failure (`parse::CONVERSION_FAILED`), not an endless "waiting to parse".
- A delete cancels an in-flight download of the same lecture first, or the writer recreates the file just after it is removed.
- A video download merges its entry into `file-manifest.json` as it is on disk (`record_manifest`), or it would drop what a sync saved during the download.
- Don't add a parse deadline or worker pool here — concurrency and timeouts belong to the parse backend ([parsing.md](./parsing.md)).
