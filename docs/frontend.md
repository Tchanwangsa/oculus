# Frontend

React 19 + Vite + Tailwind v4 in `app/src/`: a strip of tabs, each a page with
an optional side panel beside it and a router per pane, inside a Notion-style
shell. This page is the data side — backend events, settings, parse state and
library files; the rest of the frontend has its own pages:

| Page | What it covers |
| --- | --- |
| [shell.md](./shell.md) | Per-pane routers, tabs, the side panel, window shortcuts, ⌘F, search |
| [ui.md](./ui.md) | The design rules, and the WebKit and CSS traps |
| [viewers.md](./viewers.md) | Markdown, PDFs, the lecture player and the in-app browser |
| [editor.md](./editor.md) | The note editor: sessions, find, versions, code, tables, pictures, mentions, `NoteField` |
| [editor-maths.md](./editor-maths.md) | Maths in the note editor: the visual field, the toolbox, shorthands |

## Where

| Piece | Location |
| --- | --- |
| Entry, the event shim, the last-resort error overlay | `app/src/main.tsx`, `app/src/lib/tauriEvents.ts` |
| Backend events → SQLite and stores | `app/src/hooks/useBackendEvents.ts`, `app/src/hooks/useEvents.ts`, `app/src/lib/parseStatusWriter.ts`, `app/src/lib/db.ts` |
| Files tab: uploads and documents | `app/src/pages/subject/FilesPage.tsx`, `app/src/lib/uploads.ts`, `app/src/lib/documents.ts`, `app/src-tauri/src/files.rs` |
| Tasks section (`/projects`, `/tasks`) | `app/src/components/projects/`, `app/src/pages/TasksPage.tsx`, `app/src/lib/projects.ts`, `app/src/stores/projectsStore.ts` |
| Chat | `app/src/pages/ChatPage.tsx`, `app/src/components/harness/`, `app/src/stores/harnessStore.ts` |
| Settings → Library: parser, embedding, index run | `app/src/components/settings/ParserSection.tsx`, `app/src/components/settings/EmbeddingSection.tsx`, `app/src/stores/indexStore.ts` |
| Parser and embedding engine selection UI | `app/src/components/settings/EngineSelect.tsx` |
| Parse state, pipeline ledger, recovery sweep | `app/src/lib/parseState.ts`, `app/src/stores/parseStore.ts`, `app/src/stores/pipelineStore.ts`, `app/src/hooks/useQualitySweep.ts` |
| Scraped document metadata loaders | `app/src/hooks/useCourseFileData.ts`, `app/src/hooks/useModuleTocs.ts` |

## Backend events are the write path

- **`useBackendEvents` (mounted once in `App.tsx`) turns Tauri events into
  SQLite writes and store updates**; pages read the DB and stores, never the
  scraper. The CLI writes the same scrape tables through
  `app/src-tauri/src/store.rs`, so a table's shape moves both writers and the
  migration together. `harness-event` rows are written by Rust; the hook only
  feeds `harnessStore` ([harness.md](./harness.md)).
- **Callers share one in-flight database load** (`getDb` in
  `app/src/lib/db.ts`), including restored panes and StrictMode mounts; a
  failed load lets the next caller retry.
- **Unread file counts preserve unchanged subject/category maps**, and badge
  subscribers read only their own subject's counts. A refresh with identical
  counts does not publish a store update.
- **Parse status writes are ordered per file** through `parseStatusWriter`:
  progress heartbeats share a transition write, while live page counts update
  immediately. Scrape insertion and reset share that queue; only an update
  that found its row counts as persisted, so failed and early writes can retry.
- **A page reloads on the hook's own window event** (e.g.
  `FILE_SCRAPED_EVENT` in `app/src/lib/syncRunner.ts`), never on the Tauri
  event, which races the upsert. Subscribe with `useTauriEvent` /
  `useWindowEvent`.
- **Project writes refresh through `PROJECTS_UPDATED_EVENT`, not the store**,
  because the chat agent writes too (`oculus project` / `oculus task`);
  `useBackendEvents` fires the same event when a finished tool call names one.
- **`getSubjects` derives, it doesn't read**: `last_synced_at` from the latest
  *completed* `sync_runs` row, and `is_current` from `app/src/lib/terms.ts`
  rather than the stored column. Terms sort by rank (`TERM_RANK_SQL`), never by
  name — `Su` > `Se` would file Summer last.
- **Errors**: `errorElement` takes down one pane, `ErrorBoundary` the shell, and
  `app/src/main.tsx`'s overlay paints handler throws (no console in release).

