# Sync — the scrape engine

One Rust engine scrapes Canvas, Ed Discussion and Echo360. It runs identically
inside the app (on a plain thread, reporting through Tauri events) and in the
`oculus` CLI (reporting to stdout).

## Where

| Piece | Location |
| --- | --- |
| Canvas scrape engine (modules driver, link crawl, Office conversion) | `app/src-tauri/src/sync/` |
| Canvas HTTP through keyd's `canvas` route: redirects, retries, pagination, streamed downloads | `app/src-tauri/src/sources/canvas/` |
| Ed Discussion: courses, threads, XML→md; its requests through keyd's `ed` route | `app/src-tauri/src/sources/ed/` |
| Ed Lessons: lessons, quiz questions, challenge settings, workspace zips, the index and slide renderers | `app/src-tauri/src/sources/ed/lessons.rs`, `app/src-tauri/src/sources/ed/workspace.rs`, `app/src-tauri/src/sources/ed/lesson_md/` |
| A challenge workspace's local sync record | `app/src-tauri/src/sources/ed/workspace_state.rs` |
| Reconcile of a recorded workspace with Ed: the decision table, conflict copies, resolve, live pulls, the sync's run | `app/src-tauri/src/sources/ed/workspace_session/reconcile/` |
| A workspace opened in the app: its socket thread, frames, commands and `ed-workspace` events; the `/api` and workspace-server calls it makes | `app/src-tauri/src/sources/ed/workspace_session/`, `app/src-tauri/src/sources/ed/workspace_api.rs`, `app/src-tauri/src/sources/ed/sahara.rs`, `app/src-tauri/src/sources/ed/live.rs` |
| Front matter on Ed files; Ed's category colours | `app/src-tauri/src/sources/ed/front.rs`, `app/src-tauri/src/sources/ed/palette.rs` |
| The Ed phase (one course resolve for threads and lessons) and the lessons phase | `app/src-tauri/src/sync/phases/ed.rs`, `app/src-tauri/src/sync/phases/ed_lessons.rs` |
| Echo360 core | `app/src-tauri/src/sources/echo360/` |
| Echo360 Tauri commands, session cache, downloads | `app/src-tauri/src/lectures/commands.rs`, `app/src-tauri/src/lectures/mod.rs`, `app/src-tauri/src/lectures/downloads.rs` |
| Canvas HTML → Markdown | `app/src-tauri/src/pages/md/` |
| App-side entry: thread + `AppReporter`; on-demand module videos and page embeds | `app/src-tauri/src/sync/scrape/` |
| Headless DB writes, subject list state | `app/src-tauri/src/db/store/`, `app/src-tauri/src/sync/subjects.rs` |
| Chronological term ordering (`term_sort`) | `app/src-tauri/src/sync/terms.rs` |
| Agent docs written into the library | `app/src-tauri/src/agents/mod.rs` |
| Frontend sync page, runner, options | `app/src/pages/tools/SyncPage.tsx`, `app/src/lib/pipeline/syncRunner.ts`, `app/src/components/sync/SyncSettings.tsx` |

## Scraping lives in Rust, never in a WebView

The engine is plain Rust on a thread because a hidden WebView is suspended by
macOS (see [architecture.md](./architecture.md)). Progress leaves through the
`Reporter` trait in `app/src-tauri/src/sync/mod.rs`: `app/src-tauri/src/sync/scrape/mod.rs`
emits Tauri events, the CLI prints. A file announced as downloading
(`scrape-file-start`) always ends in `scrape-file` or, when its download or
save fails, `scrape-file-failed` (`{ subject_id, relative_path, error }`, a
sentence that never carries the signed URL). `app/src-tauri/src/sources/canvas/` is the
entire Canvas HTTP surface — redirects, retry policy and Link-header pagination
live only there, and both scraping and auth probing go through it. It holds no
cookie: every request to the Canvas host is a `forward` to keyd's `canvas`
route, which attaches the session and keeps it current from `Set-Cookie`.

## Redirects are followed here, not in keyd

keyd never follows a redirect, so `Canvas::get` and `Canvas::download_to`
follow up to five themselves. A `Location` on the Canvas host goes through
keyd again; one anywhere else (a pre-signed S3 `public_url`, a file CDN) is
fetched directly with no cookie, so the session never reaches a second host.
A Link-header `next` is an absolute Canvas URL and is sent as its path and
query, escapes such as `%5B` untouched. The Echo360 launch page and the Canvas
legs of Ed's LTI walk use the same hop.

## A dead Canvas session ends the run

When Canvas rejects the cookie (a 401 that is not Canvas's own
`"unauthorized"`, or a redirect to Okta or `/login`), keyd signs in again and
retries once. A rejection that reaches the app therefore means that sign-in
was refused or failed, and `Canvas` raises `CanvasError::Expired`, carrying the
sign-in's `LoginError` when keyd named one. It keeps the first and fails every
later request the same way without asking keyd again, so a run is one sign-in
attempt, not one per file. `Engine::scrape` checks it after each course: it
logs one error naming the course and the reason, calls
`Reporter::canvas_expired` (the app emits `canvas-auth-expired`, which the sync
page shows as "Canvas session expired during sync"; `oculus run` marks the run
failed and exits with the same text) and returns without starting the next
course. A Canvas outage is different: a 5xx or a failed connection is retried
and then logged per course like any other failure (as is any Ed failure: Ed
has its own token), and a probe reads it as `Unreachable`, never as an expired
session. With keyd not installed every request fails with a plain "oculus-keyd
is not running or not installed".

