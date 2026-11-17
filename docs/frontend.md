# Frontend

React 19 + Vite + Tailwind v4 in `app/src/`: a strip of tabs, each a page with
an optional side panel beside it and a router per pane, inside a Notion-style
shell. This page is the data side — backend events, settings, parse state and
library files; the rest of the frontend has its own pages:

| Page | What it covers |
| --- | --- |
| [shell.md](./shell.md) | Per-pane routers, tabs, the side panel, window shortcuts, ⌘F, search |
| [ui.md](./ui.md) | The design rules, and the WebKit and CSS traps |
| [studio-page.md](./studio-page.md) | The Studio page: New video, a video's thread and player, the Videos grid |
| [viewers.md](./viewers.md) | Markdown, PDFs, an Ed lesson's slides, the media player (lectures and library videos) and the in-app browser with downloads |
| [editor.md](./editor.md) | The note editor: sessions, find, versions, code, tables, pictures, mentions, `NoteField` |
| [editor-maths.md](./editor-maths.md) | Maths in the note editor: the visual field, the toolbox, shorthands |

## Where

| Piece | Location |
| --- | --- |
| Entry, the event shim, the last-resort error overlay | `app/src/main.tsx`, `app/src/lib/platform/tauriEvents.ts` |
| The device-sharing companion page a paired browser opens: its entry, header and build ([below](#the-companion-page-is-a-second-entry)) | `app/companion.html`, `app/src/companion/`, `app/vite.companion.config.ts` |
| The companion's transport: share-link credentials, HTTP and socket client, stored drafts | `app/src/lib/companion/` |
| The first-run gate: onboarding or the shell ([onboarding.md](./onboarding.md)) | `app/src/App.tsx`, `app/src/hooks/shell/useOnboardingGate.ts` |
| Backend events → SQLite and stores | `app/src/hooks/backend/useBackendEvents.ts`, `app/src/hooks/backend/useEvents.ts`, `app/src/lib/pipeline/parseStatusWriter.ts`, `app/src/lib/db/connection.ts` |
| Own files: uploads, notes and drawings, their rows and reconcilers | `app/src/lib/files/uploads.ts`, `app/src/lib/notes/documents.ts`, `app/src/lib/canvas/files.ts`, `app/src/lib/drive/reconcile.ts`, `app/src-tauri/src/library/files/commands.rs`, `app/src-tauri/src/library/drive/ops/reconcile.rs` |
| File category icons and recency marks | `app/src/lib/files/fileTypes.ts`, `app/src/components/files/FileRecency.tsx` |
| Path-keyed state following Drive's moves ([below](#path-keyed-state-follows-drive)) | `app/src/lib/shell/pathFollow.ts`, `app/src/lib/shell/followLibrary.ts`, `app/src/stores/shell/tabs/follow.ts` |
| Drive's folder browser and folder tree ([below](#drive-is-one-folder-browser-over-the-disk)); `list_folder` | `app/src/components/drive/`, `app/src/hooks/drive/`, `app/src/lib/drive/`, `app/src-tauri/src/library/drive/` |
| Ed board, keyword search and filters | `app/src/pages/subject/content/DiscussionPage.tsx`, `app/src/pages/subject/content/discussion/`, `app/src/lib/ed/threads.ts` |
| Ed lessons under their modules; the lesson index reader | `app/src/pages/subject/content/LessonsPage.tsx`, `app/src/pages/subject/content/lessons/`, `app/src/lib/ed/lessons.ts` |
| Weekly lecture groups and collapsible Canvas modules | `app/src/pages/subject/content/LecturesPage.tsx`, `app/src/components/lectures/LectureRow.tsx`, `app/src/pages/subject/content/ModulesPage.tsx`, `app/src/pages/subject/content/modules/` |
| Tasks section (`/projects`, `/tasks`) | `app/src/components/projects/`, `app/src/pages/work/TasksPage.tsx`, `app/src/lib/planning/projects/`, `app/src/stores/planning/projectsStore.ts` |
| Revision: cheat sheet, review queue, quizzes and artifacts ([revision.md](./revision.md)) | `app/src/layouts/RevisionLayout.tsx`, `app/src/pages/revision/`, `app/src/lib/revision/`, `app/src/stores/revision/revisionStore.ts` |
| Chat | `app/src/pages/tools/ChatPage.tsx`, `app/src/components/harness/`, `app/src/stores/chat/harnessStore.ts` |
| Settings → Parsing, Embeddings, Transcription: parser, embedding, index run, transcription (the language, the engine order and switches, a dialog per engine: Groq key, local Whisper models, on-device speech) | `app/src/pages/settings/library/ParsingPage.tsx`, `app/src/components/settings/library/ParserSection.tsx`, `app/src/components/settings/library/EmbeddingSection.tsx`, `app/src/stores/sync/indexStore.ts`, `app/src/components/settings/transcription/TranscriptionSection.tsx`, `app/src/components/settings/transcription/GroqDialog.tsx`, `app/src/components/settings/transcription/LocalWhisper.tsx`, `app/src/components/settings/transcription/OnDeviceSpeech.tsx` |
| Settings → Studio (Narration, Pronunciation, Word timing): Studio's narration engine (Kokoro with its pack download, voice and sample, or a TTS server's URL, model with List models and voice), the speed, and the aligner model download ([studio.md](./studio.md)) | `app/src/pages/settings/studio/`, `app/src/components/settings/voice/`, `app/src/lib/studio/` |
| Studio rendering: the `studio_render` wrappers, the `studio-render-progress` listener and the render store behind the Studio page ([studio-page.md](./studio-page.md)) | `app/src/lib/studio/render.ts`, `app/src/hooks/backend/useStudioEvents.ts`, `app/src/stores/studio/renderStore.ts` |
| Settings → Appearance: the mode, a card grid per mode, the accent, the window frame (macOS) and the code font ([ui.md](./ui.md)) | `app/src/pages/settings/AppearancePage.tsx`, `app/src/components/settings/appearance/`, `app/src/lib/ui/themes/` |
| Settings → Device sharing: what sharing is, live shares with Stop and Stop all ([device-sharing.md](./device-sharing.md#desktop-ui)) | `app/src/pages/settings/integrations/DeviceSharingPage.tsx`, `app/src/components/settings/sharing/` |
| Parser and embedding engine selection UI | `app/src/components/settings/shared/EngineSelect.tsx` |
| Parse state, pipeline ledger, recovery sweep | `app/src/lib/pipeline/parseState.ts`, `app/src/stores/sync/parseStore.ts`, `app/src/stores/sync/pipelineStore.ts`, `app/src/hooks/sync/useQualitySweep.ts` |
| Scraped document metadata loaders | `app/src/hooks/data/useCourseFileData.ts`, `app/src/hooks/data/useModuleTocs.ts` |
| Subject Overview data, section derivations and public item types | `app/src/hooks/data/overview/` |
| Pure file titles and routes, shared with file opening | `app/src/lib/files/labels.ts` |
| Spotify ([music.md](./music.md)): the `music-state` snapshot type and the command wrappers; the snapshot mirror and Settings → Music's switch; the listener mounted in `AppLayout` | `app/src/lib/music/index.ts`, `app/src/stores/music/musicStore.ts`, `app/src/stores/music/musicPrefsStore.ts`, `app/src/hooks/music/useMusicState.ts` |
| Module videos and page embeds downloaded on demand; YouTube/Vimeo metadata | `app/src/pages/subject/content/ModulesPage.tsx`, `app/src/stores/lectures/videoDownloadStore.ts`, `app/src/stores/lectures/embedDownloadStore.ts`, `app/src/stores/lectures/webVideoStore.ts`, `app/src/lib/files/scrapedRow.ts` |

## Windows transcription settings expose Groq only

`TranscriptionSection` keeps the shared language and Groq key controls on
Windows, hides the local Whisper and on-device speech dialogs, and skips
their model and OS-status reads. The stored settings retain every engine's
preferences; Windows only renders the Groq row. Backend selection follows
the same platform limit ([viewers.md](./viewers.md#videos-without-captions-are-transcribed)).
Credential descriptions and parse errors say credential store across platforms.

## Source is grouped by feature, and tests mirror it

- `app/src/{lib,hooks,stores,components,pages}/` hold grouped folders
  (`lib/pipeline/`, `hooks/sync/`, `stores/shell/`, `components/projects/board/`),
  never a flat pile; a file's folder names the feature it serves.
- A file that outgrows itself keeps its import specifier: the entry stays
  (`EmbeddingSection.tsx`, or `DocumentEditor/index.tsx`) and the extracted
  pieces sit in a folder next to it (`embedding/`). Split rather than
  adding banner comments to a long file.
- Tests live under `app/tests/` at the same path as the file they cover and
  import it through the `@/` alias (`tests/lib/ui/scrollFade.test.ts`).

## The companion page is a second entry

A device paired with Share on a canvas ([device-sharing.md](./device-sharing.md))
loads a page of its own, not the desktop app: `app/companion.html` and
`app/src/companion/`, built by `vite.companion.config.ts` (`bun run
companion:build`, the last step of `bun run build`, after the desktop bundle
so the desktop build's `emptyOutDir` cannot delete it) into
`app/dist/companion/`. The entry is renamed `index.html`, every URL is under
`/c/`, and the canvas WebAssembly and the Inter and Manrope fonts are hashed
files in `dist/companion/assets/` (`assetsInlineLimit: 0`, so nothing becomes a
`data:` URL); the desktop's `public/` files are left out.

- **No Tauri.** The page renders the real `CanvasSurface`, so the canvas's
  host services come from `CanvasPlatformContext`
  ([drawing-canvas.md](./drawing-canvas.md#tools-and-exports-share-the-same-geometry)),
  whose default is the browser's. `app/tests/repo/companionGraph.test.ts` walks
  the page's imports, lazy ones included, and fails on `@tauri-apps/*`,
  `@/lib/db` or the SQL plugin; it also checks the desktop entry never imports
  the page or its client. The build swaps `@/lib/menu/editRouting` for
  `src/companion/editRouting.ts`: a browser has no Edit menu taking ⌘Z first,
  and the real module would bring the maths engine in.
- **The link.** `lib/companion/link.ts` reads `#<shareId>.<secret>`, strips the
  fragment with `history.replaceState` before the first render and keeps it in
  memory and this tab's sessionStorage, for a reload of the same tab. The secret
  goes only in the `Authorization` header and the socket's hello; `redactSecret`
  scrubs any text the page shows.
- **The client.** `ShareClient` (`lib/companion/shareClient.ts`) takes its page
  through `ShareClientEnv` (fetch, socket factory, clock, random, ids, storage,
  lifecycle events), so its tests run it against an in-process server
  (`tests/lib/companion/fakeShareServer.ts`) that applies commits with
  `collaboration.ts`. The page builds that env with `browserShareEnv(page)`,
  `window` by default; the end-to-end check passes a stand-in over bun's
  globals ([device-sharing.md](./device-sharing.md#end-to-end-check)). The local drawing is the shared one with this device's
  edits on top; one commit is in flight, its transaction the difference from
  the last confirmed drawing, and a retry resends the same `(clientId, seq,
  opId)`. Remote changes come in revision order (inline `change`, or `/changes`
  after a `revision` announcement, or a snapshot when the log answers `gone`)
  and the local edits are rebased onto them. Its status is one of `connecting`,
  `connected`, `saving`, `reconnecting`, `offline-unsaved`, `conflict`,
  `paused` or `ended`; the React side reads it with `useSyncExternalStore`
  (`src/companion/useShareClient.ts`).
- **Reconnecting.** A failure retries after 0.5 s, doubling to 8 s, up to a
  quarter longer at random; `online`, a return to visible and `pageshow` try at
  once. No frame for 45 s means the socket is dead, so the server must send
  something (a `ping` frame, answered `pong`) more often than that: a browser
  never sees WebSocket-level pings. A commit times out after 20 s plus 10 s per
  MB and is then resent unchanged.
- **Drafts.** The browser uses IndexedDB for complete recovery records, including
  embedded page images, while the desktop uses disk. The operation identity,
  replay rules and stable notice layout are described in
  [device-sharing.md](./device-sharing.md#recovery-survives-a-browser-reload-without-moving-the-drawing).
  A conflict keeps the drawing until Use shared canvas; Download draft saves
  it as a `.canvas` file.

## Subject lists keep content flat and distinguish file types by colour

Subject file and content rows use flat divided lists without a surrounding
card. Canvas module items use a single list level. Module
rows stay individually collapsible; the newest TOC for each Canvas position
wins when an old position path remains, and exact repeated TOCs appear once.
Lecture recordings use the shared
[term teaching weeks](./calendar.md#teaching-weeks-are-shared-across-a-term),
presented by `app/src/lib/lectures/weeks.ts`. Rows outside the subject's
timetable are grey and remain playable; undated recordings have their own
group. File glyphs use
the `reference-*` tokens: PDFs red, Office documents blue, notes purple,
spreadsheets teal, and lecture videos pink. Search remains in the subject
sidebar rather than being duplicated above each list, except the Ed board's
thread-specific search.

The Ed board uses aligned thread, category, answer status, posted date and
activity columns, in rows the height of the lecture list's. Numbers belong to
the title (`#46: Question 4 A2`); titles come from the scraped heading, with the
filename as a fallback. Narrow panes fold category and answer status into the
title cell and hide activity. Each value draws one glyph wherever it appears, in
the row and in its filter's options (`discussion/ThreadIcons.tsx`): a Phosphor
icon per thread type, a filled tick for answered and a hollow circle for
unanswered questions, and Ed's own colour square before a category (muted when
the colour is unknown). A row with an Ed link shows *Open in Ed* on hover
(⌘ for a new tab); a thread's file page offers the same in its header, decided by the `source_url` host (`isEdUrl`, `app/src/lib/ed/links.ts`).

`parseEdThread` reads a thread's JSON front matter (number, type, answered,
category path, category colour, URL) and falls back to the
`**#46 · question · …**` meta line for files scraped before it; the parser is
`app/src/lib/markdown/frontmatter.ts`, which only takes a block whose every line
is a key with a JSON value, so a page opening with a `---` rule stays whole.
`FileMarkdown` strips the front matter before rendering a library file.

Its keyword search reads the whole synced Markdown thread, including replies;
every whitespace-separated keyword must match, regardless of case or order.
While searching, keywords are marked in each title, and a line under it shows
the post or reply prose where one first occurs (`matchSnippet`), marked too.
Type, question answer status and exact category-path filters combine, with
multiple selections within each filter matching any selected value. Answer
status filters apply only to questions; older scrapes without a status remain
unknown. Search runs over the metadata loader's cached reads, so typing makes
no network calls or file reads.

A file's recency (`FileRecency`, in Drive's details panel; `RecencyDot`, the
dot alone, on Drive's rows and Recent) shows an indigo dot and *NEW* while
the file has never been opened (new, or rewritten by a sync before then), the
same *NEW* in amber once it changed since it was last opened, and otherwise
its last-opened time.

## Drive is one folder browser over the disk

`DriveBrowser({ root, folder, folderHref, rootLabel?, leading? })`
(`app/src/components/drive/`) shows one folder of the library. `root` is
`""` for every subject (the Drive rail section) or `courses/<code>` for one
(a subject's Files tab); `folder` is the folder on screen at or below it and
`folderHref` the route that shows a folder, so the route's path is the only
state. `DriveTree({ current, folderHref })` is the nav column's tree beside
it: subjects under Subjects, the rest folded under Past subjects, each opening
in place to its folders, read when first opened; which rows are open is a
`useStoredSet`. With `onPick` and a subject `root` the same tree is the folder
picker of Copy to my files…, Move to… and the browser's Save dialog. Both
are routed: Drive's rail section at `/drive/*` and the Files tab at
`/subjects/<id>/files/*`, whose splats are the folder
([shell.md](./shell.md#each-pane-has-its-own-router-and-the-path-is-its-only-state)).

- **`list_folder({ rel, recursive? })` reads the disk**
  (`app/src-tauri/src/library/drive/`), so empty folders show and a file
  without a row still lists. It returns the folder's `zone` and `writable`
  (a file or folder made there would be the student's), its `folders`
  (`relativePath`, `name`, `zone`, `writable`, `editable`, `childCount`,
  `modified`), `files` (`relativePath`, `name`, `zone`, `editable`, `size`,
  `modified` in Unix ms) and `recordings`. `rel` must be `""` (whose folders are
  the subject folders) or inside a subject, with no dot-name and no symlink on
  the way; anything else, app state included, is refused. Left out of every
  listing: dot-names, symlinks, what `layout::zone_of` hides, and each file's
  parse artifacts and converted PDF (`paths::parse_artifacts`, the same rule
  the purge deletes by, so a student's own `x.md` beside an unparsed `x.pdf`
  stays). `recursive` lists every folder and file below `rel`, flat (at the
  root, every subject's).
- **A recording lists as its lecture.** The command reads the subject's
  `lectures` rows and asks `layout::recording_dir` for each one's folder; a
  lecture whose folder sits in the listed folder is a `recordings` entry
  (`lectureId`, `subjectId`, `title`, `date`), listed whether or not its folder
  exists yet, and opens the lecture page. The folder itself is not listed.
- **Rows are joined in the frontend**, as every file list reads them
  (`getFilesForSubject`, `useDriveRows`), by `relative_path`. A file's name is
  its row's (`fileTitle`), its parse state `ParseStateBadge` and
  `useFileParseState` (the parse store over `files.parse_status`), its new dot
  `RecencyDot`. Listings and rows re-read on `DRIVE_CHANGED_EVENT` and the
  upload, note, canvas and scrape events, once they stop for 300 ms. The PDFs
  on screen have their parse state read back from disk
  (`useReconcileParseStatus`), for a parse an earlier session finished, and a
  folder in a subject asks for its own files' rows (below).
- **The view.** The header holds the trail, a search scoped to the folder
  and everything below it, and New (folder, note, canvas, notebook, upload
  files or a folder), drawn only where the folder is `writable`. The Type,
  From and Modified pills, like the search, switch to the flat recursive list,
  narrowed by `filterListing` (`lib/drive/filters.ts`); a search alone keeps
  matching folders. Grid or list is remembered per root (`useStoredState`,
  grid at the Drive root, list in a subject by default). The grid draws folder
  cards and picture cards; the list is `GridTable` with Name, Kind (Status:
  the parse state, in a synced folder), Modified (Synced) and Size, down to
  Name and the day in a pane under 560px. One selected file gets the details
  panel from 900px. Widths are the browser's own (`useElementWidth`). A
  synced folder says once, above its entries, that it is read-only.
- **Pictures** come from what already draws each kind: a PDF's, or an Office
  file's converted PDF's, first page through the viewer's renderer
  (`openPdf`, `renderPage`); a notebook's first page from the pictures its
  editor writes for chat agents (`drive_notebook_cover`, PNG bytes from
  `canvas-pages/<file id>/`); a recording's frame from `lecture_thumbnail`; an
  image itself. A canvas, a video file and anything else show their glyph.
- **Selection is Finder's** (`lib/drive/selection.ts`): a click picks one,
  ⌘-click toggles, ⇧-click takes the run from the anchor, ⌘A picks all,
  Escape clears, a click in empty space clears. Double-click or Enter opens
  (`openItem`: a folder here, a recording's lecture page, a file as
  `openFileSmart` does). Delete or ⌘⌫ on the student's own entries asks first,
  listing every file the delete removes (`delete_preview`). ⇧⌘N makes a
  folder. A new folder, and Rename, edit the name in place.
- **The right-click menu** (`components/ui/context-menu.tsx`) acts on the
  selection when the entry is in it: Open, Open in new tab, Copy to my files…
  for synced entries, Show in Finder, then Rename, Move to… and Delete, greyed
  on synced entries with "Course files are read-only" under them.
- **Drag and drop.** Finder files drop through the webview's native events
  on a folder card, a list row, a tree row or empty space (the folder on
  screen), and go to `import_uploads` with that folder as `targetDir`. Entries
  drag with HTML5 events, so a drag reaches the tree in the nav column;
  `dragstart` puts the paths on the transfer (`DRIVE_DRAG_TYPE`) or WebKit
  cancels it. Between the student's folders a drag moves (`move_entries`), out
  of a synced folder it copies (`copy_to_own`), and each copy's parse is
  kicked. A folder the student can't add to fades with "Can't add here" while
  the drag is over it and takes nothing (`planDrop`, `useDriveDrop`).
- **Failures are a line above the entries** (`DRIVE_PROBLEM_EVENT`), never a
  toast.

The folder commands Drive calls are typed in `app/src/lib/drive/commands.ts`
and live in `app/src-tauri/src/library/drive/ops/`. All take data-dir-relative
paths and touch only the student's own files (`layout::own_path_ok`); a
target folder is a subject's root or an existing folder of the student's:

| Command | Arguments | Returns |
| --- | --- | --- |
| `create_folder` | `rel` | the new folder's path |
| `rename_entry` | `rel`, `newName` | the new path |
| `move_entries` | `rels`, `targetDir` | the new paths, in order |
| `delete_entries` | `rels` | nothing |
| `delete_preview` | `rels` | every file a delete would remove |
| `copy_to_own` | `rels`, `targetDir` | the copies' paths, in order |
| `import_uploads` | `subjectCode`, `paths`, optional `targetDir` | one outcome per file |
| `create_document`, `create_canvas` | as before, optional `targetDir` | the new file |

- **Names.** A name may not be empty, start with a dot, hold `/`, `\`, `:`,
  NUL or a control character, or start or end with a space; `Course` and
  `.oculus` are refused at a subject's root. A taken name is `exists` for
  `create_folder`, `rename_entry` and `move_entries` (nothing is
  overwritten); imports and copies step aside to `name (2).ext`, and an
  import of identical bytes reuses its name. A picked file or folder keeps
  its name, with what the rules refuse replaced (`names::entry_name`).
- **Rows follow by id.** A move or rename updates the `files` row in place:
  path, name, `file_type` on a rename, `category` (`layout::category_of`, so
  `.md` to `.txt` stops being a note) and, across subjects, `subject_id`; a
  folder's rows all follow. A parsed file takes its artifacts with it and
  stays parsed and embedded ([architecture.md](./architecture.md#one-module-owns-every-path-shape-in-the-library)),
  a note its pictures ([editor.md](./editor.md#pictures-mentions-and-citations-resolve-to-library-files)).
  A delete removes rows with their pages, versions and task links, then the
  files and artifacts; there is no trash. `delete_preview` lists what Drive
  would show of it. A copy is a new file with its own row, written before the
  command returns, with no parse artifacts (an Office file's converted PDF
  is copied); `copyInto` kicks each copied file's parse, a folder's files
  included.
- **Imports.** `import_uploads` copies files and, recursively, folders (no
  dot-names or symlinks), one outcome per file with `source` its path from the
  picked folder; each outcome's `file.category` is the row's (a dropped `.md`
  is a document). A browser download's `Dest` takes the same optional
  `target_dir`.
- **Busy files.** A move, rename or delete is refused (`io`, "being parsed
  right now") while a parse or embed runs on any file it touches, and holds
  those files (`parse::InFlight`, `InFlight::embedding`) until its rows are
  written, so no parse starts on a file mid-move; a canvas shared to a device
  is refused the same way.
- **`drive-changed`.** Each command but `delete_preview` emits
  `{ subjectIds, moved: [{ from, to }], removed, added }`, and so does
  `reconcile_own_files` when it adds rows; `useDriveEvents` turns it into
  `DRIVE_CHANGED_EVENT` for each subject and hands the moves and deletes to
  the path followers (below).

A refusal arrives as a JSON-serialised `{ kind, message }`, `kind` one of
`read_only` (anything synced or hidden), `exists`, `not_found`,
`invalid_name`, `io`; `driveErrorText` words `read_only` as "Course files are
read-only" and shows any other message, or the raw text when it isn't one.

### Own files written outside the app get rows

A chat agent, Finder or another app can write straight into a subject's own
files. Such a file lists in Drive (the listing reads the disk) but has no
`files` row until `reconcile_own_files`
(`app/src-tauri/src/library/drive/ops/reconcile.rs`) gives it one. It walks
the subject's own zone (`layout::own_files`) and inserts a row for each file
with none, with `category_of`'s category, leaving out what Drive hides:
parse artifacts, converted PDFs, and the markdown of a parse still running
(checked by claiming the PDF for a moment). It never updates or deletes a
row, and announces what it added on `drive-changed`. `reconcileOwnFiles`
(`app/src/lib/drive/reconcile.ts`) kicks each new file's parse as an
upload's is kicked, then brings notes and drawings in line
(`reconcileDocuments`, `reconcileDrawings`, which also drop rows whose file
is gone). `scheduleOwnReconcile` runs it when the browser opens a folder in
a subject (the Files tab, or Drive below a subject): calls are debounced per
subject, never overlap, and are skipped for 10 s after a pass, so the
listing reloading on the pass's own event does not start another.
`reconcileAfterTurn` forces it when a chat turn ends (`turn_finished` in
`useHarnessEvents`), for the thread's subject, or every current subject for
a thread filed under none.

### Path-keyed state follows Drive

On each `drive-changed`, `useDriveEvents` hands `{ moved, removed }` to
`followLibraryChange` (`app/src/lib/shell/followLibrary.ts`), whose rules
are `followRoute` and `followPath` (`app/src/lib/shell/pathFollow.ts`). A
folder's entries match by prefix at a `/` boundary, and the longest moved
prefix wins.

- **Routes.** A file page (`?path=`) takes its file's new path, and its
  subject's id when the file changed subject; other query parameters stay.
  A Drive or Files folder page follows its folder, and when the folder is
  deleted it moves up to the folder that held it. A deleted file's main pane
  goes to the Files tab on that file's folder, and a side item on it is
  removed.
- **Where.** Open tabs and side items (a pane with a router is navigated
  with `replace`, so its older history entries keep the old path; one never
  built only has its stored path changed), the closed-tab stack and Recent
  (`followRecents`; a row on a deleted file is dropped) in `oculus-tabs` and
  `oculus-recent-tabs`.
- **Job queues.** The parse, pipeline and embed queues re-key the files they
  are waiting on, or drop deleted ones, and never touch a running file,
  since Drive refuses to move one.
- **Paths from before the library move.** Once a session, after the first
  render, `followSavedPaths` checks each saved tab, side item, closed tab
  and Recent row on a file with no row and no file on disk against
  `path_moves` (`resolveMoved`) and applies what it finds as one move.
- **Left as written.** Chat drafts, pastes, scopes, shelves, attachment
  names and question drafts (`app/src/stores/chat/draftStore.ts`,
  `app/src/stores/chat/stashStore.ts`, `app/src/stores/chat/attachmentNames.ts`,
  `app/src/stores/chat/questionDrafts.ts`), the stash, and
  sent messages (`harness_items`, `agents/threads/`) keep their text; a
  mention whose file is gone shows as unavailable.

## The subject Overview derives activity from source dates

`useSubjectOverview` in `app/src/hooks/data/overview/` takes a subject id and
a week key; null or omitted selects the current week. Its exported
`SubjectOverviewData` and item types are the page's data contract. It returns
the [term-wide strip](./calendar.md#teaching-weeks-are-shared-across-a-term),
activity, the next five deadlines, classes or fallback recordings, student
work, behind lectures, teaching-week progress and the subject's revision due
count. An empty strip yields no weekly items, progress or behind warning;
deadlines and revision remain independent of the selected week.

Activity uses [source dates](./sync.md#library-source-dates-describe-activity-at-canvas-and-ed),
falling back to SQLite UTC `first_seen_at` only when source creation is NULL.
One row per item shows its latest qualifying change in the selected week:
file/page updates must be more than an hour after creation; Ed updates mean
replies and have no one-hour gate. `parseEdThread` counts rendered reply
headers, including nested comments; a file without a known count says
Replies. Announcements use creation only. Student documents, canvases,
notebooks and uploads appear in Yours, by creation or content change; chats
merge live harness rows and every page of the subject's completed threads.
All comparisons use parsed instants. Weekly sections use `weekOf`, including
its folding of early activity into week 1 and late activity into +3.

Canvas deadlines match task documents by assignment id or source link
(`parseTaskDoc` reads the Open in Canvas link), then by a unique title and
due minute for quiz shells whose assignment id is absent. Submitted/graded
documents are excluded. Dated open project tasks join by their project's
subject. Events prefer the subject's entire valid Canvas class layer over
recordings, even when the selected week has no classes; a local event cannot
establish that layer. An event becomes past after its end, or start without
an end. Behind counts only already-dated, unwatched recordings in teaching
weeks through the current one, excludes untimetabled streams and stays quiet
before teaching, in breaks and after teaching. Progress counts all recordings
in teaching weeks, including other streams; watched means marked Done or past
the player's `isWatched` threshold. `app/src/lib/revision/due.ts` shares the
active-and-due rule with `app/src/pages/revision/ReviewPage.tsx`.

`sinceLastVisit` is an epoch-millisecond snapshot from per-subject device
storage. A first visit marks every dated activity item new; later visits mark
items strictly newer than that snapshot. `markVisited` writes the current
time without changing the snapshot, so the page calls it on unmount and dots
survive the visit. Every storage access tolerates unavailable storage.

The hook reuses subject files and weeks, rejects stale asynchronous reads,
and refreshes after file access/scrapes, document/canvas/upload changes, calendar,
lecture/download/progress, project, revision and harness-thread events. Its
minute clock advances deadlines, past events and revision counts. Loading
and read errors are exposed; metadata read failures retain filename fallbacks.

## Backend events are the write path

- **`useBackendEvents` (mounted once in `App.tsx`) turns Tauri events into
  SQLite writes and store updates**; pages read the DB and stores, never the
  scraper. The CLI writes the same scrape tables through
  `app/src-tauri/src/db/store/mod.rs`, so a table's shape moves both writers and the
  migration together. `harness-event` rows are written by Rust; the hook only
  feeds `harnessStore` ([harness.md](./harness.md)).
- **Callers share one in-flight database load** (`getDb` in
  `app/src/lib/db/connection.ts`), including restored panes and StrictMode mounts; a
  failed load lets the next caller retry.
- **Unread file counts preserve unchanged subject/category maps**, and badge
  subscribers read only their own subject's counts. A refresh with identical
  counts does not publish a store update.
- **Parse status writes are ordered per file** through `parseStatusWriter`:
  progress heartbeats share a transition write, while live page counts update
  immediately. Scrape insertion and reset share that queue; only an update
  that found its row counts as persisted, so failed and early writes can retry.
- **A page reloads on the hook's own window event** (e.g.
  `FILE_SCRAPED_EVENT` in `app/src/lib/pipeline/syncRunner.ts`), never on the Tauri
  event, which races the upsert. Subscribe with `useTauriEvent` /
  `useWindowEvent`.
- **Project writes refresh through `PROJECTS_UPDATED_EVENT`, not the store**,
  because the chat agent writes too (`oculus project` / `oculus task`);
  `useBackendEvents` fires the same event when a finished tool call names one.
- **`getSubjects` derives, it doesn't read**: `last_synced_at` from the latest
  *completed* `sync_runs` row, and `is_current` as the newest `term_sort`
  among available rows rather than the stored column, which a course fetch
  stamps only for the courses it returned. Terms sort by `term_sort`
  ([sync.md](./sync.md)), never by name — `Su` > `Se` would file Summer last.
- **Errors**: `errorElement` takes down one pane, `ErrorBoundary` the shell, and
  `app/src/main.tsx`'s overlay paints handler throws (no console in release).

## Settings → Parsing and Embeddings: switching parser is free, switching embedder is not

- **The parser select has no confirmation dialog, and must not get one**: both
  MinerU engines write the same artifacts at the same `PARSER_VERSION`. The
  token row shows only under Cloud; the address and a four-state status line
  (reachable, unreachable, `version_mismatch`, not asked yet — never drawn as a
  failure) only under Local. Engine lists and refusals come from Rust. Saving a
  token checks it and lifts the parse latch. See [parsing.md](./parsing.md).
- **`EngineSelect` and `CredentialField` share the settings controls** (`app/src/components/settings/shared/EngineSelect.tsx`, `app/src/components/settings/shared/CredentialField.tsx`); each section owns validation, switching and destructive confirmation.
- **Changing the embedding engine is destructive on purpose** — two models'
  vectors share a table and no geometry. `embed_set_engine` clears the
  `.emb.json` records, the vectors and every `embed_status` but `'queued'`
  (the persisted queue carries over into the new space), then writes the
  setting **last**; `ReindexConfirmDialog` shows the loss first. See
  [retrieval.md](./retrieval.md).
- **`indexStore` is a queue with one worker** — the backend paces against the
  per-minute ceiling — fed by the Index button, a finished parse and a row's
  retry. The queue is a module-level array so the worker sees files appended
  mid-run. Stopping lands between files. Nothing queues until `ready`
  (`credentials_ready` and the engine's `available`), and a finished parse
  never enumerates the backlog — that is the Index button's job. Queued rows
  are marked in `files.embed_status` and re-enqueued once at boot
  ([retrieval.md](./retrieval.md#ingest-follows-a-parse-through-one-queue)).
- `embed_estimate` is its own command because it opens every outstanding PDF,
  one sweep at a time. The spend guard is
  enforced in Rust (`UsageLedger::budget`), never on this page.

## Every surface shows parse state from one function

- **`app/src/lib/pipeline/parseState.ts` maps a status and the failure discriminants
  (`kind`, `retryable`, `latching`) to eight states**: `parsed`, `parsing`,
  `queued`, `skipped` (the user's choice — quiet, never a failure),
  `not parsed` (quiet — not a failure), `failed`, `can't parse`
  (`retryable: false`) and `on hold` (a latching cause that condemns the whole
  library). Unknown discriminants stay their own case.
- **A file row shows the state as an icon** (`app/src/components/files/ParseState.tsx`),
  its title and a few words by failure `kind` on hover — never the backend's
  message, which can run to a TLS chain. Clicking `failed` or `can't parse`
  opens the file with its header's parse popover open: the whole message and,
  for `failed`, Retry. `not parsed` re-kicks `parse_file`; `skipped` lifts
  the mark (`parse_skip` with `skip: false`) and then parses, and its file
  page's popover offers the same as Parse now; a token cause opens
  Settings → Parsing; `on hold` has nothing to do.
- **`"quality"` is the terminal parse success, not a tier** — every parsed row
  stores it and Rust's skip check reads it. `embed-status` uses `"done"`.
- **`"skipped"` is the user's skip, stored in `files.parse_status`** through
  the same writer. It is settled, never a failure or a latch: the sweep's query
  leaves it out, and a late `running` heartbeat or the cancelled parse's
  `error` is dropped while a file is skipped. Only Parse now takes it back,
  setting both stores to queued before it calls Rust. New bytes from a sync
  reset it with the rest of the pipeline.
- **The pipeline is `download → parse → embed`**, the third stage drawn only
  when `embedStage` is set, and only for a PDF-backed file (`embedsIn`). A
  spreadsheet's row (`isPipelineFile`) is two stages, its parse being the
  conversion to text: done reads "Converted to text", its steps say
  "Converted", and it offers no Embed or Skip. A running cloud parse carries a `phase` —
  `upload_wait` (its batch is in, another file uploads first), `uploading`
  with bytes, `processing` — which the row keeps with the first and latest
  upload samples; `uploadEta` gives a time left only after 10 s of them, and
  `fmtEta` rounds it coarser as it grows. Embed progress comes from page coverage
  (`getEmbedCoverage`), never `files.embed_status`, which can't tell which
  model wrote the vectors; that column is read only for a failure.
- **A rate-limited embed stays `active`** (`embedWaitingUntil` and
  `embedWaitingReason` on the row, from `embed-status`; any embed event
  without them clears them). `statusOf` keeps the phase, so the row keeps its
  place, and words it as a caption with the reason and a per-second countdown
  ("resuming…" once past due). Settings → Embeddings appends the same wait to
  the index run's line.
- **The Sync page's pipeline rows keep their immutable item identity**
  (`app/src/components/sync/PipelineTable.tsx`), so one progress tick renders
  only its changed row.
- **A File Activity row says each fact once**: the file (name, subject code),
  one segmented track that is the row's status — a segment per stage, done
  filled green, the moving one filled to its percent in brand (pulsing when no
  percent is known), a held one (waiting for its upload turn, rate-limited)
  in amber and unanimated, a paused file's next stage amber, skipped hatched,
  failed red — with one caption beside it (`statusOf`'s label — "Indexed" once
  done — an upload's time left, or a failure's short cause), and when the file
  last moved. The track's tooltip is the whole caption, plus a finished file's
  page count. The table sizes by its own width (`@container`): narrow, the
  caption hides behind that tooltip, and narrower still the subject code goes,
  so the name keeps the room. Clicking the name opens the file beside the page (⌘-click a new
  tab, via `data-tab-href`); clicking elsewhere expands it. Row actions
  (▶ resume, retry, embed or parse a skipped file; Skip; Open beside) take the
  time's place on hover or keyboard focus. Skip is offered until the parse
  finishes and needs no confirmation — Parse now undoes it. The expanded row
  holds only what the row leaves out: each step with its clock time (and the
  upload's size), the whole error sentence or what a wait means, the path, and
  the actions as labelled buttons.
- **Live events and the DB seed share each row, and neither may strand it.**
  A sync's `unchanged` file only settles an existing row's download — it
  never creates one, since no parse event follows. `updated` resets parse and
  embed, because Rust purged the artifacts. An embed event marks the parse
  done only on a row's first sighting, or a re-parse behind an old embed
  would lose its progress. A skip-path `quality` (already parsed) neither
  restamps `parsedAt` nor re-queues an embedded file.
- **The seed merges and prunes, and runs on every activation and after every
  sync run** (`SyncPage.tsx`, `pipelineStore.seed`): an idle row advances to
  the DB's state where the DB is further along, a row touched in the last
  minute is left to its events, and a row whose file left the DB is dropped.
  "Clear finished" removes completed and skipped rows (`isComplete` counts a
  skip as settled) and remembers them, so a re-seed doesn't bring them back.
- **Rows sort by their latest stage completion, never `updatedAt`**, which
  every progress tick bumps — live rows would swap places and jump pages.
  Phases rank active, waiting, paused, failed, skipped, done. Retry is offered
  only where it can work: not on a failed download (the next sync fetches it),
  a non-retryable error, or a latching one.
- **`useQualitySweep` is the recovery path, with two gates** (and it never
  touches a skipped file): never re-kick
  `retryable === false`, and stand down on a latching failure until a parse
  progresses or `LATCH_PROBE_AFTER_MS` allows one probe. Without them one bad
  token marches the library through the same error every sweep.
  Files already kicked this session go to the back of the line, so a file
  that fails every time cannot take the whole budget. Only a `running`, or a
  `quality` after a live `queued`, lifts the latch — a skip-path `quality`
  proves nothing about the engine.

## Uploads and documents are ordinary library files

- **An upload gets its `files` row before its first parse kick**
  (`app/src/lib/files/uploads.ts`), because the `parse-status` handler finds the row
  by path. `recordUploads` handles picked files and
  [browser saves](./viewers.md#downloads-wait-for-a-destination-while-their-bytes-arrive).
  Paths cross IPC, never bytes, and a taken name steps aside
  (`notes (2).pdf`) unless the bytes are identical. A new upload lands in the
  folder it was dropped on (`targetDir`), else at the subject's root beside
  `Course/`; its row's category is the one Rust read off its path.
- **Finder drops land on Drive's folders** (`useDriveDrop`): a folder card,
  list row or tree row, else the folder on screen. `useFileDrop` ignores a
  `visibility: hidden` element — how background panes hide.
- **Delete is Drive's `delete_entries`** (above), guarded by
  `layout::own_path_ok`; an upload's name never starts with a dot, which the
  guard refuses. `deleteFileRow`, which the reconcilers use, deletes `pages`
  and `document_versions` by hand, since the cascade fires only with
  `foreign_keys` on. A parse in flight can land after a delete, so
  `store_upload` purges artifacts on a reused name whenever the bytes differ.
- **The PDF/markdown toggle probes file metadata**, through
  `course_file_has_content` in `app/src-tauri/src/library/files/commands.rs`, without reading or
  transferring the parsed text. `app/src/components/files/FileMarkdown.tsx`
  renders the markdown (a parsed PDF's from its `.pages.json` record,
  [viewers.md](./viewers.md)) and discards obsolete read responses.
- **Scraped header metadata loads through `useCourseFileData`**: assignments,
  discussion threads and module TOCs supply a stable parser, while the hook
  owns parallel reads and cancellation of stale answers. Unchanged file rows
  reuse parsed results and pending reads; scrape/access stamps invalidate them,
  and entries outside the current list are discarded.
- **Subject and file lists share in-flight reads**: concurrent subject reads
  and each subject's initial file reads share one query, without caching
  completed rows. Explicit reloads read fresh rows; obsolete replies cannot
  replace a newer subject or refresh. File-access and document-change events
  carry their subject id, so unrelated mounted file lists do not requery;
  events without an id refresh every list.
- **A module video downloads from its Modules row**, and a `<canvas-video>`
  in a page from its card (the backend side is in
  [sync.md](./sync.md#a-sync-lists-module-videos-and-the-student-downloads-them)).
  Both use `videoDownloadStore`, which keys state by Canvas file id so a
  download outlives the page and the two agree, and subscribes to
  `canvas-video-progress` on its first download. The row comes from the ordinary `scrape-file` handler, which can land after the
  command resolves, so the store waits for `FILE_SCRAPED_EVENT`
  (`scrapedRow`) before the row opens the file.
- **An Echo360 video embedded in a page downloads from its card**
  ([viewers.md](./viewers.md#one-markdown-renderer-serves-every-surface),
  backend in [sync.md](./sync.md#an-embedded-echo360-video-is-resolved-and-downloaded-by-its-card)).
  `embedDownloadStore` is the same shape keyed by the embed's `linkId`: each
  link's info (asked once per app session) and download, `echo-embed-progress`
  subscribed on the first download (`downloading` sets the percent, `muxing`
  the joining label; the invoke settles the rest), and the same wait on
  `scrapedRow` before the card shows Play.
- **A YouTube or Vimeo video in a page only plays**, in a browser tab.
  `webVideoStore` holds each video's oEmbed answer (`web_video_info`), asked
  once per app session ([viewers.md](./viewers.md#one-markdown-renderer-serves-every-surface)).
- **A document is a `.md` among the student's own files**
  (`courses/<code>/<title>.md`, or deeper) with a `category = 'document'`
  row. The title is the filename (`fileTitle`); a rename moves the file and
  keeps the row id. `reconcileDocuments` brings rows in line with
  `list_documents` (every own `.md` at any depth, less a parsed PDF's or a
  sheet's text) when `reconcileOwnFiles` runs, so a note written elsewhere
  just appears.

The editor a document opens in is [editor.md](./editor.md). Drawing canvases are
listed by Drive, with a `canvas` category and versioned `.canvas` files; see
[drawing-canvas.md](./drawing-canvas.md).

## Gotchas

- **Sibling elements never share a `key`.** React finds a child's previous render by key
  among its siblings; with a shared key one old sibling is dropped from the tree
  without being unmounted, so its DOM stays on the page and its effects never clean up
  (a task page would stack every body it had left). `app/tests/repo/siblingKeys.test.ts`
  fails on two JSX siblings written with the same key.
- **Import `app/src/lib/platform/tauriEvents.ts` before `./App`** — it patches an
  `unlisten` that throws under StrictMode's remount and leaks the subscription.