## Settings → Library: switching parser is free, switching embedder is not

- **The parser select has no confirmation dialog, and must not get one**: both
  MinerU engines write the same artifacts at the same `PARSER_VERSION`. The
  token row shows only under Cloud; the address and a four-state status line
  (reachable, unreachable, `version_mismatch`, not asked yet — never drawn as a
  failure) only under Local. Engine lists and refusals come from Rust. Saving a
  token checks it and lifts the parse latch. See [parsing.md](./parsing.md).
- **`EngineSelect` and `CredentialField` share the settings controls** (`app/src/components/settings/EngineSelect.tsx`, `app/src/components/settings/CredentialField.tsx`); each section owns validation, switching and destructive confirmation.
- **Changing the embedding engine is destructive on purpose** — two models'
  vectors share a table and no geometry. `embed_set_engine` clears the
  `.emb.json` records, the vectors and `embed_status`, then writes the setting
  **last**; `ReindexConfirmDialog` shows the loss first. See
  [retrieval.md](./retrieval.md).
- **`indexStore` is a queue with one worker** — the backend paces against the
  per-minute ceiling — fed by the Index button, a finished parse and a row's
  retry. The queue is a module-level array so the worker sees files appended
  mid-run. Stopping lands between files. Nothing queues until `ready`
  (`credentials_ready` and the engine's `available`), and a finished parse
  never enumerates the backlog — that is the Index button's job.
- `embed_estimate` is its own command because it runs pdfium over the library,
  one sweep at a time (pdfium is one session per process). The spend guard is
  enforced in Rust (`UsageLedger::budget`), never on this page.

## Every surface shows parse state from one function

- **`app/src/lib/parseState.ts` maps a status and the failure discriminants
  (`kind`, `retryable`, `latching`) to seven states**: `parsed`, `parsing`,
  `queued`, `not parsed` (quiet — not a failure), `failed`, `can't parse`
  (`retryable: false`) and `on hold` (a latching cause that condemns the whole
  library). Unknown discriminants stay their own case.
- **`"quality"` is the terminal parse success, not a tier** — every parsed row
  stores it and Rust's skip check reads it. `embed-status` uses `"done"`.
- **The pipeline is `download → parse → embed`**, the third stage drawn only
  when `embedStage` is set. Embed progress comes from page coverage
  (`getEmbedCoverage`), never `files.embed_status`, which can't tell which
  model wrote the vectors; that column is read only for a failure.
- **The Sync page's pipeline rows keep their immutable item identity**
  (`app/src/components/sync/PipelineTable.tsx`), so one progress tick renders
  only its changed row.
- **`useQualitySweep` is the recovery path, with two gates**: never re-kick
  `retryable === false`, and stand down on a latching failure until a parse
  progresses or `LATCH_PROBE_AFTER_MS` allows one probe. Without them one bad
  token marches the library through the same error every sweep.

## Uploads and documents are ordinary library files

- **An upload gets its `files` row before its first parse kick**
  (`app/src/lib/uploads.ts`), because the `parse-status` handler finds the row
  by path. Paths cross IPC, never bytes, and a taken name steps aside
  (`notes-2.pdf`) unless the bytes are identical.
- **The drop target is the Files tab's root**, so the import state lives in the
  tab (`app/src/hooks/useUploadImport.ts`) to survive switching to Uploads.
  `useFileDrop` ignores a `visibility: hidden` element — how background panes
  hide.
- **Delete is guarded by `is_upload_rel`** (`app/src-tauri/src/paths.rs`), not a
  dialog. `deleteFileRow` deletes `pages` by hand, since the cascade fires only
  with `foreign_keys` on. A parse in flight can land after a delete, so
  `store_upload` purges artifacts on a reused name whenever the bytes differ.
- **The PDF/markdown toggle probes file metadata**, through
  `course_file_has_content` in `app/src-tauri/src/files.rs`, without reading or
  transferring the parsed text. `app/src/components/files/FileMarkdown.tsx`
  renders the markdown and discards obsolete read responses.
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
- **A document is `courses/<code>/documents/<title>.md`** with a
  `category = 'document'` row. The title is the filename (`fileTitle`); a rename
  moves the file and keeps the row id. `reconcileDocuments` brings rows in line
  with the folder on mount, so a note written elsewhere just appears.

The editor a document opens in is [editor.md](./editor.md).

## Gotchas

- **Import `app/src/lib/tauriEvents.ts` before `./App`** — it patches an
  `unlisten` that throws under StrictMode's remount and leaks the subscription.