## The current term is ranked, not compared as text

`list_courses` marks the newest term that still has available courses as
current; `oculus run` syncs it by default, `oculus list` marks it, and a
search with no named subject falls back to it. Canvas term names do not sort
chronologically (`"2026 Summer Term"` > `"2026 Semester 2"` as strings) and
mean different months in different places, so `app/src-tauri/src/sync/terms.rs`
gives each term a `YYYY-MM` key, `subjects.term_sort`:

- From the term's `start_at` when Canvas has one.
- Otherwise from the name: the first 19xx/20xx anywhere in it, and the month
  its words name — a season first (`"Summer Term 1"` is a summer term), then
  numbered semesters, trimesters, terms and quarters (as whole words, so the
  year in `"Fall Term 2026"` is no term number), then month names. Seasons
  follow the hemisphere (`Institution::southern_hemisphere`, from the hosts'
  country code): Summer opens a southern year and sits mid-year in a northern
  one. A month-named southern intensive (`"2026 June"`, July) ranks with
  Winter, and an unrecognised term sorts after every real one in its year.
- An academic-year range (`"2025-26"`, `"2025/2026"`) puts a term opening in
  the first half of a year — winter, spring, summer, or any numbered term —
  in the later year, so `"Spring 2025-26"` follows `"Fall 2025"`.
- Migration 45 fills the column for rows that have none from UniMelb's term
  names, since an install that predates the institution profile is UniMelb's;
  every course fetch rewrites it. Every reader — the frontend, the CLI —
  sorts by the column and ranks no names itself.

## Modules drive the scrape, and every body feeds one link crawl

- The engine walks each course's modules and fetches pages and files through
  them, mirroring Canvas's layout under `courses/<code>/`.
- Every phase that converts an HTML body (home/syllabus, announcements,
  assignment/quiz descriptions, pages) reports the pages and files it links
  into a per-course `LinkCrawl`, drained depth-first after the content phases
  with `seen` sets breaking cycles. Anything reachable from any scraped body
  lands on disk.
- `SyncOptions` (announcements, assignments+quizzes, modules, Ed threads,
  Ed lessons) gates the phases; the app persists the choice from `SyncSettings.tsx`, the CLI syncs
  everything. The "Lectures" and "Calendar" toggles are not Rust phases: the
  frontend refreshes Echo360 lecture lists and the calendar
  ([calendar.md](./calendar.md)) after `scrape-complete`, database-only.

## Canvas HTML becomes markdown with a few kept tags

