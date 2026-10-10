# Frontend

React 19 + Vite + Tailwind v4 in `app/src/`: a strip of tabs, each a page with
an optional side panel beside it and a router per pane, inside a Notion-style
shell. This page is the data side — backend events, settings, parse state and
library files; the rest of the frontend has its own pages:

| Page | What it covers |
| --- | --- |
| [shell.md](./shell.md) | Per-pane routers, tabs, the side panel, window shortcuts, ⌘F, search |
| [ui.md](./ui.md) | The design rules, and the WebKit and CSS traps |
| [viewers.md](./viewers.md) | Markdown, PDFs, the media player (lectures and library videos) and the in-app browser |
| [editor.md](./editor.md) | The note editor: sessions, find, versions, code, tables, pictures, mentions, `NoteField` |
| [editor-maths.md](./editor-maths.md) | Maths in the note editor: the visual field, the toolbox, shorthands |

## Where

| Piece | Location |
| --- | --- |
| Entry, the event shim, the last-resort error overlay | `app/src/main.tsx`, `app/src/lib/platform/tauriEvents.ts` |
| The first-run gate: onboarding or the shell ([onboarding.md](./onboarding.md)) | `app/src/App.tsx`, `app/src/hooks/shell/useOnboardingGate.ts` |
| Backend events → SQLite and stores | `app/src/hooks/backend/useBackendEvents.ts`, `app/src/hooks/backend/useEvents.ts`, `app/src/lib/pipeline/parseStatusWriter.ts`, `app/src/lib/db/connection.ts` |
| Files tab: uploads and documents | `app/src/pages/subject/files/FilesPage.tsx`, `app/src/lib/files/uploads.ts`, `app/src/lib/notes/documents.ts`, `app/src-tauri/src/library/files/commands.rs` |
| Tasks section (`/projects`, `/tasks`) | `app/src/components/projects/`, `app/src/pages/work/TasksPage.tsx`, `app/src/lib/planning/projects/`, `app/src/stores/planning/projectsStore.ts` |
| Chat | `app/src/pages/tools/ChatPage.tsx`, `app/src/components/harness/`, `app/src/stores/chat/harnessStore.ts` |
| Settings → Parsing, Embeddings, Transcription: parser, embedding, index run, transcription (the language, the engine order and switches, a dialog per engine: Groq key, local Whisper models, on-device speech) | `app/src/pages/settings/library/ParsingPage.tsx`, `app/src/components/settings/library/ParserSection.tsx`, `app/src/components/settings/library/EmbeddingSection.tsx`, `app/src/stores/sync/indexStore.ts`, `app/src/components/settings/transcription/TranscriptionSection.tsx`, `app/src/components/settings/transcription/GroqDialog.tsx`, `app/src/components/settings/transcription/LocalWhisper.tsx`, `app/src/components/settings/transcription/OnDeviceSpeech.tsx` |
| Parser and embedding engine selection UI | `app/src/components/settings/shared/EngineSelect.tsx` |
| Parse state, pipeline ledger, recovery sweep | `app/src/lib/pipeline/parseState.ts`, `app/src/stores/sync/parseStore.ts`, `app/src/stores/sync/pipelineStore.ts`, `app/src/hooks/sync/useQualitySweep.ts` |
| Scraped document metadata loaders | `app/src/hooks/data/useCourseFileData.ts`, `app/src/hooks/data/useModuleTocs.ts` |
| Module videos downloaded on demand | `app/src/pages/subject/content/ModulesPage.tsx`, `app/src/stores/lectures/videoDownloadStore.ts` |

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
  *completed* `sync_runs` row, and `is_current` from `app/src/lib/format/terms.ts`
  rather than the stored column. Terms sort by rank (`TERM_RANK_SQL`), never by
  name — `Su` > `Se` would file Summer last.
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
  by path. Paths cross IPC, never bytes, and a taken name steps aside
  (`notes-2.pdf`) unless the bytes are identical.
- **The drop target is the Files tab's root**, so the import state lives in the
  tab (`app/src/hooks/data/useUploadImport.ts`) to survive switching to Uploads.
  `useFileDrop` ignores a `visibility: hidden` element — how background panes
  hide.
- **Delete is guarded by `is_upload_rel`** (`app/src-tauri/src/library/paths/own_files.rs`), not a
  dialog. `deleteFileRow` deletes `pages` and `document_versions` by hand,
  since the cascade fires only with `foreign_keys` on. A parse in flight can land after a delete, so
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
- **A module video downloads from its Modules row** (the backend side is in
  [sync.md](./sync.md#a-sync-lists-module-videos-and-the-student-downloads-them)).
  `videoDownloadStore` keys state by Canvas file id so a download outlives the
  page, and subscribes to `canvas-video-progress` on its first download. The
  row comes from the ordinary `scrape-file` handler, which can land after the
  command resolves, so the store waits for `FILE_SCRAPED_EVENT` before the
  row opens the file.
- **A document is `courses/<code>/documents/<title>.md`** with a
  `category = 'document'` row. The title is the filename (`fileTitle`); a rename
  moves the file and keeps the row id. `reconcileDocuments` brings rows in line
  with the folder on mount, so a note written elsewhere just appears.

The editor a document opens in is [editor.md](./editor.md).

## Gotchas

- **Import `app/src/lib/platform/tauriEvents.ts` before `./App`** — it patches an
  `unlisten` that throws under StrictMode's remount and leaks the subscription.
