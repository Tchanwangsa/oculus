# Frontend

React 19 + Vite + Tailwind v4 in `app/src/`: a strip of tabs, each holding one
or two panes with their own router, inside a Notion-style shell. The design
rules are in [UI system](#ui-system); the WebKit and CSS traps are in
[Gotchas](#gotchas).

## Where

| Piece | Location |
| --- | --- |
| Entry, the event shim, the last-resort error overlay | `app/src/main.tsx`, `app/src/lib/tauriEvents.ts` |
| Route table | `app/src/routes.tsx` |
| Shell: sidebar, tab strip, floating card, zoom | `app/src/layouts/AppLayout.tsx`, `app/src/components/sidebar/`, `app/src/components/tabs/TopTabBar.tsx` |
| Tabs, split panes, per-pane routers, titles from paths | `app/src/stores/tabStore.ts`, `app/src/components/tabs/TabPane.tsx`, `app/src/lib/tabRouters.ts`, `app/src/components/tabs/tabInfo.tsx` |
| Window shortcuts (all menu items) | `app/src-tauri/src/menu.rs` |
| ⌘-click → a new tab, app-wide | `app/src/lib/newTabClicks.ts` |
| Search (⌘K and the new-tab field), Recent group | `app/src/lib/search.ts`, `app/src/lib/searchFilters.ts`, `app/src/components/search/SearchList.tsx`, `app/src/stores/recentTabsStore.ts` |
| Backend events → SQLite and stores | `app/src/hooks/useBackendEvents.ts`, `app/src/hooks/useEvents.ts`, `app/src/lib/db.ts` |
| Files tab: uploads and documents | `app/src/pages/subject/FilesPage.tsx`, `app/src/lib/uploads.ts`, `app/src/lib/documents.ts`, `app/src/components/documents/DocumentEditor.tsx`, `app/src/components/documents/editor/`, `app/src-tauri/src/files.rs` |
| Tasks section (`/projects`, `/tasks`) | `app/src/components/projects/`, `app/src/pages/TasksPage.tsx`, `app/src/lib/projects.ts`, `app/src/stores/projectsStore.ts` |
| Chat | `app/src/pages/ChatPage.tsx`, `app/src/components/harness/`, `app/src/stores/harnessStore.ts` |
| Side panel (file/lecture peek) | `app/src/components/panel/`, `app/src/stores/sidePanelStore.ts` |
| In-app browser | `app/src/pages/BrowserPage.tsx`, `app/src/hooks/useBrowserTabs.ts`, `app/src/lib/browserHistory.ts`, `app/src-tauri/src/browser.rs` |
| Settings → Library: parser, embedding, index run | `app/src/components/settings/ParserSection.tsx`, `app/src/components/settings/EmbeddingSection.tsx`, `app/src/stores/indexStore.ts` |
| Parser and embedding engine selection UI | `app/src/components/settings/EngineSelect.tsx` |
| Parse state, pipeline ledger, recovery sweep | `app/src/lib/parseState.ts`, `app/src/stores/parseStore.ts`, `app/src/stores/pipelineStore.ts`, `app/src/hooks/useQualitySweep.ts` |
| Markdown, maths, mermaid, lightbox, PDF | `app/src/components/markdown/`, `app/src/components/ui/Lightbox.tsx`, `app/src/components/files/PDFViewer.tsx` |
| Lecture player | `app/src/components/lectures/`, `app/src/lib/lecturePlayback.ts`, `app/src/stores/playerPrefsStore.ts`, `app/src/hooks/useTranscriptDock.ts` |
| Persisted view state and collapsed groups | `app/src/hooks/useStoredState.ts` |
| Scraped document metadata loaders | `app/src/hooks/useCourseFileData.ts`, `app/src/hooks/useModuleTocs.ts` |
| Subject page width, loading rows and empty views | `app/src/components/subjects/SubjectPage.tsx`, `app/src/components/ui/PageParts.tsx` |
| Drag gestures | `app/src/hooks/usePointerDrag.ts`, `app/src/hooks/useCardDrag.ts`, `app/src/hooks/useFileDrop.ts` |
| Subject grouping and persisted collapsed groups | `app/src/lib/subjectGroups.ts`, `app/src/hooks/useCollapsedGroups.ts` |
| Transcript search and source-index mapping | `app/src/hooks/useTranscriptSearch.ts` |
| Table chrome | `app/src/components/ui/GridTable.tsx`, `app/src/components/ui/ViewTabs.tsx`, `app/src/components/ui/TablePagination.tsx` |

## UI system

Quiet and neutral: dead-grey whites (no warm or blue cast — anything else
fights the accent), muted indigo `#5e6ad2` as the one colour, Manrope for
headings and Inter for everything else, Notion-style layout.

- **The shell frames a floating document.** Sidebar and tab strip sit on the
  window's `background` with no fill or divider; content is an inset rounded
  `card` with a hairline border (`app/src/layouts/AppLayout.tsx`). Tabs are
  pills on that ground. A sidebar divider or a rule under the strip breaks the
  effect — the card's border is the separation.
- **Buttons and chips are pills** (`rounded-full` in
  `app/src/components/ui/button.tsx`); rectangles are for segmented toolbars
  that override the radius at the call site.
- **Three primitives are a notch below stock shadcn**, whose sizes suit a 16px
  page where this app's body is 14px: `button.tsx` `default` is `h-8`;
  `dialog.tsx` is `p-5`/`rounded-xl` with a 16px title and 13px description;
  `tooltip.tsx` adds `max-w-64` and `break-words` so a long page title or file
  name wraps. Call sites don't override these — don't restore what
  `shadcn add` generates.
- **Monospace is for code only** — the one `font-mono` is
  `app/src/components/markdown/MdComponents.tsx`; the note editor's theme
  (`app/src/components/documents/editor/theme.ts`) gives `--font-mono` to code
  and LaTeX source. Timestamps, counts, IDs and badges take the body font,
  with `tabular-nums` when digits hold a column.
- Headings get Manrope from an `h1–h4` rule in `@layer base`; a title that
  isn't a heading element takes `font-display`.
- **shadcn/ui** lives in `app/src/components/ui` (config `app/components.json`,
  primitives from the unified `radix-ui` package). Add with
  `bunx shadcn@latest add <name>`, then swap `lucide-react` for
  `@phosphor-icons/react`.
- **Colours go through the semantic tokens in `app/src/index.css`.** In
  shadcn's vocabulary `accent` is the quiet hover surface, not the brand. The
  indigo has two tokens: `primary` is the *fill* (buttons, active underline,
  today), `brand` is the *accent* (links, selection, in-flight progress,
  new-item chips) — split so the accent can be retuned without restyling every
  button.
- **Dark mode is a `.dark` class on `<html>`** written only by `applyTheme`
  (`app/src/lib/theme.ts`); `@custom-variant dark` follows the class, not the OS.
- **Full pages scroll through `page-scroll`**, which reserves the scrollbar
  gutter so a page that starts overflowing doesn't jog sideways.
- **Every scroller fades its overflowing edges through `useScrollFade`**
  (`app/src/hooks/useScrollFade.ts`; `syncScrollFade` in
  `app/src/lib/scrollFade.ts` for non-React callers like the maths palette).
  It marks the element `data-scroll-fade="x|y|xy"` and `index.css` turns that
  into a mask, so no overlay and no background colour. Set `--scroll-fade` for
  a ramp other than 24px, and keep the scroller flush against what it sits on
  — padding between them leaves a visible gap under the fade.
- **Tables are full-bleed in `GridTable`**, header outside the scroller (or the
  bar runs down it). Alternate views are sibling `ViewTabs`/`PillTabs`, not a
  dropdown, over a fixed-height toolbar so switching never jolts the rows.
- **Subject tabs share `SubjectPage` for width and gutters**, `SubjectLoading` for skeleton rows and `SubjectEmpty` for the empty view. Per-tab content and actions stay with the page.
- **View preferences use `useStoredState`**, with readers that own defaults and validation; `useStoredSet` keeps collapsed group keys. Storage failures leave the live view usable.
- **No toasts, no bottom progress bars** — background jobs surface in the
  sidebar only. No placeholder UI, section-header icons, stat cards or filler.
- Slugs display through `humanizeSlug`, Canvas codes through `displayCode`
  ("MULT20015", not "MULT20015_2026_SM2"), both in `app/src/lib/format.ts`.

## Each pane has its own router, and the path is its only state

`app/src/routes.tsx` is the route table, and each **pane** — a tab, or one half
of a split tab — builds its own memory router over it
(`app/src/components/tabs/TabPane.tsx`). The shell sits above all of them and
navigates through `navigateActive` / `goInActiveTab` in
`app/src/lib/tabRouters.ts`, which resolve to the focused pane and hold the
departure rules: close the peek, keep a browser tab pinned to its page, ask
before leaving a playing lecture.

- **A tab is titled from its path alone** (`tabInfo`, shared with Recent), so a
  title not in the path rides in the query — `?n=` from `projectHref` /
  `taskHref`, `?t=` on `/lecture` — and a page re-`navigate`s to its own href
  (`replace: true`) after a rename.
- **`/chat` carries the thread, not just its name**: `?t=<id>&n=<title>`
  (`chatHref`). Every tab has its own router, so two Chat tabs hold two threads
  and a restored tab or Recent row reopens its own; a bare `/chat` is the empty
  composer ([harness.md](./harness.md#each-chat-tab-owns-its-conversation-in-its-route)).
- **`/projects` and `/tasks` are one section**, switched by
  `app/src/components/projects/SectionHeader.tsx`, which *navigates* because
  crumbs, ⌘-click, restored tabs and `tabInfo` all key off the path; `NavItem`'s
  `match` lights one sidebar row for both. A project is top-level because it may
  have no subject; a filed task sits under its project, an unfiled one at
  `/tasks/:taskId`, so `tabInfo` tests the task route first. The model is
  [projects.md](./projects.md).
- **`SubjectLayout` resolves the subject once** and hands it down as outlet
  context; tab pages must not re-fetch it. Files is one tab whose sub-tabs are
  routes (`files/downloads`, `files/uploads`, `files/documents`), and its
  `useFilesTab()` context *extends* the subject, because `useOutletContext`
  reads the nearest Outlet and a different shape would make `useSubject()` lie.
- **`/subjects/:id/file` and `/subjects/:id/lecture` sit outside
  `SubjectLayout`** so a document takes the whole card; `SubjectCrumbs` gives
  them a trail back, as buttons with `data-tab-href` rather than `Link`s so a
  click goes through `navigateActive`.

## The shell owns tabs, splits and every window shortcut

- **Window shortcuts are menu items** (`app/src-tauri/src/menu.rs`): macOS
  gives the menu bar every ⌘-key first, and a browser tab's native page takes
  keys the app's webview never sees. They reach the frontend as `menu-*`
  events and the frontend owns what each means. Close Window is ⇧⌘W, and the
  Edit submenu must stay or ⌘C/⌘V die in every field.
- **⌘1–⌘8 are strip positions** (a missing slot is a no-op), ⌘9 is the last
  tab, and ⇧⌘T pops `tabStore`'s `closed` stack back to the old index. A
  browser tab is remembered by URL, recorded by `TopTabBar` *before* Rust
  destroys the page.
- **⌥⌘T splits a tab.** `AppTab extends PaneState`: the main pane carries the
  tab's id, and everything below a tab — router, peek, playing lecture, Recent
  entry — is keyed by pane id. `tab.focus` is set in the capture phase, so it
  has moved before the clicked thing reacts; `focusedPane` is what every shell
  navigation resolves through. The focus marker sits on the seam, since a
  native browser page covers anything drawn inside a pane.
- **Strip tabs are fixed-width** (`TAB_W` down to `TAB_MIN_W`), and the column
  between them (`SEPARATOR_W`) always renders because the reorder maths counts it.
- **The history arrows follow React Router's index** (`history.state`), not
  `window.history.length`, which counts entries a reload left behind.
- **Zoom is the webview's page zoom** (`setZoom` in `AppLayout`); on a browser
  tab ⌘=/⌘−/⌘0 zoom the page. `--app-zoom` exists for the traffic-light gap,
  whose height is `trafficLightPosition` in `app/src-tauri/tauri.conf.json` —
  tied to the strip's height and `DEFAULT_ZOOM`, so move one and re-measure all
  three.
- **⌘-click opens a new tab everywhere, and no call site knows it.**
  `app/src/lib/newTabClicks.ts` is one capture-phase listener that walks up to
  an `href` or a `data-tab-href`, stops at `data-tab-skip`, and matches against
  the real route table (an agent's absolute file paths are not routes). A file
  chip is the one exception, carried by `openLibraryPath` in
  `app/src/lib/openFile.ts`. Plain external links open in the in-app browser
  through a capture handler in `AppLayout` (`openExternal`).
- **A new tab lands on `/new`**, not Home: a search field, a browser door, a
  chat door and the Recent trail. **Home is a launcher with no state of its
  own**; each section hides when empty and re-reads on its tab's front edge
  (`useHomeSection`), and its composer always starts a new thread.
- **Recent is a still list** (`recentTabsStore`): a visit lands only after the
  pane settles, a row is a *thing* (`recentKey` — a subject's tabs are one row),
  and a listed page refreshes in place. Browser tabs, Home and `/new` stay out.

## The side panel is docked, per pane, and opened from anywhere

`app/src/components/panel/SidePanel.tsx` is **docked, not overlaid**: the card
is a flex row, so the page beside it is really narrower and `BrowserPage` can
re-place its native view instead of hiding it. Contents are per pane; width is
global.

- `open()` takes no pane id — background panes are `inert`, so a click can only
  come from the front. A list re-fetching an open row calls `sync()`, which
  names its pane.
- **It unfolds on a count of `open` calls** (`opens`), so re-opening the item
  already showing still unfolds a folded panel.
- **Shell navigation closes it** (`navigateActive` → `closeActivePanel`).
  Expand goes to the full page in the same tab, committed at the end of a width
  sweep; the frame stays mounted at zero width so open and close animate.
- ⌥⌘S/⌘B/⌥⌘B test `e.code`, because ⌥ rewrites the key's character on macOS.

## Backend events are the write path

- **`useBackendEvents` (mounted once in `App.tsx`) turns Tauri events into
  SQLite writes and store updates**; pages read the DB and stores, never the
  scraper. The CLI writes the same scrape tables through
  `app/src-tauri/src/store.rs`, so a table's shape moves both writers and the
  migration together. `harness-event` rows are written by Rust; the hook only
  feeds `harnessStore` ([harness.md](./harness.md)).
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
- **Scraped header metadata loads through `useCourseFileData`**: assignments, discussion threads and module TOCs supply a stable parser, while the hook owns parallel reads and cancellation of stale answers.
- **A document is `courses/<code>/documents/<title>.md`** with a
  `category = 'document'` row. The title is the filename (`fileTitle`); a rename
  moves the file and keeps the row id. `reconcileDocuments` brings rows in line
  with the folder on mount, so a note written elsewhere just appears.
- **The note editor is CodeMirror 6 over the file's exact text**
  (`app/src/components/documents/editor/`); nothing is re-serialised.
  `DocumentEditor.tsx` owns load, save, title and mode. `FilePage` keys the
  whole editor by row id: pending saves and image attachments belong to one
  note, while a rename keeps its editor. **Live** mode renders markdown in place —
  headings, marks, links, lists and checkboxes, quotes, rules, code, pictures,
  KaTeX maths and mermaid diagrams — and shows a construct's source while the selection touches
  it (a heading, quote or list marker while the caret is on its line). **Raw**
  is the same view with those decorations swapped out by a `Compartment`
  for `rawMode.ts`: monospace source with line numbers, markdown and
  frontmatter YAML coloured with the `--color-syntax-*` tokens.
  Under the title, `DocumentMeta` shows subject, created (`first_seen_at`),
  last updated (`modified_at` or this mount's last save) and a word count.
- **Fenced code is parsed in its own language**
  (`app/src/components/documents/editor/codeLanguages.ts`): the info string's
  first word picks a `@codemirror/language-data` grammar, loaded lazily on
  first use; an untagged fence is guessed with highlight.js over a small
  subset, cached, and left plain unless a pattern for that language agrees.
  Highlight classes stop at a nested grammar's edge, so block code's font is a
  `cm-code-text` mark in both modes. Live mode hides the fences behind a
  header row (the label, which reveals the fence to retag it, and Copy) until
  the selection touches the block. Colours are the `--color-syntax-*` tokens.
  A ```mermaid fence that owns its lines draws as a diagram (`MermaidWidget`
  in `widgets.ts`, through `mermaidRender.ts`, shared with `Mermaid.tsx`);
  while the selection touches it, the source shows with the diagram under it,
  redrawn in place as it is typed and kept at its last good drawing while the
  source does not parse.
- **A pipe table is a grid whose cells edit in place**
  (`app/src/components/documents/editor/table.ts`): its DOM and input handlers
  use `app/src/components/documents/editor/tableModel.ts` for source-preserving
  edits. A cell writes back only its own source range, Tab/Enter move between
  cells, and the frame scrolls sideways.
  A drag across cells (or Shift with a click or ↑/↓) selects a block, which
  the scroller holds focus for: Delete empties it, and on a block already
  empty removes the whole rows (never the header) or columns it spans, or
  the table. ⌘C copies it tab-separated. Edge handles for the row and column
  under the pointer select them on a click and move them on a drag; a moved
  row keeps its bytes, a moved column rewrites rows with outer pipes, and the
  header row never moves (a body row may be short of cells).
  Rows are split from the line text, since lezer emits no node for an empty
  cell. In Live mode it never shows its source: its range is atomic, and
  `tableKeys` (`livePreview.ts`) hands the caret in — ↑/← from the line under
  it into the last row, ↓/→ from the line over it into the header, Backspace
  and Delete likewise (deleting an empty line in between when no text would
  end up against the table). Arrows at the grid's edges, and Esc, hand it back.
  Text typed or pasted at a table's edge goes on its own line, after a blank
  line below it, since GFM reads a line straight under a table as a row.
  **Leading YAML frontmatter** parses as `Frontmatter`
  (`app/src/components/documents/editor/frontmatter.ts`) and draws as a
  Properties card, revealing its source when touched.
- **Toolbar buttons and shortcuts are plain CodeMirror commands**
  (`app/src/components/documents/editor/commands.ts`) that rewrite markdown
  and unwrap when already applied, so they work in both modes. ⌘-click opens a
  link — library files through `libraryLinkTarget` and `openFileSmart`, web
  URLs in the in-app browser; a plain click edits it. Maths is `$…$` (pandoc's
  spacing rule), `\(…\)`, and `$$` / `\[` blocks on their own lines
  (`app/src/components/documents/editor/mathSyntax.ts`).
- **Maths is typed as LaTeX in the note, with a popover beside it**
  (`app/src/components/documents/editor/mathTools.ts`): while the caret is in
  maths (`mathAt` in `mathContext.ts`), in either mode, a CodeMirror tooltip
  under the maths shows a live KaTeX preview (the parse error in red), recents
  (`localStorage`), a one-row tab strip that scrolls sideways, and a palette
  three rows tall that scrolls, with a matrix-size grid (`mathPalette.ts`). Esc hides it until the caret leaves that maths. Inside
  maths, `\` plus a letter opens completion with KaTeX previews; it is the
  editor's one `autocompletion()` (`extensions.ts`), so other sources join its
  `override`. Palette buttons and completions insert `snippet()`s whose `{}`
  slots are Tab fields; the snippet keymap is `Prec.highest`, above the note's
  Tab. The caret between a lone `$$` pair (what Σ inserts mid-line) counts as
  empty maths, since `$$` never parses inline.
- **Typed shorthands expand inside maths**
  (`app/src/components/documents/editor/mathShorthand.ts`, rule tables at the
  top): `a/` → `\frac{a}{}`, `sr` → `^2`, `@a` → `\alpha`, `->` → `\to`,
  `sin ` → `\sin `, `\left…\right` around a closed group holding a tall
  construct. Never inside `\text{}`-like arguments. The typed character lands
  first and the rewrite is its own history event, so ⌘Z gives back what was
  typed.
- **A pasted, dropped or picked picture is written beside its note** in
  `documents/assets/`, on arrival, and linked relatively; `FileViewer` and the
  editor both resolve it with `libraryImageSrc` (`app/src/lib/libraryLinks.ts`).
  `is_document_rel` demands a `.md` one level down, so nothing can edit
  `assets/`.
- **AI suggestions are ghost text at the caret**
  (`app/src/components/documents/editor/aiSuggest.ts`), off by default
  (`document_suggestions_enabled`; the topbar's `SuggestToggle`, shared by
  every note through `documentPrefsStore`). 500 ms after an edit, with one
  empty caret not mid-word, outside code and frontmatter and with no
  completion open, up to 4000 characters before the caret and 1500 after go to
  `document_suggest` on the `documentSuggestions` job's model. An answer
  lands only if it is the newest request and the text and caret are
  unchanged. Tab takes it (one undo step), Mod-→ the next word, Esc drops it;
  typing its front eats it, anything else drops it. Its keymap is
  `Prec.highest` but passes without a ghost, so the note's and snippets' Tab
  are untouched. Blur, toggle-off and note switch call
  `document_suggest_cancel`. A failure is a red dot on the toggle until a
  request succeeds.
- **`@` links a library file**
  (`app/src/components/documents/editor/mentions.ts`): at a word start,
  outside maths, code and frontmatter, the query after it (the chat's caps)
  searches every file of the note's subject, parsed or not, but the note
  (`searchNoteLinkFiles`); an empty query lists recently opened files. It is a
  source in the one `autocompletion()`. Accepting writes
  `[title](../<path>)`, each segment percent-encoded (`libraryLinkHref`),
  which `libraryLinkTarget` decodes, so ⌘-click and `FileViewer` resolve it
  alike. The subject and note path reach the editor through `NoteHost`.
  Lectures have no link form a note can open, so `@` lists files only.

## One markdown renderer serves every surface

- **`app/src/components/markdown/MdComponents.tsx`** renders Canvas bodies,
  parsed PDFs, Ed threads and replies with KaTeX. `normalizeMath` rewrites
  `\(…\)` / `\[…\]` to `$…$` / `$$…$$` because CommonMark eats the backslash
  first. An inline `<code>` holding only a library path renders as `FileChip`.
- **A reply goes through `CompactMd`**, whose `.md-compact` rules in `index.css`
  are deliberately *unlayered* — in `@layer base` they would lose to the
  utilities they override. **`InlineMd` flattens blocks** because chapter
  summaries sit inside buttons, and a `<p>` in a `<button>` closes it early.
- **`FileViewer` resolves in-file links locally** — relative links and Canvas
  `/files/<id>` or `/pages/<slug>` URLs, by `canvas_id`, path or `source_url`
  (`app/src/lib/libraryLinks.ts`, shared with the note editor).
- **A ```mermaid fence is caught at `pre`** (`Mermaid.tsx`), and the original
  `<pre>` shows until it renders or if it never parses. Config and drawing are
  `mermaidRender.ts`:
  - `htmlLabels: false` must sit at the config's **top level** (mermaid 12
    ignores it under `flowchart`); HTML labels wrap by an exact float compare
    that page zoom breaks, clipping every long label.
  - `layout: "dagre"` must ship with the spacing keys (`FLOWCHART_LAYOUT`), or
    `rankSpacing`/`nodeSpacing` are discarded.
  - The label is bounded, not the box: `--diagram-min`/`--diagram-max` keep
    labels at or above `MIN_LABEL_PX`, walking tall diagrams toward
    `TARGET_HEIGHT_PX`; below the floor it scrolls rather than shrinks.
  - Colours come from `--diagram-*` in `index.css`, which exist because
    Tailwind v4 emits a theme variable only if something references it.
  - The lightbox copy gets rewritten ids (`rescope`), since mermaid scopes styles
    and arrowheads by id.
- **`Lightbox.tsx` pans with its container's scroll and zooms with a
  `transform`**; every input moves a target the painted scale eases toward, so
  bursts compose.
- **`PDFViewer` mounts pdf.js's own viewer**, loaded by `app/src/lib/pdfjs.ts`
  through awaited dynamic imports because `pdf_viewer.mjs` reads
  `globalThis.pdfjsLib` at evaluation. `index.css` pins `color-scheme` to
  `.dark`, overriding the `:root` rule pdf.js's stylesheet adds. A pinch arrives
  as both `ctrlKey` wheel and `gesture*` events; only the gesture zooms.

Editor commands, links, maths and completions share `ancestorAt` in
`app/src/components/documents/editor/syntax.ts`; each caller chooses its caret
bias and eligibility rules at a construct's boundary.

Chat's composer (`MentionInput.tsx` sends chips as backticked library paths),
picker and timeline are [harness.md](./harness.md).

## The in-app browser is a native page per tab, owned by Rust

A browser tab is `/browse/<id>`, naming a WKWebView that
`app/src-tauri/src/browser.rs` parks over an empty slot in `BrowserPage` (Canvas
refuses iframes).

- **Rust owns the tab list** and pushes `browser-state` on every change;
  `useBrowserTabs` reconciles it per *pane*. Page navigations change the URL in
  Rust only, never the route. What a page knows about itself (history, find,
  zoom) is pushed as events, because `with_webview` returns nothing.
- **A native page can't interleave with the DOM.** `BrowserPage` hides it while
  a portal overlaps the slot, while its tab is backgrounded (`useTabActive`),
  and while the address list is open — showing a PNG still
  (`browser_snapshot`) so the card doesn't blank.
- **History is one row per URL, ranked by frecency**, recorded on load
  *finish* (one row per redirect chain). `historyUrl` drops the fragment, and
  the whole query if any part looks like a credential.
- **A page must claim to be Safari** (`PAGE_USER_AGENT`) or UA-sniffing sites
  serve fallbacks, and an `initialization_script` reports a real
  `outerWidth`/`outerHeight` (a child view's are 0, which drops canvas renderers
  to minimum scale).

## Search is one module behind two fields

`app/src/lib/search.ts` serves ⌘K and the new-tab field; it returns data and
`openSearchItem` dispatches, because the palette drives the shell from outside
every router while the field drives its own pane. Titles match every typed word
in any order. **Text inside documents is lexical**: `searchPageText` over the
`pages_fts` index, one row per file, only for files the title search missed —
semantic search stays in chat ([retrieval.md](./retrieval.md)). No match is
offered as a URL or web search through `normalizeAddress`, shared with the
address bar.

`app/src/lib/searchFilters.ts` owns the filter catalogue, tokens and resolution
without database or navigation effects. ⌘K takes Discord-style filters: `in:<subject>` and `type:<kind>`. Typing
the colon turns the key into a chip with the caret inside it, and the list shows
only its values; picking one (or a space after an unambiguous value) fixes the
chip, one per key. Backspace at a field's start drops the chip being typed, or
else the last chip. Chips scope the SQL (`SearchScope` in
`app/src/lib/db.ts`) and drop the Web and Go to sections; `runSearch`'s
`offerFilters` is what the new-tab field leaves off.

## The lecture player's elements outlive the page

`LecturePlayer.tsx` streams from Rust's media server via `mediaSrc()`
(`app/src/lib/media.ts`), not `convertFileSrc` ([architecture.md](./architecture.md)).

- **`app/src/lib/lecturePlayback.ts` owns the `<video>` elements**, so playback
  survives tab switches. Between players they park in a 480×270 off-screen
  host — `display: none` lets WebKit stop playback, and a tiny host decodes
  tiny. Progress saves every 5s and on pause, seek, end and `pagehide`.
- **Only the player in front adopts the elements** and hears Space.
- **Leaving a playing lecture asks at the door** — the strip's × and
  `navigateActive` / `goInActiveTab` call `confirmLeavingLecture` — never
  through a router blocker, which strands the pane when nobody answers. The
  peek's body sits under a `TabContext.Provider` so it has an owning tab.
- **Two streams, one clock.** The main-frame source leads (audio, clock,
  progress); followers are rate-trimmed past `MAX_DRIFT` and seeked only past
  `SEEK_DRIFT`. `syncLectureSources` takes the whole plan, so only it can tell
  a fresh lecture (resume) from a source switch (keep the second). Speed and
  volume re-apply per leader element, since both reset on load.
- **Fullscreen is the Tauri window plus a `fixed inset-0` overlay**: element
  fullscreen shows only its subtree, and the player's Radix popups portal
  outside it. Fullscreen and the dock are off in the side panel.
- **Preferences are global** (`playerPrefsStore`); only the position is per
  lecture. The clock counts against the leader's duration, not the catalogue's.
- **The dock is Chapters, Transcript and Chat** in a reorderable `ViewTabs`
  strip, always mounted because Chat needs no file. `useTranscriptDock` raises
  the floor for Chat and caps the drawn size by the container (`KEEP_W` /
  `KEEP_H`), dropping a side dock to the bottom when it can't fit — without
  rewriting the preference.
- **The transcript is virtualised** (~2500 cues a lecture) through `FollowList`,
  shared by both registers. Following is tracked by pointer *intent*, since the
  follow-scroll fires `scroll` too, and runs off the active-cue index rather
  than `timeupdate`. Don't call `measure()` on a search: heights are cached by
  cue key and survive it. Chapters and the reading copy are
  [chapters.md](./chapters.md).

## Gotchas

- **Drag in-window on pointer events via `usePointerDrag`** (tab strip,
  `ViewTabs`, chat groups, boards and tables via `useCardDrag`); HTML5 DnD only
  for leaving the window.
- **HTML5 drag needs `dataTransfer.setData()`** or WebKit cancels it silently,
  and `preventDefault()` on `dragstart` cancels it outright — see the drag-out
  in `app/src/components/harness/Timeline.tsx`.
- **A text selection pre-empts an element drag in WebKit** — hold it off with
  `DRAG_SURFACE`, or a press on a card's text selects instead of lifting.
- **`usePointerDrag` captures only past the threshold** — a capture on the
  press retargets the `click`, so a button inside a draggable header never fires.
- **Never `preventDefault()` on `pointerdown`** — WebKit builds `click` from a
  mousedown/mouseup pair, so the click dies; cancel `pointermove` instead.
- **`ViewTabs` swaps on the leading edge, `TopTabBar` on the centre** — with
  clamping, a centre never passes an equal-width end tab.
- **Finder drops are webview events**: listen on `getCurrentWebview()` (a window
  listener never fires) and measure the scale, since the position is in points
  despite `PhysicalPosition` (`app/src/hooks/useFileDrop.ts`).
- **No `md:` text size on a base field** — variants emit after plain utilities,
  so `md:text-sm` beats every call-site size; `input.tsx`/`textarea.tsx` carry
  one `text-[13px]`.
- **Every base reset in `index.css` goes inside `@layer base`** — unlayered CSS
  beats every utility, killing `border-*` and `select-*` app-wide.
- **Page zoom, never CSS `zoom`** — in a CSS-zoomed subtree WebKit mixes visual
  pointer coords with layout rects, breaking popups and drags.
- **No native date/time inputs** — segmented hover, an OS picker in Buddhist-era
  years, and a half-typed value reads empty. Use
  `app/src/components/projects/DateTimeField.tsx`; bare `type="time"` is fine.
- **`scrollbar-gutter` is a no-op in WebKit** — reserve the gutter with
  `overflow-y: scroll` (`page-scroll`, `GridTable`, the lightbox).
- **Centre overflowing content with auto margins and `flex: none`**, not
  `justify-content: center`, which makes the start-edge overflow unreachable.
- **Import `app/src/lib/tauriEvents.ts` before `./App`** — it patches an
  `unlisten` that throws under StrictMode's remount and leaks the subscription.
- **Never nest a `<button>` in a row that is a button** — WebKit drops the inner
  clicks; use `RowAction` (`app/src/pages/subject/LecturesPage.tsx`).
- **A `Link`'s ⌘-click reloads the whole webview** — leave modified clicks to
  `newTabClicks.ts`.
- **CodeMirror block decorations come from a `StateField`** — it throws for
  block widgets from a view plugin
  (`app/src/components/documents/editor/livePreview.ts`). Reveal-on-caret
  decorations rebuild on selection and focus changes, not only edits.
- **An editable widget must patch itself in `updateDOM`** — every edit rebuilds
  the field, and fresh widget DOM drops the caret in a focused table cell
  (`app/src/components/documents/editor/table.ts`).
- **`@codemirror/view` is patched** (`app/patches/`) — its scroller search took
  any box whose `scrollHeight` beat `clientHeight`, and under page zoom the
  editor's `overflow: visible` wrappers round a pixel over, so drag-select never
  scrolled `page-scroll`. The sticky toolbar's height is a `scrollMargins` top.
- **A window shortcut skips a key an editor already took** — `AppLayout`'s
  ⌘B (sidebar) checks `defaultPrevented`, since the note editor's ⌘B is bold.
  ⌘K is a menu item, so it never reaches a keymap: the palette's `menu-search`
  handler asks `linkInFocusedNote` first, and a focused note makes a link.
- **`data-tauri-drag-region` needs `core:window:allow-start-dragging`**
  (`app/src-tauri/capabilities/default.json`) or it silently does nothing.