`app/src-tauri/src/pages/md/` converts every Canvas body. The renderer parses
raw HTML in a page and sanitizes it
([viewers.md](./viewers.md#one-markdown-renderer-serves-every-surface)), so
the output is markdown plus this contract:

- **Text never reads as HTML.** Every text node turns `&` into `&amp;` and
  `<` into `&lt;` (`app/src-tauri/src/pages/md/escape.rs`), so prose like
  `your <head> section` stays text. Code spans and `<pre>` fences are literal;
  so is an `amber-pre` code block, which becomes a fence even with no `<pre>`.
- **A `$` in text is always `\$`**, so remark-math never pairs currency
  (`$10 … $15`). Maths comes from two places only
  (`app/src-tauri/src/pages/md/maths.rs`): a Canvas equation image
  (`img.equation_image`) becomes `$<LaTeX>$` from its
  `data-equation-content` (else `title`, the twice-encoded src path, or `alt`
  minus `LaTeX: `), or a `$$` block on its own lines when it is all its
  `<p>`/`<div>` holds; and MathJax `\(…\)` / `\[…\]` in text become `$…$`
  and a `$$` block that splits its paragraph. Nothing is fetched, and
  equation images are never downloaded as images. A table cell keeps maths
  inline.
- **`<details>` stays HTML around markdown**:
  `<details>\n<summary>text</summary>\n\n…\n\n</details>`. The summary is
  plain text, since it shares the open tag's HTML block; the blank lines are
  what make CommonMark parse the body. Nested ones nest.
- **Show/hide widgets are collapsed `<details>`**, as the page shows them
  (`app/src-tauri/src/pages/md/widgets/`): a `uom-ui-accordion` /
  `uom-mspace-accordion` trigger with its panel, a `uom-ui-reveal` button with
  the `uom-ui-hidden-content` after it, each tab of a `uom-ui-tablist` with its
  panel (by `aria-controls`, else in order), and a Canvas `element_toggler`
  with the element its `aria-controls` names, rendered at the toggler. The
  paired body is not rendered again where it sits.
- **Other widgets:** consecutive `p.uom-ui-checkbox` become one task list
  (`- [ ] text`); a `table.uom-ui-tiles` becomes its cells' blocks, not a
  table; a `uom-ui-notice`, a Bootstrap `alert` or a block with
  `uom-ui-emphasise` becomes a blockquote, headed `> **Note**`
  (info/primary), `> **Warning**` (warning/danger) or `> **Tip**` (success)
  when the class names a kind. A `div` whose inline style has both a real
  border and a background and that holds block elements is an unlabelled
  blockquote; inline styles are otherwise ignored.
- **Videos Oculus can play become tags**, each with its open tag alone on a
  line after a blank one (a CommonMark HTML block), splitting any paragraph or
  indented under a list item; a table cell keeps one on one line. Every
  `title` is the iframe's, whitespace-collapsed, and left out when empty or a
  player's name (`Kaltura Player`, `YouTube video player`, …)
  (`app/src-tauri/src/pages/md/embeds.rs`):
  - `<echo-video link-id src title>` for an Echo360 LTI launch: `src` is the
    Canvas `external_tools/retrieve` URL made absolute, `link-id` the
    `linkId` inside its `url` parameter.
  - `<canvas-video file-id title>` for a Canvas
    `/media_attachments_iframe/<id>` player, whose id is an ordinary Canvas
    file; `title` drops the leading `Video player for `. The link crawl never
    follows it: the video is downloaded only on request.
  - `<web-video provider video-id hash title>` for a YouTube `/embed/<id>`
    or `player.vimeo.com/video/<id>` player with a well-formed id; `provider`
    is `youtube` or `vimeo`, and `hash` is Vimeo's `h` parameter, present only
    for an unlisted video.
- **Any other iframe becomes a named link**, `[<Label>: <title>](<src>)` or
  `[<Label>](<src>)` when the title is a player's name or the label itself
  (`app/src-tauri/src/pages/md/labels.rs`). A Canvas LTI launch is labelled by
  the tool in its `url` parameter (`H5P`, `Kaltura`, else `Canvas tool`;
  `Readings Online` for an `ERES_` title with no `url`); other hosts by
  provider (`Padlet`, `Survey`, `Google Form`, `Mentimeter`, `SharePoint`,
  `Echo360`, `Canvas media` for a legacy `/media_objects_iframe/`, …), else by
  the host without `www.`. One without an http(s) `src` is dropped.
- **Links are repaired** (`app/src-tauri/src/pages/md/links.rs`), and the link
  crawl reads the same repaired href: a `google.<tld>/url?q=` redirect becomes
  its target, a `mailto:` behind Canvas's origin loses the origin, and an
  unresolved `$CANVAS_…_REFERENCE$` takes the `data-api-endpoint` beside it
  when that names a course file or page (as a relative `/courses/…` path).
  A dead placeholder, or an `<a>` with no href, leaves only its text.
- **Pasted noise is dropped**: PDF.js text layers (`pdf-page-container`,
  `plv-text-layer`, `textLayer`), Ally score badges, Canvas's own
  `data-testid="message_title"` heading, and any image without a `src`.

A sync never resolves an embed: the tag in the page is the only record of it.

## A scrape scaffolds the agent docs

`Engine::scrape` writes the central `agents/AGENTS.md` once per run, then links
each subject's course folder after scraping it (`app/src-tauri/src/agents/mod.rs`),
so a subject enrolled mid-semester is usable by a coding agent from its first
sync. The central copy is written first because the links are relative. A
subject with no folder is skipped, and a failure is a warning, never a failed
sync. `OCULUS-CLI.md` is rendered from the binary's command tree, so the run
cannot write it — see [cli.md](./cli.md).

## Files are fetched once and invalidated by their bytes

- `file-manifest.json` in the data dir maps Canvas file id → (`modified_at`,
  size); when the metadata call reports the same pair and the file is on disk,
  nothing is downloaded.
- Bodies (pages, announcements, tasks, Ed threads and lessons) are always re-fetched;
  the byte-compare in `library::paths::write_course_bytes` decides new/updated/unchanged.
- An `updated` write purges the parse/embed artifacts
  (`library::paths::purge_parse_artifacts`) and the file's pages, so the parse re-runs.
- An `application/octet-stream` upload is judged by extension (`office_ext_of`, `is_video`; allowlists).
- `MAX_FILE_BYTES` (100 MB) skips any larger file except a video, which a sync never downloads.

## A sync lists module videos, and the student downloads them

Lecture videos uploaded as Canvas files (`video/*`, or an untyped
`.mp4/.mov/.m4v/.webm`) are ordinary library files fetched on request; they
have nothing to do with Echo360 lectures.

- `fetch_file` records a video without downloading it, and its module TOC line
  carries the Canvas id and the path it will land at:
  `- [title](../files/<name>.mp4) _(video <id>)_` (`file_toc_line`, parsed by
  `app/src/lib/pipeline/moduleToc.ts`). Locked videos skip like any file.
- Once downloaded, a sync reports it `unchanged` while the manifest pair
  matches. A copy Canvas has since changed is left as it is: only the
  student downloads a video, through the Modules page.
- `canvas_download_video` (`app/src-tauri/src/sync/scrape/videos.rs`) streams it with
  `Canvas::download_to` into a `.part` sibling, renamed on success and removed
  on any failure or cancel, then reports a `scrape-file` like a synced file, so
  the frontend writes its `files` row. Progress is `canvas-video-progress`
  (`canvasFileId`, `percent`, `phase`); `canvas_cancel_video` sets the
  file's flag in `VideoCancels`, polled per chunk.
- `download_to` streams a Canvas URL through keyd (`send_stream`) in 256 KiB
  chunks, and fetches the signed file host a Canvas download redirects to
  directly, with no cookie. A body shorter than its `content-length` is
  refused, since keyd ends a stream early without an error when Canvas drops
  the connection.
- Videos never reach parse or embed: every pipeline gate is a PDF/Office or
  spreadsheet allowlist (`library::paths::doc_pdf_rel`, `library::paths::is_sheet`,
  `isPdfBacked`, `isPipelineFile`, `PDF_BACKED_SQL_LIST`).

## An embedded Echo360 video is resolved and downloaded by its card

An `<echo-video>` tag's card calls into `app/src-tauri/src/sync/scrape/embeds/`;
the launch, Bearer and CDN signing are in
[auth.md](./auth.md#an-embedded-echo360-video-launches-per-link-and-plays-on-a-bearer-token).

- `echo360_embed_info` returns the link's `echo_embeds` row, or resolves it
  (launch, Bearer, player properties), saves the poster as
  `courses/<code>/images/echo-<mediaId>.jpg` (its signed URL expires) and
  writes the row. A link's media never changes, so the row is kept for good.
  Its paths are data-dir-relative, and one whose file is gone from disk comes
  back null (a deleted download is cleared from the row).
- `echo360_embed_download` refuses a `restrict_download` video. A recording
  another link of the subject already downloaded is pointed at, not fetched
  again. Otherwise it takes fresh player properties (signed URLs expire),
  then the highest-`BANDWIDTH` variant of the video master (the lowest
  `sourceIndex` with a video track; other video sources are only counted in
  `extra_sources`) and of the audio-only master.
- A variant is byte ranges over one fMP4 its `#EXT-X-MAP` names, so a download
  is two whole-file GETs and `ffmpeg -c copy -movflags +faststart`, no segment
  stitching. A variant whose segments name any other file is refused as a
  packaging Oculus doesn't support, rather than half-supported.
- Both streams go through `runtime::stream::copy_to_file` (the same loop as
  `Canvas::download_to`) into `tmp/echo-embeds/<linkId>/` in the data dir,
  which is emptied first and removed on every exit. Only the muxed video and
  its captions (`<video>.vtt`) move into `courses/<code>/files/`, named after
  the media name as `.mp4`; a name another file holds gets `-<first 8 of the
  media id>` before the extension.
- The new file is reported as a `scrape-file` with `source_url` set to the
  retrieve URL and no Canvas id, so the frontend writes its `files` row.
  Progress is `echo-embed-progress` (`linkId`, `percent`, `phase`: `downloading`,
  `muxing`, `complete`, `cancelled`, `error`); the percent is of both streams'
  bytes when HEAD gave both sizes, else 80 for the video and 20 for the audio.
  `echo360_embed_cancel` sets the link's flag in `EmbedCancels`, polled per chunk.

## Office documents are stored as themselves plus a derived PDF

Parsing and embedding are PDF-shaped, so `.pptx/.docx/.ppt/.doc` are kept
intact and LibreOffice headless writes `deck.pptx.pdf` beside them
(`office_to_pdf`). The derived PDF never gets a `files` row; the original is
the library row, and `library::paths::doc_pdf_rel` resolves it for every consumer. The
student's own uploads go through the same conversion
(`app/src-tauri/src/library/files/`).

Spreadsheets (`.xlsx/.xlsm/.xls/.ods/.csv`) are kept intact too, but become text,
not a PDF: `marks.xlsx.md` beside them, written as they download
(`crate::pages::sheets`, see
[parsing.md](./parsing.md#spreadsheets-are-converted-to-text-never-parsed)).
One needs a spreadsheet type (or none) *and* a spreadsheet's extension
(`is_sheet_type`): every later gate goes by the extension, and a `.csv` can
arrive labelled `application/vnd.ms-excel`.
A `.csv` counts under a CSV, plain-text, Excel or missing type
(`is_csv_type`).

## Ed content is a custom XML dialect

Ed returns threads and lesson slides as `<document>` XML, parsed with an HTML parser in
`app/src-tauri/src/sources/ed/document.rs`: `<link>` is renamed `edlink` (HTML-void), `<image>`
becomes `<img>`, and `<break/>` swallows following siblings, so every renderer
emits its marker and still recurses. The token is in [auth.md](./auth.md).

- Ed course codes are staff-typed free text ("comp10002 2024s2", "CS 101"),
  matched to Canvas subjects from `/api/user` on the subject token — the words
  up to the first with a digit — then year, and a UniMelb `SM<n>` or a shared
  season name. A tie that leaves is no match: the course's LTI launch, which
  names the course exactly, decides instead, and `ed-courses.json` in the data
  dir keeps what it named (Canvas course id → Ed course id). That answer comes
  first on later syncs while the session is still enrolled in the course, so
  a tie costs one launch, not one per sync (`app/src-tauri/src/sources/ed/courses.rs`).
- The Ed tab is the external tool labelled "Ed Discussion", "Ed" or "EdStem".
- A launch's landing page names Ed's web region (`edstem.org/<region>/courses/…`,
  or a `<region>.edstem.org` host), and the region names the API: the bare
  host's `edstem.org/api` for the home region, `<region>.edstem.org/api` for
  every other (`app/src-tauri/src/sources/ed/region.rs`). The profile keeps both
  (`ed_region`, `ed_api`), and the login token is exchanged on that API only
  once the profile and oculus-keyd have taken it. A launch that lands off
  Ed's hosts — a tab labelled like Ed that runs another tool — fails, and its
  token goes nowhere. A profile with an `ed_api` and no `ed_region` reads the
  region from the API host; with neither, lessons carry no Ed link and a
  workspace fetch fails saying why.

## Ed files open with front matter of JSON literals

Every Ed thread and lesson file starts with `---`-fenced front matter for the
board and Lessons views to read (`app/src-tauri/src/sources/ed/front.rs`; the
frontend's reader is `app/src/lib/markdown/frontmatter.ts`). Each line is
`key: <json>`, or `key:` followed by `  - <json>` items for a non-empty list:
every value is a JSON literal, which is also valid YAML. Missing data is
`null`, never an omitted key, and no other YAML form appears. Web links go to
`https://edstem.org/<region>/…` from the profile's Ed region, and are `null`
(with no `source_url`) while it is unknown.

A thread, `ed/NNNN-<slug>.md` (`NNNN` is Ed's per-course number), keeps its
Markdown below the front matter: `ed_thread_id`, `number`, `type`
(question, post, announcement), `answered` (a question's resolved state, the
same one its `**#46 · question · resolved …**` meta line shows; `null` for
other kinds), `category`, `subcategory`, `subsubcategory`, `category_color`
and `url` (`/courses/<c>/discussion/<t>`, also the file's `source_url`). A
top-level category takes Ed's colour for its position among the course's
`settings.discussion.categories`, from the `/api/user` enrolment; past Ed's 16
colours, or with no list, it is `null` (`app/src-tauri/src/sources/ed/palette.rs`).
When the URL is known, a thread's text carries `[Open in Ed](url)` under its
**By:** line, and a slide's under its title.

The sync reads threads and slides without `?view=1`, so a scrape never counts
as reading them in Ed. Opening a thread or a lesson slide in the app sends the
web app's own view request once per app session (`ed_mark_viewed`,
`app/src-tauri/src/sources/ed/views.rs`; `app/src/lib/ed/views.ts`): a thread
gains the view and a slide is marked seen, never completed.

## Ed Lessons are an index per lesson and a file per slide

The Ed phase resolves the subject's Ed course once and runs lessons, threads or
both (`ed_lessons`, `ed`), so a course costs at most one LTI walk. Lessons run
first: the board re-reads every thread and is what meets Ed's rate limit. Every
Ed request that gets a 429 waits (`Retry-After`, capped at a minute, else 2, 4,
8 … seconds) and is sent again, up to six times (`sources/ed/client.rs`). Lessons go
in sidebar order (module, then `index` with unset ones last, then id), each to
an index, `lessons/<lessonId>-<slug>.md`; the id leads so a renamed lesson
keeps its prefix. Progress reports the phase `ed_lessons`. A lesson's slides
are all fetched before any of its files is written, so a cancelled sync
leaves the last whole lesson.

- The index's front matter is `ed_lesson_id`, `title`, `lesson_type` (Ed's
  lesson `type`), `module`, `order` (the 0-based sidebar position),
  `state`, `available_at`, `due_at`, `locks_at` (Ed's timestamps verbatim),
  `url` (`/courses/<c>/lessons/<l>`) and `slides`: one object per slide in
  `index` order with `id`, `title`, `type`, `status` (Ed's `unseen`, `seen`
  or `completed`), `file`, `pdf`, `url` (`…/slides/<s>`) and `challenge_id`
  (a `code` or `jupyter` slide's Ed challenge, its workspace), paths relative
  to `lessons/`. Its body is the title, module, dates and Ed link, then a
  numbered link to each slide file (a PDF slide's PDF); it holds no slide
  content.
- Each slide is `lessons/<lessonId>-<slug>/<NN>-<slide-slug>.md` (`NN` is its
  position), with front matter `ed_slide_id`, `ed_lesson_id`,
  `ed_challenge_id` (as the index's `challenge_id`), `lesson`, `title`,
  `type`, `status`, `index` (`NN`), `pdf`, `url` and `challenge`, then
  `# <slide title>` and the slide's content. Its `source_url` is the slide's
  page in Ed. A PDF slide has no file (`file` is `null`): its stem's `.md` is
  where the parser writes the PDF's text ([parsing.md](./parsing.md)). The
  index links the PDF, or Ed's copy with a note when the download failed.
- A challenge slide's `challenge` is what its workspace needs, from
  `GET /challenges/<ch>` (one read per challenge slide, before its
  workspace): `{"type", "run_command", "features": {"run", "mark",
  "terminal", "feedback", "confirm_submit"}, "due_at"}`. `type` is Ed's
  (`pyunit`, `code`, `none` …); an empty `run_command` is `null`, and Ed's
  Run then does nothing; a feature Ed leaves out is `null`; `due_at` is the
  lesson's when the challenge has none of its own. The student's standing
  (`score`, `status`, `attempts_remaining`) is left out, since it goes
  stale. When the read fails the slide logs a warning and `challenge` is
  `null`; `ed_challenge_id` comes from the slide itself and the workspace is
  still fetched.
- A slide's assets share its stem: a PDF slide's `<NN>-<slide-slug>.pdf`, a
  challenge's files under `<NN>-<slide-slug>/`. Links are relative to the
  linking file's folder and percent-encoded, a slide file's led by `./`,
  which is the form the viewer resolves.
- Every request is a read without `?view=1`, which would mark the slide seen,
  except a challenge's workspace (`app/src-tauri/src/sources/ed/live.rs`):
  1. `POST /challenges/<ch>/connect` attaches the student's own workspace
     (what opening the slide does) and returns a session ticket.
  2. Ed only hands out a download for a live workspace, so the sync opens
     `wss://sahara.<region>.edstem.org/connect?ticket=…` directly over rustls, with
     the web app's `Origin`, and waits up to 30 s for the `init` frame.
  3. `POST /workspaces/<w>/download {"path": "."}` returns a one-shot ticket,
     and the zip comes from `sahara.<region>.edstem.org/download/<ticket>` with no
     credentials, through the same no-cookie fetch as a Canvas file CDN.
  4. The socket closes once the zip is in.

  Both tickets are credentials: neither goes through keyd, and no URL that
  carries one reaches a log.
- Zip entries are read with `enclosed_name` (an escaping path is refused and
  logged); directories, dotfiles, dot-directories and `__MACOSX` are dropped,
  and a file over 20 MB is skipped with a log line. Each file is stored at
  Ed's own workspace-relative path (no renaming, unlike other library files),
  through the open session's guard (`workspace_session/paths.rs`), so the
  sync and a session name every file alike; a path the guard refuses is
  skipped with a log line. Text files up to 20 KB are
  also shown inline as fenced code. A workspace is fetched every sync, since
  Ed gives it no version; the byte compare keeps unchanged files unchanged.
- A workspace with a sync record, `ed-workspaces/<challenge_id>.json` in the
  data dir (`app/src-tauri/src/sources/ed/workspace_state.rs`), holds local
  work, so the sync reconciles it with Ed instead of overwriting it
  ([below](#reconcile-brings-a-recorded-workspace-in-step-with-ed)): it
  connects and holds the workspace live itself, as for a download. It
  leaves the folder alone, with an `info` line, while the challenge is open
  in the app (its session reconciles it) or when the record names another
  folder (the lesson was renamed). A conflict is a `warning` line; a file
  reconcile wrote is reported `updated`. The record is
  `{"workspace_id", "slide_dir", "base_commit", "files", "submissions"}`,
  `files` mapping each path in the workspace to the SHA-256 (hex) of the
  bytes last synced. The slide's Files list is then those paths, read from
  the slide's folder on disk (one missing there is left out). A record that
  cannot be read still counts, as an empty one.
- A PDF slide's `file_url` names its content, so `file-manifest.json` keeps it
  under `ed-slide-<id>`, and a match with the file on disk skips the
  download. The PDF is an ordinary library file: parsed and embedded.
- A quiz lists each question's answers as `- [ ]` in Ed's order. Ed sends a
  question's `solution` only once the lesson's solutions are released, and
  only then does the file get a collapsed `<details>` naming the answer and
  the explanation.
- Slide bodies are Ed's `<document>` XML ([above](#ed-content-is-a-custom-xml-dialect)), except `html` slides,
  which go through `app/src-tauri/src/pages/md/` as they are. A slide's own
  headings are pushed two levels down (h1 → h3, capped at h6), below the
  file's `# <slide title>` and level with a quiz's `### Question` headings. Images stay remote, as in threads. An unknown slide type renders a one-line note.
- A lesson Ed refuses (a password-gated quiz answers 403) is a warning and the
  course carries on; a slide whose PDF, questions or workspace fails is a
  warning and a note in the slide's file (in the index, for a PDF).

## An open challenge workspace is a live session

Opening a `code` slide's workspace in the app starts a session
(`app/src-tauri/src/sources/ed/workspace_session/`): one per challenge, held in
the managed `EdWorkspaces` state. The library copy under the slide's folder is
the working copy; Ed gets each save as an upload.

- `ed_workspace_open {challengeId}` finds the slide by the `ed_challenge_id`
  in its front matter (the folder the sync record names first, else every
  `courses/*/lessons/*/*.md`), lists the folder's files, reads the
  `challenge` snapshot for `runCommand`, `features` and `dueAt`, starts the
  socket thread and answers at once with status `connecting`. A second open
  answers for the session already running; `ed_workspace_close` ends it and
  is a no-op when none is open.
- The thread owns the socket (`live.rs`'s TLS and handshake, shared with the
  sync): `POST /challenges/<ch>/connect` for a ticket, `wss://sahara.<region>…/connect`,
  `init`, then reads of at most 200 ms, between which it sends what the
  commands queued and a `keep_alive` every 270 s. When the socket drops it
  goes `offline` with the reason and connects again with a fresh ticket after
  2, 4, 8 … seconds, at most a minute; nothing times out a live session on
  top of Ed's server.
- The first connect of a challenge with no sync record downloads the tree
  (`"."` zip, unpacked as the sync does), writes it into the slide's folder,
  and saves the record: `base_commit` is `init.last_commits[0]`, each file's
  hash is of Ed's bytes. A save or delete made before that (offline, or while
  connecting) is kept over Ed's copy, then uploaded or removed. Any other
  file in the folder that Ed lacks is new here and is uploaded too, except
  an earlier sync's rewritten name for one of Ed's files (`my_file.py` for
  `my file.py`), which is deleted (logged). Every later connect reconciles
  ([below](#reconcile-brings-a-recorded-workspace-in-step-with-ed)) before
  the session goes online.
- Every path is workspace-relative and goes through one guard
  (`paths::resolve`): no empty, absolute, `.`/`..`, hidden (dot), `\`, `:` or
  NUL component, and nothing a symlink takes outside the folder. The file list
  leaves out hidden files and symlinks.
- `ed_workspace_save {challengeId, path, text}` writes the file atomically,
  then, while online, gets an upload ticket (`POST /workspaces/<w>/upload
  {wid, path: "."}`) and posts the file as one multipart `file` part named by
  its path to `sahara…/upload/<ticket>`. Every upload is a commit: the answer
  is `{hash}`, which becomes `base_commit`, and the file's hash goes into the
  record. The answer is `{saved: "ed", hash}`; offline it is `{saved:
  "local", hash: null}`; a failed upload is an error, with the file already
  written. Saves and deletes of one challenge run one at a time in the order
  they were asked (`queue.rs`).
- `ed_workspace_delete` removes the file or folder, and while online sends
  `fsop {type: "remove", param1: <path>, param3: "file"|"folder"}` (the
  server ignores a remove without `param3`). The record keeps the file until
  a reconcile finds it gone from both sides, so a remove lost with the socket
  is sent again; an offline delete is found the same way.
  `ed_workspace_read` returns the library copy as UTF-8 (lossy).
- One event, `ed-workspace`, to the main window: `{challengeId, kind}` with
  `status {state: connecting|online|offline, reason}`, `commit {hash, ours}`,
  `files {files}` (after the first download writes, after each reconcile,
  and when a save or delete changes the list), `conflict {path, serverPath}`
  (one per file in conflict, after each reconcile), `error {message}`
  (Ed's `display_error`, or a pull that failed), and the terminal and Run
  kinds below.
  Each upload makes two `new_commit` frames: a snapshot commit whose hash
  the upload never returns, then its own, which can beat the HTTP answer.
  So a frame is ours when its hash is one an upload of ours answered or when
  it arrives while an upload of ours is in flight (`commits.rs`); another
  client's commit hidden that way is caught by the next reconcile.
  `ours: false` is another client, such as Ed's web app. The `list_reply` the
  server sends after an upload is ignored.
- Frames are `{type, data}` JSON (`frames.rs`); kinds the workspace does not
  use (live editing, cursors, chat) are ignored.
- Tickets are held in a `Ticket` whose `Debug` hides them, and the workspace
  server's errors name a status or a transport kind, never the URL. `init`'s
  `auth` is never read into a field. None of the three reaches a log, an
  event, a result or an error.
- Terminals (`terminal.rs`) run on the session socket. `ed_terminal_open
  {cols, rows}` sends `program_open {id: "term-<u32>", command: "bash
  --login", pty_size, start_if_stopped: true, persist: true}` and answers
  `{id, resumed}`. A persisting terminal outlives the socket: `init`'s
  `programs` and `program_state` list the ones the server still runs, and
  opening one of those ids again attaches to it instead of starting a shell
  (what Ed's web app does after a reload). The pane gets a listed terminal it
  does not hold yet first (`resumed: true`), else a new id. Each connect
  re-attaches every terminal the pane holds at its last size, and reports one
  the server no longer lists as `termState exited`, so the pane opens a
  terminal once per session. `ed_terminal_input {id, data}` sends
  `program_data` with the UTF-8 text in base64, `ed_terminal_resize` sends
  `program_set_pty`, `ed_terminal_kill` sends `program_signal {signal:
  "kill"}`. Incoming `program_data` becomes `term {id, data}` (base64 passed
  through), `program_init`/`program_update`/`program_close` become
  `termState {id, state: running|exited}`; programs whose id is not
  `term-…` are not the pane's. Offline, each command is an error.
- Run and Mark take the workspace's latest commit (`snapshot.rs`), after the
  saves asked before them have landed: the record's `base_commit`, which
  every upload moves. A workspace with none (Ed named no head and nothing
  was uploaded) sends `commit {id}` on the session socket and waits for the
  `new_commit` whose `context.id` echoes it (announced as `commit {ours:
  true}`); going offline ends the wait with an error.
- Run (`run/`): `ed_workspace_run {cols, rows}` needs a `run_command` and
  the session online, stops the challenge's run in progress, gets a ticket
  (`POST /challenges/<ch>/run {pty_size, hash, command: run_command,
  dump_files: true}`), opens `wss://sahara.<region>…/run?ticket=` with
  `live.rs`'s TLS and handshake, and hands it to a thread of its own; it
  answers `{runId}`. The thread streams `run_frame` stdout/stderr data as
  `run {runId, data}` (base64), writes `status_built`'s output and, when
  `ok` is false, ends with "The build failed"; `run_exit {exit_status: {type,
  value}}` becomes `runExit {runId, code | signal | message}` (`normal` →
  code, `signal` → signal, `killed`/`exec` → words); close code 1013 is "No
  resources available". Every run ends with exactly one `runExit`.
  `ed_run_input` sends `run_frame {pid: 0, type: "stdin", data}`,
  `ed_run_resize` `run_set_size`, `ed_run_stop` closes the socket ("Stopped");
  closing the workspace stops its run.
- Mark (`mark/`): `ed_workspace_mark` (`POST /challenges/<ch>/mark {hash}`)
  is a real submission. Only the workspace's Mark button calls it, after the
  pane's confirm dialogs; a second Mark while one is in flight is refused,
  nothing retries it, and no sync, reconnect, agent or CLI reaches it. Its id
  is appended to the record's `submissions` before the answer is shaped.
  The answer is the pane's `Submission`: camelCase, a testcase's
  `run.output` as `output`, and `diffs`, the lines of every `check_diff`
  check's `value.diff` in order. Ed writes a line as `[[sign], [sign|"=",
  text]…]`; each element becomes a `[sign, text]` pair, so a line starts
  with its sign (`-` observed, `+` expected) and empty text, and `["mn"]`
  (missing newline) keeps its sign. `ed_workspace_submissions` reads the
  record's ids newest first (`GET /challenges/submissions/<id>`, 300 ms
  apart, since Ed 429s bursts); Ed lists none for a student.
  `ed_challenge_standing` reads `GET /challenges/<ch>` live for `{score,
  status, attemptsRemaining, dueAt}`, `dueAt` falling back to the slide's
  front matter. Neither read needs the workspace open.

## Reconcile brings a recorded workspace in step with Ed

`app/src-tauri/src/sources/ed/workspace_session/reconcile/` compares, per
file, its bytes in the slide's folder, the record's hash (the base) and Ed's
bytes (`download "."`), and acts on the decision table in `table.rs`:

| Here | Ed | Action |
| --- | --- | --- |
| = base | changed | take Ed's |
| changed | = base | upload |
| changed | changed, same bytes | record |
| changed | changed, different | **conflict** |
| new | absent | upload |
| absent | new | write here |
| deleted | = base | `fsop remove` on Ed |
| = base | deleted | delete here |
| changed | deleted | upload |
| deleted | changed | write here |

- A conflict keeps the local file and writes Ed's beside it as
  `<name>.ed-server.<ext>` (`server_copy.rs`: before the last extension,
  `x.tar.gz` → `x.tar.ed-server.gz`; after a name with none,
  `Makefile.ed-server`). The record keeps the old base, so each reconcile
  calls it a conflict again and refreshes the copy; while the copy exists
  reconcile never uploads the file. Names of that shape are reserved: never
  uploaded by a save, removed on Ed by a delete, recorded, or taken from Ed.
- `ed_workspace_resolve {challengeId, path, keep: "mine" | "ed"}` settles one,
  in the save queue, online or not. `mine` records Ed's version as the base
  (so the local edit is the only change), deletes the copy and, while
  online, uploads the file. `ed` writes the copy over the file, drops it from
  the record (recorded once Ed is found to agree, a conflict again if Ed has
  moved on) and deletes the copy.
- `init.last_commits[0]` equal to `base_commit` means Ed has not changed:
  nothing is downloaded and only local changes go up. A pull that was owed
  (below) turns that off for the next connect.
- After a reconcile the record holds the new hashes and `base_commit` is the
  last upload's commit, else Ed's head. Every step leaves the sides so a
  second run decides the same or just records, so one cut short (an upload
  fails, the socket drops) is finished by the next; the record is saved
  only when the whole run succeeds.
- A file the zip leaves out (over 20 MB, a name the guard refuses) is left
  alone, never taken for deleted. A folder missing from the library is an
  error, never a delete of everything.
- When: every connect of a session with a record (after the lesson sync
  lets go of the challenge, if it holds it), every lesson sync of a
  workspace not open in the app, and while a session is live, each
  `new_commit` from another client. That pull runs on a worker thread in the
  save queue, so it lands between saves and never stalls the socket; commits
  arriving during a pull make one more pull. A pull that fails or finds the
  session offline is owed to the next connect, which then downloads even at
  the base commit. The app process keeps the lesson sync and its sessions
  off the same challenge (`registry.rs`).

## Echo360 is an LTI launch with up to two streams

The app's Echo360 commands run synchronous HTTP, file I/O and ffmpeg on
blocking workers, leaving async runtime threads available for other commands.
Workers share the course session cache and source-qualified cancellation flags.

- Canvas mints an OAuth-signed form on the course's external-tool page, which
  `echo360::connect` fetches through keyd; POSTing it to Echo360 (directly,
  with Echo360's own in-process cookies) creates the session, and the
  CloudFront cookies that come back are what the media CDN accepts.
- The tool is found per course: tabs labelled Echo360, "lecture recording" or
  "lecture capture" first, then the profile's `echo360_tool` (UniMelb's, since
  its tab can be hidden). The first page with an `action="https://echo360…"`
  form whose launch opens a `/section/…` is the session; a launch that opens
  none (a tab can launch Echo360 elsewhere) gives the next candidate its turn,
  and when every launch fails their errors surface together
  (`echo360::auth::first_launch`). A course with no form at all answers
  `echo360::NO_TOOL`, which `echo360_sync_lectures` turns into no lectures
  rather than an error. A tool page that answers 401, 403 or 404 is skipped;
  any other failure surfaces.
- The launch lands on the university's Echo360 region (`echo360.org`,
  `echo360.net.au`, …); `Session::base` keeps it for the syllabus,
  transcripts and downloads.
- A capture is one media id with the Presenter screen at `hd1.mp4` and the room
  camera at `hd2.mp4`. They land as `source1.mp4`/`source2.mp4`, trimmed
  identically so the player runs them off one clock. The trim is the
  university's intro bumper, `echo360_lead_in_secs` in the profile (UniMelb's
  is 14 s); with none the download is the file and ffmpeg is not run. VTT
  captions are aligned to the trimmed timeline.
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
`db::store::upsert_pages` so `oculus grep` never waits on the vector index, and
queues its embedding ([retrieval.md](./retrieval.md)). Hitting an
already-parsed file with no page rows folds its `.pages.json` in.

## Gotchas

- An office file whose conversion fails (no LibreOffice) is still stored but kept out of `file-manifest.json`, so the next sync retries the conversion. A derived PDF from older bytes is deleted with its parse artifacts, and the row gets a `document` parse failure (`parse::CONVERSION_FAILED`), not an endless "waiting to parse".
- A delete cancels an in-flight download of the same lecture first, or the writer recreates the file just after it is removed.
- A video download merges its entry into `file-manifest.json` as it is on disk (`record_manifest`), or it would drop what a sync saved during the download.
- Don't add a parse deadline or worker pool here — concurrency and timeouts belong to the parse backend ([parsing.md](./parsing.md)).
