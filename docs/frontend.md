# Frontend

React 19 + Vite + Tailwind v4, hash-routed, Notion-style layout. UI
conventions (palette, shadcn, icons, no-toasts) are in the root `CLAUDE.md` —
this page is the structure.

## Where

| Piece | Location |
| --- | --- |
| Router + event bridge | `app/src/App.tsx` |
| Shell: sidebar + top tab strip | `app/src/layouts/AppLayout.tsx`, `app/src/components/sidebar/`, `app/src/components/tabs/TopTabBar.tsx` |
| App menu (⌘K / ⌘T / ⌘W and friends) | `app/src-tauri/src/menu.rs` |
| Search: what it finds, what a row does | `app/src/lib/search.ts`, `app/src/hooks/useSearch.ts`, `app/src/components/search/SearchList.tsx` |
| ⌘K palette (the dialog around that search) | `app/src/components/palette/CommandPalette.tsx`, `app/src/stores/paletteStore.ts` |
| Per-subject layout (underline tabs) | `app/src/layouts/SubjectLayout.tsx` |
| Subject tab pages | `app/src/pages/subject/` |
| Home (the launcher: composer, Today, Continue, Projects) | `app/src/pages/HomePage.tsx`, `app/src/components/home/` |
| New tab (the landing screen behind +, ⌘T and a fresh split) | `app/src/pages/NewTabPage.tsx` |
| Split panes (⌥⌘T), the seam, the focused half | `app/src/components/tabs/TabPane.tsx`, `app/src/stores/tabStore.ts`, `app/src/lib/tabRouters.ts` |
| ⌘-click → a tab of its own, app-wide | `app/src/lib/newTabClicks.ts` |
| Chat (a CLI agent's thread list, timeline, composer) | `app/src/pages/ChatPage.tsx`, `app/src/components/harness/`, `app/src/stores/harnessStore.ts`, `app/src/lib/harness.ts` |
| Calendar (month / week / upcoming, the new/edit event dialog) | `app/src/pages/CalendarPage.tsx`, `app/src/components/calendar/`, `app/src/lib/calendar.ts`, `app/src/stores/eventEditorStore.ts` |
| Projects (index, one board, a subject's tab) | `app/src/pages/ProjectsIndexPage.tsx`, `app/src/pages/ProjectPage.tsx`, `app/src/pages/subject/ProjectsPage.tsx`, `app/src/components/projects/`, `app/src/stores/projectsStore.ts`, `app/src/lib/projects.ts` |
| Tasks across every project, and the ones filed nowhere | `app/src/pages/TasksPage.tsx`, `app/src/components/projects/TasksBoard.tsx`, `app/src/components/projects/TasksTable.tsx`, `app/src/components/projects/universalTasks.ts`, `app/src/hooks/useTaskList.ts` |
| Writing a task down, and filing it later | `app/src/components/projects/NewTaskButton.tsx`, `app/src/components/projects/ProjectPicker.tsx` |
| The card/row drag every board and table shares | `app/src/hooks/useCardDrag.ts` |
| Overlap packing, shared by the week grid and the project timeline | `app/src/lib/lanes.ts` |
| Agent/model/reasoning picker (settings rows + composer) | `app/src/components/harness/ModelPicker.tsx` |
| A subject's own files (the Uploads tab) | `app/src/pages/subject/UploadsPage.tsx`, `app/src/lib/uploads.ts`, `app/src-tauri/src/files.rs` |
| Sync page + runner | `app/src/pages/SyncPage.tsx`, `app/src/lib/syncRunner.ts` |
| Settings | `app/src/layouts/SettingsLayout.tsx`, `app/src/pages/settings/` |
| Library counts, and the page that composes the two engine sections | `app/src/pages/settings/LibraryPage.tsx` |
| Parse engine, the MinerU token, the server address + status | `app/src/components/settings/ParserSection.tsx`, `app/src-tauri/src/parse/commands.rs` |
| Embedding backend, the index run, the re-index it costs, the run estimate and the spend guard | `app/src/components/settings/EmbeddingSection.tsx`, `app/src/components/settings/ReindexConfirmDialog.tsx`, `app/src/stores/indexStore.ts`, `app/src-tauri/src/embed/commands.rs`, `app/src-tauri/src/embed/estimate.rs` |
| Theme switch (light / dark / system) | `app/src/pages/settings/AppearancePage.tsx`, `app/src/components/settings/AppearanceSection.tsx`, `app/src/lib/theme.ts` |
| Academic term ordering | `app/src/lib/terms.ts` |
| Side panel (file/lecture preview) | `app/src/components/panel/`, `app/src/stores/sidePanelStore.ts` |
| In-app browser (route, toolbar, tab mirror, API) | `app/src/pages/BrowserPage.tsx`, `app/src/hooks/useBrowserTabs.ts`, `app/src/stores/browserStore.ts`, `app/src/lib/browser.ts`, `app/src-tauri/src/browser.rs` |
| Browser history, autocomplete ranking, site icons | `app/src/lib/browserHistory.ts`, `app/src/components/settings/BrowserHistorySection.tsx` |
| Browser preferences (search engine, where links open) | `app/src/stores/browserPrefsStore.ts`, `app/src/components/settings/BrowserSection.tsx` |
| Viewers | `app/src/components/files/PDFViewer.tsx`, `app/src/lib/pdfjs.ts`, `app/src/components/files/FileViewer.tsx`, `app/src/components/lectures/LecturePlayer.tsx`, `app/src/components/lectures/ChaptersPanel.tsx`, `app/src/components/lectures/ReadingList.tsx` |
| Parse state: the words, and the row badge / viewer notice | `app/src/lib/parseState.ts`, `app/src/components/files/ParseState.tsx` |
| shadcn components (source, editable) | `app/src/components/ui/` |
| Table chrome: view tabs, the quieter pill strip, footer pagination | `app/src/components/ui/ViewTabs.tsx`, `app/src/components/ui/PillTabs.tsx`, `app/src/components/ui/TablePagination.tsx` |
| Zustand stores | `app/src/stores/` |
| Hooks | `app/src/hooks/` |
| DB access (tauri-plugin-sql) | `app/src/lib/db.ts` |

## Routes

The route table is `app/src/routes.tsx`, and each **pane** builds its own
memory router over it (`app/src/components/tabs/TabPane.tsx`) — the shell is
above all of them, so there is no one router to name. A pane is a tab, or one
half of a split tab — ⌥⌘T, described under **How it connects** below. `/` is **Home**; it used to
redirect to `/chat`. Then `/chat`, `/calendar`, `/projects`,
`/projects/:projectId` and `/projects/:projectId/tasks/:taskId`, `/tasks` and
`/tasks/:taskId`, `/subjects`, `/subjects/:subjectId` (SubjectLayout →
overview / modules / downloads / uploads / lectures / announcements /
assignments / discussion / projects), `/subjects/:subjectId/file` and `/lecture` (the side
panel promoted to a full Notion-style page, outside SubjectLayout on purpose),
`/sync`, and `/settings/*`. Legacy routes (`/lectures`, a subject's `files`
tab) redirect.

A project lives at the top level rather than under its subject even when it has
one, because it can have none: the subject's Projects tab and the index are two
filtered views of one list, and both link to the same `/projects/:projectId`.
Its name rides in the route's query (`?n=`, `projectHref`) for the tab strip's
sake — `tabInfo` titles a tab from the path alone and has no project list to
look one up in, the same trade `/lecture` makes with `?t=`.

**A task is a page too**, one level further down, and it carries its title in
`?n=` for the same reason (`taskHref`). A *filed* task sits under its project
rather than at `/tasks/:id` because the page cannot draw anything without the
project: a status is a column on *that* board, and `moveTask` has to be handed
an id the board actually has. Both pages re-`navigate(…, { replace: true })` to
their own href after a rename, so the tab you are looking at re-titles itself
rather than waiting to be reopened — which is the cost of titling from the
path, paid at the one moment it shows.

**`/tasks` is every task there is**, across every project and including the
ones that belong to none (`app/src/pages/TasksPage.tsx`) — a board of the four
columns every project is born with, or a flat table, over `useTaskList` rather than
`projectsStore`, which holds one open project and is the wrong shape for a view
that spans them all. It carries a second, nested route: **an unfiled task's own
page is `/tasks/:taskId`**, the same `TaskPage` component with no project
segment to nest under, reading its board as `boardOf(null)`'s default four. So
`taskHref` has two shapes, and `tabInfo` tests the task route *before* the list
route because one is a prefix of the other. What this view gives up for
spanning projects is manual order — `position` only orders one project's column
— so its columns are sorted, a same-column drag is a no-op and the table sorts
by header; [projects.md](./projects.md#the-universal-view) is where that rule
and `refileTask` are written down. The page's chrome is a fixed `h-12`
toolbar, then a row carrying the deliberately quiet `PillTabs` strip for the two
views on the left and the four filters — status, project, subject, due, opening
on Todo — on the right, so switching view cannot jolt the work below. The
`All tasks · Unfiled` scope strip that used to sit above the toolbar is gone:
Unfiled is a value of the project filter now.

**`/chat` carries `?n=` on the same terms**, and it is the one page that writes
its own query rather than being linked with it: the conversation is picked
inside the page, not by the link that opened it, so `ChatPage` replaces its
route as the open thread changes and again when the model's name for the
thread lands (`docs/harness.md`). With nothing open there is no `?n=` and the
tab is plainly *Chat*.

## How it connects

- **Settings → Library offers the parser, and switching it costs nothing**
  (`ParserSection`). Two engines: MinerU's cloud service, or a MinerU server
  the user runs on their own Mac. Unlike the embedding control directly below
  it, the change is **not destructive** — both engines write the same artifacts
  at the same `PARSER_VERSION`, nothing is re-parsed and nothing is thrown
  away — so the select is a plain `onValueChange` with **no confirmation
  dialog**, and one must not be added for symmetry with `ReindexConfirmDialog`.
  The engine list, its labels and any refusal come from Rust, so the page
  cannot offer what the backend would refuse. Three sub-rows appear
  conditionally: the MinerU token **only under Cloud** (a key field under a
  backend that cannot use it is where people paste secrets), and the server
  address plus a live status line only under Local. That status line has four
  states, not two — reachable, unreachable, a server that answers but speaks
  MinerU 4's V1 API, and *not asked yet*, which must never be drawn as a
  failure. Its sentence is Rust's, because Rust distinguishes causes the page
  cannot see. The privacy boundary is still written as a fact rather than an
  offer, now one per engine: uploaded to MinerU's PRC-hosted OSS storage, or
  nothing leaves the machine. See [parsing.md](./parsing.md).
- **Settings → Library also owns the embedding backend, and changing it is
  destructive on purpose.** One engine is selected (`embed` in the `settings`
  table, read by `embed_config()` in `app/src-tauri/src/embed/mod.rs`) and
  search runs against that one — no fallback between engines, no two spaces
  fused at query time. Vectors from two models share a table, a width and a dot
  product and share no geometry, so a mixed index returns a confident ranking of
  unrelated pages, which is worse than an error. `embed_set_engine` therefore
  clears every stored vector, the per-file `embed_status` and the `.emb.json`
  records on disk in the same call, in that order, writing the setting **last**
  so a failure can cost a re-index but never leave the setting naming one space
  while the table holds another. The page says so before the change, in the
  numbers the index actually holds (`ReindexConfirmDialog`), and skips the
  dialog entirely when there is nothing indexed to lose. The engine list, its
  labels and the reason an engine is unavailable come from Rust, so the page
  cannot offer what the backend would refuse. The *embedding* `Engine::Local`
  is real in that seam and has no server to talk to yet, so it is listed and
  disabled in the menu rather than hidden — which is the one place these two
  sections differ, because the parser's local engine does have a server and is
  always selectable. `unavailable_reason` is still Rust's, and Rust still
  refuses the engine in those words; the section no longer *prints* it under
  the control, because a paragraph about a control nobody can use was crowding
  out the numbers that decide whether to press Index.
- **The same section starts and stops the index run**
  (`app/src/stores/indexStore.ts`). Until it existed the index could only be
  built by `oculus index` from a terminal: `embedFile` was written and wired to
  Rust and had no caller, so a library could sit permanently unsearchable with
  nothing in the app admitting it. The store is **a queue with one worker**,
  fed from three places — the Index button (the whole outstanding backlog), a
  parse finishing (one file, if a key is stored), and a row's retry in the
  File Activity table. One file at a time, because the backend paces itself
  against the account's per-minute ceiling and a second loop would only split
  the same tokens between two. The queue itself is a module-level array rather
  than store state: zustand replaces state, so a worker holding a snapshot
  would drain a copy and never see a file appended mid-run.
  A run can legitimately last hours, which shapes the rest: progress names the
  file it is on **and the page inside it**, the bar's fraction includes that
  page term (a 200-page deck on the free programme is an hour in which a
  file-counting bar does not move), the state lives in the store so it survives
  navigating away, and **stopping is cooperative and lands between files**,
  never mid-document, because abandoning one mid-flight would waste the quota
  already spent on its pages. Per the house rules there is no toast and no
  bottom bar: the page that owns the index shows the detail, and a spinner on
  the sidebar's Sync row is the whole of its presence elsewhere.
- **`ready` is the one switch in front of all of it.** It is
  `embed_settings`' `credentials_ready` and the selected engine's
  `available`, asked in `useBackendEvents` at startup and again whenever a key
  is saved or cleared. Nothing queues itself while it is false — an app that
  embedded before it had somewhere to embed to would write one failed row per
  parsed file — and the pipeline table hides its third stage for the same
  reason, since a permanently grey dot reads as a stall rather than as an
  option nobody turned on.
- **The stats rows distinguish the two spaces, and the labels had to change to
  do it.** `IndexStats.model` is the space this build *writes* — `stats` reads
  it off the seam's constants so it can answer before a key exists — not the
  space the stored vectors are in. Shown as "Vector space" it claimed the
  library held Voyage vectors while every one of them was Qwen, so it is
  "Search space" now.
- **What is *not* indexed is one row, not the stale-vector essay it used to
  be.** `pages_stale` and `stale_models` are still on `IndexStats` and still
  the honest account of a library embedded by a retired model — but as a row
  *and* a paragraph on this page they answered a question nobody was asking
  here. "Not indexed" counts **files** from the same predicate the run walks
  (`getUnembeddedPdfs`), which already follows the current space, so a retired
  model's vectors show up in it as what they are. The stale numbers still reach
  a human through `oculus status` and through the re-index confirmation.
- **"Voyage plan" is allowed to say it does not know.** Voyage publishes no
  usage endpoint, so the plan is the tier the rate-limit detector learned from
  429 bodies during a run (`embed/voyage/ledger.rs`). An install that has never
  indexed anything has never had the chance to learn it, and prints "Not
  measured yet" rather than a guess about somebody's billing.
- **The allowance is a meter, not two rows and a paragraph.** Spent, what this
  run would add, and where the spend guard stops it are three facts about one
  quantity, and a percentage set against a number you cannot see means nothing
  until it is a mark the fill is heading for. Same grammar as Settings →
  Storage: a stacked fill on a track with a `destructive` hairline for the
  limit. The caveat that every figure is Oculus's own count — `voyage-usage.json`,
  not Voyage's books — is a `StatRow` `hint` tooltip, because a sentence under
  every qualified figure is how a settings page becomes an essay.
- **The run estimate is measured, not phrased**, and its headline is the
  *saving*. `embed_estimate` opens every outstanding PDF's page boxes with
  pdfium, bills them the way Voyage does and packs them with the batcher the
  run itself uses. The counter-intuitive fact it exists to deliver: the 150B
  free pixels are granted to **every** account, so a payment method does not
  make a coursework library cheaper — it is already free — it makes it two
  hundred times faster. "About 18 hours" is a number to sigh at; "save about 18
  hours, at no extra cost", with both times beside each other and a link to the
  dashboard, is the same measurement and a number to act on. The upgrade line
  is shown only over a *measured* tier, never an `assumed` one, because
  pitching an upgrade off a guess is a different thing entirely.
- **`embed_estimate` is a separate command from `embed_settings` because it is
  seconds slow** — pdfium over a whole library — and the section draws the rest
  of itself while it runs. It also runs **one sweep at a time**: pdfium is a
  single session process-wide, so two concurrent calls queue rather than
  parallelise, and StrictMode's double-invoked mount effect was asking for two
  every time the page opened. A request arriving during a sweep is remembered,
  not dropped, so changing the spend limit always re-measures against the
  setting actually in force. See [retrieval.md](./retrieval.md).
- **The spend guard is a brake on an unattended run**, not a preference. A
  free-tier index of this library is most of a day, so nobody is watching when
  the grant runs out; the percentage is enforced in the reservation the client
  takes before every request (`UsageLedger::budget`), never in this page.
- Token save/remove invokes Rust keychain commands, never DB writes. Saving
  checks the token against MinerU first, so a bad or expired one is named at
  that moment, and it lifts the app-wide parse latch — the in-process client
  keeps no rejection state of its own, so nothing else would
  ([parsing.md](./parsing.md)). A token MinerU refuses mid-parse shows as
  Expired, read from that latch in `parseStore` rather than polled: the state
  is session-scoped, because the next parse reads the keychain afresh.
- A lecture row in `app/src/pages/subject/LecturesPage.tsx` carries its own
  controls: download when there is no file, percentage plus a cancel while one
  is transferring, and a ✓ plus a delete once it is on disk. Cancel disappears
  during `trimming` — that phase is ffmpeg on a complete file, with no
  transfer left to stop. Deleting asks first, because a download is minutes of
  transfer, and says what it keeps. They are `div`s with a button role
  (`RowAction`), not `<button>`s: the row itself is a button, nesting one is
  invalid HTML and WebKit drops the inner element's clicks.
- Settings → Appearance owns the theme. `applyTheme` in
  `app/src/lib/theme.ts` is the only writer of both the `.dark` class and the
  stored preference, so the control keeps no state of its own. `system`
  resolves the OS preference *and* keeps following it — `watchSystemTheme`,
  started once in `App.tsx`, re-applies on change so an evening switch to dark
  lands without a relaunch. An explicit light/dark choice ignores the OS.
- Subject lists order by term through `app/src/lib/terms.ts`, not by
  comparing term names as text. Canvas names them `"2026 Summer Term"` /
  `"2026 Semester 2"`, and `Su` > `Se`, so a plain string sort files Summer
  *last* in its year when it runs first. A month-named term (`"2026 June"`)
  ranks as the term it runs inside — winter — rather than as unknown, which
  sorts after every real term. `TERM_RANK_SQL` inlines the same ranking as a
  `CASE` for the query in `getSubjects`.
- **`getSubjects` derives `is_current`; it does not read the column.** The
  stored flag is stamped at sync time by `list_courses` in
  `app/src-tauri/src/sync.rs`, which takes the newest term with `.max()` over
  term *names* — so a single summer enrolment outranks the semester actually
  being studied and marks every real subject past. Recomputing at read time
  costs one pass, cannot go stale between syncs, and keeps the current/past
  split, the chat subject picker and the project pickers honest. The column
  itself is written correctly too — `app/src-tauri/src/terms.rs` is the same
  ranking, and the CLI reads the column — but deriving here means a database
  stamped by an older build is not believed.
- **Backend events are the write path.** `app/src/hooks/useBackendEvents.ts`
  (mounted once in `App.tsx`) listens for scrape/parse/auth Tauri events,
  upserts SQLite through `app/src/lib/db.ts`, and updates the stores. Pages
  read from the DB and stores; they do not talk to the scraper directly.
  The exception is `harness-event`: its rows are already written by Rust,
  so the hook only hands it to `harnessStore`, which keeps the streaming
  text and patches tool rows — see [harness.md](./harness.md). It is
  app-level rather than page-level because a thread keeps running on
  another page and the sidebar's Chat row spins while one does.
- **`app/src/lib/tauriEvents.ts` is imported for its side effect only, before
  `./App` in `app/src/main.tsx`, and is not dead code.** Tauri's injected
  `unlisten` reads `listeners[eventId].handlerId` after checking only that the
  *event* has a listener map, not that this id is still in it — so an unlisten
  that lands before its own registration script has run throws, and because the
  throw precedes the `plugin:event|unlisten` call the subscription leaks on the
  Rust side too. StrictMode's mount/unmount/remount opens that window on every
  listener in the app. The module swallows the miss so the IPC half still runs.
  Import order is the contract: it has to be evaluated before the first
  `listen()`.
- **Two error boundaries, plus a last-resort overlay**
  (`app/src/components/ErrorBoundary.tsx`). The root route's `errorElement`
  catches a page that throws, so the crash takes that pane and leaves the
  shell, the other pane and every other tab alive; navigating clears it.
  `ErrorBoundary` wraps the shell in `app/src/App.tsx`, which is above every
  router, and offers a reload. Neither catches an event handler or a rejected
  promise — those still reach the fullscreen overlay in `app/src/main.tsx`,
  which paints whatever threw because a release webview has no console. Its
  `BENIGN` list is for throws that are noise: the ResizeObserver frame
  notice, and pdf.js's global `selectionchange` handler walking off the end of
  a text layer it detached itself.
- In the app it is the **frontend** that owns scrape-table writes (the Rust
  engine only emits events); the CLI writes the same rows itself via
  `app/src-tauri/src/store.rs`. Change a table's shape and both writers must
  move together, plus the migration in `app/src-tauri/src/lib.rs`. The
  `projects` tables are the same pair, one layer up:
  `app/src/lib/projects.ts` in the app, `app/src-tauri/src/projects.rs`
  headless ([projects.md](./projects.md)).
- **A project write refreshes through an event, not through the store.** Every
  write in `app/src/lib/projects.ts` fires `PROJECTS_UPDATED_EVENT` on
  `window`; `projectsStore`'s write wrappers deliberately do not re-read, and a
  component showing project data subscribes to that event and calls `reload` —
  the contract the calendar already has with `CALENDAR_UPDATED_EVENT`.
  Re-reading in the wrappers *as well* cost one drag two task reads plus a list
  read and let the two paths land out of order, but the real reason is that
  there is a second writer with no access to this store at all: the chat agent
  runs `oculus project` / `oculus task` in its own process. `useBackendEvents`
  watches the harness stream for a finished tool call whose command names one
  of those and fires the very same event, so an agent's write and a click
  arrive by one door and nothing can work for one and not the other.
- **`packLanes` (`app/src/lib/lanes.ts`) is shared by the week grid and the
  project timeline.** It came out of
  `app/src/components/calendar/WeekView.tsx` when the timeline needed the same
  overlap packing, and it is generic over the item rather than typed to
  `CalEvent` because the two callers measure a span in different units — a
  class in milliseconds, a task bar in pixels — and the packing never needs to
  know which.
- **`sync_runs` is the only sync clock.** A subject's `last_synced_at` is not
  stored — `getSubjects` in `app/src/lib/db.ts` (and the CLI's
  `store::subjects`) derives it from the latest *completed* run whose
  `subject_codes` include the subject, so the subject list can never disagree
  with the history table and interrupted runs never count as a sync.
- `SubjectLayout` resolves the subject once and hands it to tab pages via
  outlet context — tab pages must not re-fetch it. Its underline tabs are a
  **sideways scroller**, not a row that gets clipped: nine tabs already crowd
  the centred column and the card is narrower still with the side panel docked
  open, so the row scrolls with its bar hidden, a fade over each live edge (the
  sidebar's affordance) and the active tab scrolled into view. The `-mb-px`
  that lands the active underline on the header's border sits on the scroller,
  not on the tabs: `overflow-x` clips on both axes, so inside it the underline
  would go with it.
- **Uploads is the one subject tab whose rows nothing scraped**, and it is
  thin because it has to add almost nothing. `import_uploads` copies the picked
  bytes into `courses/<code>/uploads/` and converts Office documents there the
  same way a download is converted; from that moment the file is an ordinary
  library file, so the parse sweep, the embed hop, ⌘K, semantic search and the
  chat agent's `courses/` all reach it without knowing it was never on Canvas.
  What `app/src/lib/uploads.ts` adds is the `files` row and the first parse
  kick — **in that order**, because `embedAfterParse` in `useBackendEvents`
  resolves a finished parse back to a file by `(subject_id, relative_path)`,
  and a file parsed before its row exists would never be indexed.
  Three choices are worth keeping. The picker and the drag both hand back
  **paths**, never bytes, so nothing large crosses the IPC bridge. A name
  already taken steps aside (`notes.pdf` → `notes-2.pdf`) rather than
  overwriting, except for byte-identical content under the same name, which is
  the same file again and keeps its parse; adding the wrong file can therefore
  never destroy the right one. And a Finder drag is a **native window event**
  (`onDragDropEvent`) rather than an HTML drop — WebKit never sees those files
  — which means it is the *window's* event, not the page's: every tab stays
  mounted, so the page scopes it with `useTabActive` or a backgrounded Uploads
  tab would claim a drop meant for whatever is in front.
  Deleting is the only destructive control in the library, and its guard is
  `is_upload_rel` in `app/src-tauri/src/paths.rs`, not a confirmation dialog:
  only a path under some subject's `uploads/` can be removed at all. It takes
  the converted PDF and `purge_parse_artifacts` with it, and the `pages` rows
  too — which `deleteFileRow` writes out by hand, since nothing sets
  `PRAGMA foreign_keys=ON` and the declared cascade is documentation rather
  than a guarantee.
  **A delete cannot fully clean up after itself, so the next write does the
  rest.** The parse and embed skip checks — `parse_mode` and
  `embed::is_embedded` — read those artifacts rather than the PDF's bytes, and
  a parse still in flight when the delete lands writes its `{stem}.md` out
  afterwards, beside a PDF that is gone — so a later upload handed that freed
  name would inherit a stale parse. `store_upload` therefore purges the
  artifacts on the name it is about to use whenever the bytes are not
  byte-identical to what is there, which is the one case they provably are its
  own. That purge is also why `purge_parse_artifacts` takes `{stem}_images/`:
  those figures are referenced only from the markdown it deletes, so keeping
  them is not a fallback, and both parse tiers rebuild the directory anyway.
- **The shell is furniture around a floating document.** `AppLayout` puts the
  sidebar and `TopTabBar` straight onto the window ground and renders content
  as an inset rounded card, so neither needs a divider of its own. Two
  consequences: the sidebar keeps its left inset when collapsed because the
  row uses `gap-2` rather than a margin on a zero-width element, and the tab
  strip's tabs are pills rather than browser tabs merging into the page.
- **Full-page views scroll through `page-scroll`**, the utility in
  `app/src/index.css`, not a bare `h-full overflow-y-auto`. Because the
  scrollbar is a classic one that takes width, a page that only overflows
  sometimes — a collapsible group opening, a list growing under a live sync —
  would otherwise jog sideways the moment it does; the utility reserves the
  gutter on both edges up front, which also keeps content centred in the
  card. The bar's track is inset from both ends (`::-webkit-scrollbar-track`)
  so the thumb stops clear of the card's rounded corners instead of being
  clipped into a stub.
- **`MD_COMPONENTS` is sized for a document; a panel overrides it in the
  cascade.** One markdown renderer serves the file viewer, the calendar popover,
  the chat timeline and the player's dock
  (`app/src/components/markdown/MdComponents.tsx`), and its sizes are baked in
  as utilities. A reply is a smaller register than a lecture page, so it goes
  through `CompactMd`, which wraps it in `.md-compact`, and
  `app/src/index.css` scales the type down under that class alone. (The Read
  tab's lines and chapter summaries sit inside the buttons that seek, so they
  take `InlineMd`, which flattens block grammar and is sized at the call site.) That block
  is the one rule in the file that is deliberately **unlayered** — layer order
  beats specificity, so the same rules inside `@layer base` would lose to the
  very utilities they exist to override. Base *resets* still belong in the
  layer; this is the inverse case.
- **A ```mermaid fence is drawn, and the whole feature is that one fence**
  (`app/src/components/markdown/Mermaid.tsx`, reached from `MD_COMPONENTS.pre`
  so every markdown surface gets it at once). Four things about it are not
  obvious from the code:
  - **It is caught at `pre`, not at `code`.** An SVG inside a `<pre>` inherits
    `white-space: pre`, which turns the gaps between mermaid's own elements
    into rendered whitespace. The `<pre>` that would have been is passed down
    as children and is what shows until the diagram renders — during streaming
    and, if the source never parses, for good. A broken fence therefore
    degrades to exactly what it looked like before this existed; mermaid's own
    red error card is switched off (`suppressErrorRendering`).
  - **The palette is read, not named.** Mermaid wants colour *values*, so the
    component resolves `--diagram-*` from `index.css` with `getComputedStyle`.
    Those aliases exist because **Tailwind v4 only emits a theme variable that
    something references** — measured, `--color-surface` and the
    `--color-chart-*` family come back as empty strings otherwise, and an
    empty string is "Unsupported color format" and no diagram. The `var()`
    references in that block are what pin them. Re-read on every render and
    re-rendered on a `.dark` flip, via `subscribeDark` in `lib/theme.ts`.
  - **`htmlLabels: false` goes at the top level of the config, and it is the
    single most load-bearing line in the file.** Under `flowchart` it is
    silently ignored (measured, mermaid 12.0); at the top level mermaid
    propagates it and labels come out as SVG `<text>` instead of HTML in a
    `<foreignObject>`. That matters because **this app runs the webview at a
    page zoom** (`setZoom`, `AppLayout`) and mermaid lays an HTML label out by
    setting `white-space: nowrap` with a `max-width`, measuring, and switching
    to wrapping only when `bbox.width === width` — an exact float equality
    against the unscaled constant it just set. Page zoom scales
    `getBoundingClientRect`, so at any zoom but 1 the equality is false, the
    label never wraps, and every caption longer than its box is cut off
    mid-word, with the edge labels' backgrounds mis-sized to match so the
    connector line draws straight through their text. Measured at zoom 1.3:
    four labels, four clipped; the same diagram at zoom 1.0 was perfect, which
    is what made it look like it worked. An SVG label is laid out by mermaid
    itself and has neither problem — and cannot be reached by `.md-compact`'s
    markdown rules either. The `.diagram` guard in `index.css` stays for the
    diagram types that have no such switch.
  - **The label size is what is bounded, not the box — and that is the whole
    sizing policy.** Mermaid fits the picture to its container, so the *column*
    decides how big the type is. Measured in the 760px chat page: the
    twelve-node `graph TD` (natural 360 × 795) renders at scale 1.0 with 13px
    labels while a five-column `graph LR` (natural 1460 × 115) is fitted to
    0.846 — and before this, to **0.50, putting its labels at 6.5px**. One
    reply, two diagrams, one at body-text size and one an unreadable smear,
    with nothing in the fence to explain the difference. There *was* a floor
    and it was in the wrong unit: a flat 520px, which in a 760px column never
    fires at all, and which for a wide diagram would have meant scale 0.34 —
    worse than the bug. A floor has to be proportional to the diagram to say
    anything about its type. So `.diagram > svg` is
    `width: clamp(--diagram-min, 100%, --diagram-max)`, and the whole policy is
    one sentence: *a diagram is drawn as large as it can be without passing the
    column, the size it was drawn at, or `TARGET_HEIGHT_PX` — and never so
    small that a label drops under `MIN_LABEL_PX`.* `Mermaid.tsx` computes both
    bounds because it is the only place that knows mermaid's output *and* the
    size it told mermaid to draw labels at. `--diagram-max` starts at the
    natural width and walks down the band for a **tall** diagram until it meets
    the height target or runs out of band, which is how a tall one lands at
    11px and a short one at 13px with neither being a special case;
    `--diagram-min` is the hard floor, past which the diagram **overflows**
    instead of shrinking. Horizontal scroll is cheap and illegible type is not.
    Measured across both column widths and four diagram types, every label lands
    in [11px, 13px]. Bounding the label bounds the shapes with it, which is why
    there is no separate cap on a node: mermaid sizes every box from its own
    label and padding.
  - **`layout: "dagre"` is what makes the spacing keys work, and it is the one
    lever that shrinks a diagram without shrinking anything in it.** Measured,
    the twelve-node flowchart's 791px is 481px of node boxes — 165 of which is
    a single decision diamond — and **310px of gaps**, seven ranks at ~44px. So
    `rankSpacing` is where the size is. It does nothing on its own: with the
    top-level `htmlLabels: false` the renderer needs,
    `flowchart.rankSpacing`/`nodeSpacing` are read and discarded (mermaid
    12.0.0, byte-identical output at 0, 20 and 200) while `padding` in the same
    object applies. Naming `layout` routes flowcharts through the pluggable
    layout loader, where they take effect. The two keys ship together —
    `FLOWCHART_LAYOUT` in `Mermaid.tsx` says so — and dropping `layout` makes
    the spacing silently stop meaning anything. At ~half mermaid's defaults the
    flowchart goes 360 × 791 → 352 × 665 and a five-column `graph LR` 1450 →
    1276 wide, which takes pressure off the horizontal overflow too.
    `layout: "dagre"` is flowchart-only: sequence and pie render identically
    with it set, and labels stay SVG `<text>`, which is the thing that must not
    regress.
  - **Tall is allowed; endless is not.** Nothing caps how tall mermaid draws a
    `graph TD` — the flowchart above is 795px, which at the app's page zoom is
    very nearly the whole window. Scaling it to fit is the wrong fix for the
    same reason as everything above (scale ~0.55, 7px labels), so `.diagram`
    caps its own height at `min(60vh, 30rem)` and the picture keeps the size it
    was drawn at inside that: it scrolls, or it opens. 80vh was tried first and
    is a takeover rather than a figure — at this app's page zoom it is the whole
    pane — and the `rem` half stops a tall window handing a diagram 700px just
    because it can. Most diagrams now come in under the cap on their own,
    because the spacing above took ~16% off the height before this rule sees it.
  - **Where the picture runs out, it fades.** A capped scrolling box
    guillotines a diagram mid-node and the cut reads as the end of it, so each
    edge with something behind it is softened — four `--fade-*` custom
    properties written straight onto the element from `Mermaid.tsx` on every
    scroll, 1 where there is more to see and 0 where there is not, so an edge
    with nothing behind it stays hard. A **mask**, not a gradient overlay, and
    that is the point: a mask fades to transparent and needs no opinion about
    what is behind the diagram, where an overlay would have to name a colour and
    this component renders on `card` in a reply and `background` in the lecture
    dock, in both themes. The two gradients compose with `mask-composite:
    intersect` so both axes apply; that and `calc(var())` inside a gradient stop
    were confirmed in the app's own WKWebView rather than in a Chromium tab,
    which matters because the in-app browser used for UI checks is Chromium. The
    stops sit 1px outside the box because at exactly 0/100% the unfaded case
    still antialiases its last pixel and takes the hairline border off an edge
    node. The same scroll pass measures `pannable`, and its `> 1` tolerance is
    load-bearing: a clamped, scaled width leaves sub-pixel slack that at `> 0`
    reads as a permanent fade on a diagram with nothing behind it.
    Mermaid's own spacing would have been the cheaper lever for the height and
    is not available: with the top-level `htmlLabels: false` the renderer
    needs, `flowchart.rankSpacing` and `nodeSpacing` are ignored outright
    (measured on mermaid 12.0.0 — byte-identical output at 20 and at 100),
    though `padding` and `diagramPadding` do still bite.
  - **A figure centres, and the centring cannot be `justify-content`.** A
    narrow `graph TD` shoved against the left margin of a 760px reply reads as
    a column of boxes rather than a figure. But centred content that outgrows
    its scroller overflows equally in both directions and the overflow before
    the start edge cannot be scrolled to, so the left end of a wide diagram
    would be unreachable. An auto margin on a *flex item* is defined to absorb
    only positive free space — an overflowing item ignores it and overflows in
    the end direction — so `display: flex` on `.diagram` plus `margin-inline:
    auto` and `flex: none` on the SVG centres a small diagram and pins a wide
    one with one rule. Measured: 146.9px of margin each side on the narrow
    case, `0px` on every overflowing one. `AppLayout` and the lightbox learned
    this the hard way; this is the third place it applies, and `flex: none` is
    there for the same reason it is in the lightbox — a flex item's default
    `flex-shrink: 1` would take the clamp's floor straight back out.
  - **A press pans the figure; only the expand control opens it.** The inline
    scroller used to treat a press that moved less than a few pixels as "open
    the lightbox", which made every attempt to select a label or nudge a wide
    diagram sideways end in a full-window takeover. There is no click gesture
    on the picture now, the hover-revealed control in the corner is the whole
    door, and the grab cursor appears only when the box actually clips the
    picture — measured against `scrollWidth`/`scrollHeight` behind a
    `ResizeObserver`, since the column resizes under it.

  **The opened diagram is `DiagramLightbox.tsx`**, and the thing to know about
  it is that **panning is the container's own scroll and only the scale is a
  transform**: the picture sits in an `overflow-scroll` box, as `PDFViewer`
  lays its pages out, so two-finger panning, momentum, scrollbars and keyboard
  scrolling all arrive for free, and plain scrolling stays the pan; what a zoom
  changes is a `scale()` on an SVG host of fixed size, inside a layout box
  carrying `natural × zoom` so the scroll extent still tells the truth. That
  split is not the CSS-`zoom` mistake `AppLayout` made once — under CSS `zoom`
  WebKit reports pointer coordinates in visual pixels and element rects in
  layout pixels, where under a `transform` both are visual, so the anchor maths
  measures one space. It is also 3× cheaper per frame (0.3ms against 1.0ms),
  because the SVG's own layout never re-runs.

  **The zoom is smoothed on the render side, and that is the fix for a judder
  that two passes of event bookkeeping did not reach.** Measured, a zoom frame
  costs ~1ms and not one frame is dropped at 120Hz — so a stuttering zoom was
  never the drawing being slow, it was the *number* arriving in steps: one
  mouse notch is a ±120 lurch, `gesturechange` delivers a quantised `scale`, a
  button is a single 1.25× jump. So every input — wheel, pinch, buttons, keys,
  double-click — only moves a **target**, and what is painted eases toward it
  each frame with a 45ms time constant, which is smooth whatever arrived and
  however often. Two consequences worth knowing: every request composes on the
  target rather than on what is on screen, so a burst of wheel events or four
  impatient clicks on `+` add up instead of overwriting each other; and the
  cursor anchor is stored as a diagram point plus a screen point, which is
  scale-free, so the same anchor is re-measured and re-applied on every frame
  of the ease rather than predicted once — a scroll the browser clamped at its
  bounds simply corrects itself on the next frame. The wheel's gain is
  separately a bug that was there: `exp(-delta * 0.01)` turned one mouse notch
  into a 1.65× jump, and 0.0022 with a per-event ceiling makes it ~1.3×.
  Nothing in the viewer re-renders for a zoom — the percentage is written
  straight to its text node, and the only React state is which end of the range
  the zoom is resting on, so the toolbar can grey the button that would do
  nothing. (`PDFViewer` no longer lays its
  own pages out at all: it mounts pdf.js's `PDFViewer` component from
  `pdfjs-dist/web/pdf_viewer.mjs` — Firefox's reader minus its chrome — which
  owns the page window, the text layer and the zoom. This file used to
  hand-roll each of those over `react-pdf`, and each hand-rolled version had a
  visible bug: a page window sized from a guessed aspect ratio, a
  `transform: scale()` preview folded into a real raster on an idle timer, and
  an anchor measured across two frames that snapped the document toward the
  middle at the end of every pinch. The library's counterparts are
  `updateScale({ origin, drawingDelay })` — which writes the scale to a
  `--scale-factor` CSS variable for the compositor and re-rasterises once, the
  same preview/commit split, and holds the pinched point still — and its own
  virtualisation. The lightbox needs none of it: an SVG costs nothing to
  rescale.) A
  press that lands on a label selects it rather than panning, so the I-beam the
  cursor shows there is the truth.

  **`PDFViewer` draws the same line, and a toolbar toggle moves it.** A drag
  pans unless it starts on a word, with a `Select`/`Pan` pair beside the layout
  modes for the page too dense to have a margin left to grab, and ⌥ as the
  one-off override. The test is the text layer's *spans*, never the layer: it
  is `position: absolute; inset: 0`, so asking whether a press landed in
  `.textLayer` answers yes over every margin and leaves nothing pannable.
  Switching tools switches the layer's `pointer-events` off rather than
  out-specifying it, which settles the cursor, the selection and that span test
  in one move — `pdf_viewer.css` is imported unlayered, so its `cursor: text`
  on the spans beats any utility, which is exactly what makes the I-beam honest in
  `Select` and is why `Pan` cannot simply paint over it. Space is not the
  modifier for this, however conventional: `LecturePlayer` binds it on `window`
  for play/pause, and a file panel open beside a playing lecture would fire
  both. Zoom is pdf.js's, but the
  *gestures* are still this app's, because the library binds neither desktop
  path — Firefox's `app.js` does, and that is the chrome we are not shipping.
  One pinch arrives as `ctrlKey` wheel events **and** as real `gesture*`
  events, and answering both made the zoom creep and stall, so a flag gives the
  gesture the zoom while the fingers are down and leaves the wheel path to
  ⌘-scroll and a real mouse. Each event hands `updateScale` a ratio against the
  previous one rather than an absolute scale, so a burst composes instead of
  racing; no zoom number lives in this file at all.

  **pdf.js is loaded on demand** (`app/src/lib/pdfjs.ts`), and that shape is
  forced rather than chosen. `pdf_viewer.mjs` is a webpack bundle that treats
  the core as an external *global*: its module body opens with
  `const {…} = globalThis.pdfjsLib`, so the global has to exist before that
  module is **evaluated**, which no arrangement of static imports in one file
  can manage — they are all hoisted above any assignment. Awaited dynamic
  `import()`s make the order real, and pay a second time by keeping the core,
  the viewer layer and its stylesheet — 200 KB gzipped, before the worker — out
  of the entry chunk until a PDF is actually opened. The stylesheet carries one
  trap with it: it declares `color-scheme: light dark` on `:root`, a whole-app
  statement smuggled in by one view, which on a Mac set to dark would turn
  native scrollbars and form controls dark underneath a light app — dark mode
  here is the `.dark` class and nothing else. `index.css` pins the scheme to
  that class, one element more specific than `:root` so the fix does not depend
  on which stylesheet Vite emits last.

  Five things bit while
  building it, all measured:
  - The box is a flex item, and a flex item's default `flex-shrink: 1` takes
    every pixel of new width straight back out: the zoom read 165% while the
    picture had not moved. `shrink-0`.
  - Mermaid writes its natural width as an **inline** `max-width`, which
    outranks any class, so `max-w-none` without a `!` silently capped the
    picture at natural size however far it was zoomed.
  - Centring the content with `justify-content` makes the overflow before the
    start edge unreachable — the top-left of a zoomed-in diagram cannot be
    scrolled to. An auto margin collapses to 0 once free space goes negative,
    so `m-auto` centres a small diagram *and* pins a large one.
  - `overflow-auto` on the scroller put the scrollbars in and out as the
    picture crossed the window's size, and with classic scrollbars on that
    takes 15px out of `clientWidth` mid-zoom, re-centring `m-auto` and moving
    the fit under the fingers at the moment they are zooming through it.
    `overflow-scroll` reserves both gutters and makes the geometry constant;
    `scrollbar-gutter` would be the tidy way to say it and is a no-op in
    WebKit. The fit gutter is not square either — the toolbar floats *over* the
    picture, so a symmetric one fitted the diagram's last node neatly behind
    it.
  - The `select-none` that stops a pan painting a selection behind itself
    carries a `!`, and it is not the cascade-layer trap: the two rules that
    give an SVG label its I-beam and its selection are utilities on the same
    element, two elements more specific (`.x svg text` against `.x *`), so at
    equal weight the label keeps both right through the pan.

  Two copies of one diagram are on screen while it is open, so the lightbox
  gets the SVG with its ids rewritten (`rescope` in `Mermaid.tsx`): mermaid
  scopes the `<style>` it puts inside the SVG by that id and references
  arrowheads as `url(#<id>-…)`, and duplicate ids resolve to whichever comes
  first in the document. The full-bleed shell is `DialogCanvas` in
  `components/ui/dialog.tsx` — its own export rather than a pile of overrides
  on `DialogContent`, which is a centred card and would have needed all of
  them undone.

  The agent is told the fence renders, which is the half of the feature that
  makes it fire at all: see the Diagrams section of
  `app/src-tauri/templates/HARNESS.template.md`.
- **`InlineMd` is the same renderer with every block element flattened**, and
  it exists because of where it is used: a chapter's summary is drawn inside
  the `<button>` that seeks to it, and a `<p>` inside a button is invalid HTML
  that WebKit resolves by closing the button early. Paragraphs become spans and
  headings are unwrapped to their text, so the one thing those summaries really
  carry — inline maths — renders without risking the control around it.
- `TopTabBar`'s tabs are **uniform and fixed-width, Chrome-style**: every tab
  is `TAB_W` however long its title, and only once the strip is full do they
  shrink together to share it, down to `TAB_MIN_W` before it scrolls. The
  width is computed from the measured strip rather than left to
  `flex-shrink`, because a flex container that scrolls reports its *content*
  width as its intrinsic width in WebKit — the strip sizes itself to the
  titles and the tabs then shrink to fit that, which is the content-hugging
  the fixed width exists to avoid. A title too long for its tab fades out at
  the edge (a mask on the title, which is the full leftover width, so a title
  that fits never fades) rather than being clipped mid-glyph or ellipsised.
- `TopTabBar`'s drag-reorder maths measures a neighbour swap as one tab width
  plus the column between tabs (`SEPARATOR_W`), so that column is always
  rendered — coloured or transparent — and the gap between pills must never
  become a flex `gap`, which neither rect measures.
- **Home is a launcher, and owns no state of its own.** `/` lands on
  `app/src/pages/HomePage.tsx`, which is a composer over three recency
  sections (`app/src/components/home/`). Each section reads for itself, hides
  itself entirely when it has nothing — a quiet day is a shorter page, not a
  column of empty states — and shares one `useNow()` clock the page owns, so
  six rows are not six minute timers. Today renders the calendar's own
  `app/src/components/calendar/EventRow.tsx` rather than a second row that
  merely looks like it, and Continue opens a lecture, file or thread the way
  that item's own list opens it, so Home adds no second path into anything.
  Because every tab stays mounted, each section re-reads on the front edge of
  its tab as well as on its events (`useHomeSection` in
  `app/src/components/home/useHomeSection.ts`) — a backgrounded launcher
  showing hour-old recency is the one way this page can lie. Playback's own
  `LECTURE_PROGRESS_EVENT` is deliberately not among those events: it fires
  every five seconds of a recording, and the front edge catches what it missed.
- **Home's composer always starts a new thread.** It sends with a null thread
  id and then leaves for `/chat`; continuing a conversation is a row in
  Continue, not a second inbox here. That is why nothing on it is
  `providerLocked` or `subjectLocked` — those exist to stop an *open* thread
  changing the agent it was bound with, and there is no open thread on Home.
- `ChatPage` renders **one** composer in one of two places: centred under the
  hero while the chat is empty, docked at the bottom once there is a
  transcript. It is a single element moved between branches, not two, so the
  box keeps its ref, focus and draft text across the switch. Its
  controls are `app/src/components/harness/ModelPicker.tsx` — agent, model and
  reasoning level in a single trigger, with the vendor marks from
  `ProviderMark.tsx` beside them — and `SubjectSelect.tsx`, which scopes the
  thread to one subject or leaves it library-wide. `@` in the box opens a
  file menu narrowed to that subject and writes the picked file's library path
  into the message; nothing is read or attached in the frontend. **The text
  box is not a textarea**: mentions are drawn as inline chips — the file's own
  glyph plus its display name — so it is a contenteditable
  (`app/src/components/harness/MentionInput.tsx`), and what the composer
  checks for emptiness and hands to `onSend` is that editor *serialized*, with
  every chip back to its backticked library path. The path is the whole point
  of a mention (`oculus read` takes it as-is), so the drawn and sent forms
  differ by design and the display name is never what goes out. The chip
  itself is one shared component
  (`app/src/components/markdown/FileChip.tsx`), used by the composer, by the
  question bubble in `app/src/components/harness/Timeline.tsx` and by inline
  code in the agent's own prose, and it is built from the path alone
  (`pathFile` in `app/src/lib/openFile.ts`, mirroring `category_from_path` in
  `app/src-tauri/src/paths.rs`) so a reader draws a hundred of them without a
  query. The menu
  offers only files the agent can read, so an unparsed PDF is absent — and an
  empty type-ahead reads as a typo, which is why a query that matched only
  unparsed files gets a line saying so (`countUnparsedMentionMatches` in
  `app/src/lib/db.ts`) rather than rows that cannot be picked. A message
  typed while the agent is working is **queued by Rust**, not sent — the box
  keeps taking input and grows a stop button beside its send — and it is
  drawn as a dashed bubble at the end of the thread until it goes out. See
  [harness.md](./harness.md#the-model-picker),
  [subject scope and `@`](./harness.md#subject-scope-and-) and
  [one turn at a time](./harness.md#one-turn-at-a-time).
- Files and lectures open in the **side panel** (`sidePanelStore` +
  `app/src/components/panel/SidePanel.tsx`); "expand" navigates to the
  full-page route **in the same tab**, and to a new one on ⌘-click — the web's
  own rule, rather than a control that can only ever spawn tabs. A same-tab
  expand is animated: the frame sweeps out to the full width of the card with
  its contents growing along with it, and the navigation is committed at the
  *end* of the sweep, so the peek reads as becoming the page instead of
  vanishing and being replaced. The panels hand `SidePanel` a route and let it
  own the move, since it is the panel that animates through it. It is **docked, not overlaid**: `AppLayout`'s content card
  is a flex row with the pane stack on the left and the panel against its
  right edge, so the page beside it is genuinely narrower. That is what keeps
  the in-app browser honest — `BrowserPage` measures its own slot and re-places
  the native webview when the slot resizes, where an overlay could only ever
  trip the "covered" check and hide it. ⌥⌘S folds the panel away, after ⌘B for
  the sidebar and ⌥⌘B for the chat's conversations column; all three test
  `e.code`, because ⌥ rewrites the character the key produces on macOS. The
  frame stays mounted at zero width when nothing is open, so opening and
  closing animate as a width transition rather than a mount (nothing can
  animate a mount); the contents lag the store by that one animation so a
  closing panel still has something to slide out, and they are pinned to the
  panel's right edge at its rest width so the wipe costs no reflow inside.
  The grip is `ResizeHandle`, a zero-width flex sibling *outside* the panel —
  it survives the fold, so dragging it back out is the second way in — and
  `useResizablePanel` takes a `side`, since a right-docked panel widens as the
  pointer moves left.
  The panel's *contents* are per tab and its *size* is not: each tab keeps
  what it opened and switching tabs swaps the contents, while dragging it
  wider in one tab widens it everywhere. `open()` takes no tab id on purpose —
  background panes are `inert` (see `app/src/components/tabs/TabPane.tsx`), so
  a click can only come from the tab in front, which is what lets every list
  row in the app call `openFileSmart` with nothing but a file. A list
  re-fetching a row it has open uses `sync()` instead, which does name its tab
  and no-ops unless that tab still holds the same item.
  Two rules keep a folded or stale panel from eating clicks. **The shell's
  navigation shuts it** — `navigateActive` calls `closeActivePanel`, so a
  sidebar row, a Recent entry or a ⌘K result leaves the peek behind with the
  page it was opened from (a lecture peek is stopped on the way out, the
  pairing the panel's × makes); opening a *new* tab does not, since the tab
  left behind keeps its own page and its own peek. And **the panel unfolds on
  a count of `open` calls**, not on the item changing: re-opening the row that
  is already showing leaves the item identical, and that click has to unfold a
  folded panel too, or the file you last looked at is the one file you cannot
  re-open. The top tab strip is `tabStore` +
  `app/src/components/tabs/TopTabBar.tsx`, Notion-style. It replaces the
  native title bar, so the strip and the empty space after the last tab carry
  `data-tauri-drag-region` to keep the window movable — which only works
  because `app/src-tauri/capabilities/default.json` grants
  `core:window:allow-start-dragging`; `core:default` does not include it, and
  without it the attribute silently does nothing.
- **⌥⌘T splits a tab**: a second pane beside the first, inside the one tab —
  somewhere to keep a lecture, a PDF or a web page next to what you are
  working on. The strip still shows one tab, titled by its main half, because
  promoting the second half to a tab of its own is exactly what you were
  avoiding by splitting. A fresh split opens on `/new`, which is the page that
  asks where you are going.
  - **A pane is the unit, not a tab.** `AppTab extends PaneState` in
    `app/src/stores/tabStore.ts`: a tab's main pane carries the tab's own id,
    so an unsplit tab is exactly what it always was, and a split gets an id of
    its own from the same counter. Everything below a tab is keyed by that
    pane id — the router registry (`app/src/lib/tabRouters.ts`), the side
    panel's peek (`sidePanelStore`), a playing lecture (`ownsPlayback`), a
    Recent entry. So each half peeks, plays and remembers separately, and
    closing a tab or folding a split drops both kinds of state for the panes
    that went away.
  - **The shell drives the half you last clicked.** `tab.focus` is set in the
    capture phase by the pane's wrapper — pointer *and* focus, so the keyboard
    counts — which means it has already moved by the time the thing you
    clicked reacts to the click. `focusedPane` is then what `navigateActive`,
    `goInActiveTab`, `useActivePath` (the sidebar's highlight) and the strip's
    arrows all resolve through, so "where am I" has one answer however the tab
    is split, and it is the same half a ⌘K result lands in.
  - **The focus marker lives on the seam**, not inside a pane
    (`app/src/components/tabs/TabPane.tsx`). A pane showing a browser page is
    covered by a native WebView the DOM cannot draw over, so an inset ring or
    an inner border would vanish for exactly the half a split is most often
    used for. `ResizeHandle` takes a child for this, and that child is an
    indigo hairline on the driven side.
  - **Which half is per tab; how wide is not.** The ratio is one
    `localStorage` value shared by every tab, the same trade the side panel's
    width makes: what you opened is yours, how big the furniture is belongs to
    the window. The seam is dragged as a *fraction* rather than a width, so it
    keeps its place when the sidebar folds or the side panel opens.
  - **⌥⌘T is a menu item** (`app/src-tauri/src/menu.rs`), like ⌘T and ⌘K and
    more so: the half you are reaching *from* is often a browser page, whose
    native WebView takes the keys and would never hand them to the app's own
    webview. It toggles in three steps rather than two — whole splits, split
    while you are in the main half moves you across, split while you are
    already in it folds it away — because pressing it from the left almost
    always means "go there", not "close that".
  - **Navigating a split half away from a browser page is allowed**, where the
    main half gets a tab of its own instead (`navigateActive`). The page is
    not lost: it belongs to no pane for one snapshot, and `useBrowserTabs`
    puts it back in the strip as a tab. So the half you were pointing at is
    the half you get, which is the whole point of having split it.

- **⌘-click opens a new tab everywhere, and no call site knows it.**
  `app/src/lib/newTabClicks.ts` is one capture-phase listener on `document` —
  the in-app twin of the external-link net in
  `app/src/layouts/AppLayout.tsx` — that walks up from whatever was clicked
  for the route it leads to and opens a tab at it instead. The rule used to be
  per call site, which is why most of the app quietly did nothing on ⌘-click,
  and why a `Link` did something worse: react-router hands a *modified* click
  to the browser on purpose, and the browser's answer inside a Tauri webview
  is to reload the whole app out from under the strip.
  - **Anchors are free.** Every `Link` and `NavLink` already carries a
    resolved `href`, so the projects list, a task title, the subject tabs and
    the settings tabs were fixed without an edit.
  - **`data-tab-href` is for the rows that are buttons**, which in this app is
    most of the sidebar, the crumb rows — they must stay buttons, so that a
    plain click goes through `navigateActive`'s departure rules — and every
    list row that opens a file or a lecture in the side panel. A peek has no
    href of its own; the attribute names the full page that *is* its
    tab-sized form (`filePageHref` in `app/src/lib/openFile.ts`,
    `lecturePagePath` in `app/src/lib/lectures.ts`), and is left off where
    there is no such page — a binary handed to the system viewer, a chat
    thread, which is selected in the one chat page rather than routed to.
  - **`data-tab-skip` is the opt-out**, for a control nested inside a row that
    leads somewhere: the download button in a lecture row, the parse badge in
    a Downloads row. Found first on the way up, it stops the walk.
  - **The path is matched against the real route table**, not sniffed for a
    leading slash. An absolute path proves nothing here — a CLI agent's
    markdown is full of `/…/com.tchan.oculus/courses/…` links, which are files
    and belong to `app/src/lib/openFile.ts`.
  - **One click is still carried by hand**, and has to be: a file chip in
    a thread holds a *library path*, and which route that stands for is a
    database lookup away. `openLibraryPath` (`app/src/lib/openFile.ts`) takes
    the modifier and answers it once the row is in hand — opening the way it
    always did when the path resolves to no page, or to no row at all.

- **A new tab lands on `/new`, not on Home.** `app/src/pages/NewTabPage.tsx`
  is where the + button, ⌘T and a fresh split go, and where the last tab in
  the strip is sent when its page is closed out from under it (`tabStore`'s
  `HOME`). It is **a field, two doors and a trail**: the search above, drawn
  inline rather than over the window, then a browser tab and a new
  conversation over the same Recent trail the sidebar lists, in the
  `Section`/`ROW` grammar every other list in the app uses, and nothing else.
  The results are an **overlay** hung off the field rather than a page that
  replaces this one: the doors and the trail stay where they are underneath,
  and the field does not move as answers land — the same rule that hangs the
  ⌘K palette from the top of the window rather than centring it. Home is the
  *launcher* and stays that: a composer over today's agenda, what you were
  last in, your projects. A tab you opened to put something beside what you
  are already reading wants a way in, not a second dashboard.
  Both doors consume the tab they were clicked in, and so does a picked web
  result, which the cases have to do differently. Chat is a plain navigation,
  so the tab becomes the conversation. A browser tab cannot be — its page is a
  native WebView Rust owns, and `useBrowserTabs` puts a tab of its own in
  front — so the new-tab tab closes itself behind it. In a **split half**
  there is no tab to close behind: `openUrlInFocusedPane` navigates the half
  itself to the page's route and drops the strip tab the reconcile may already
  have made for it. The safety net for losing that race is
  `tabStore`'s sole-tab rule: a lone tab is sent to `/new`, which is where it
  already is, so the worst case is the tab staying put rather than jumping
  somewhere unasked. The start page is `searchHome()` in
  `app/src/lib/browser.ts` — the configured engine's home, shared with
  `normalizeAddress` and with ⌘K's web row, so none of the three can disagree
  about which search this app does.
- **A full page keeps a trail, because it loses the shell.**
  `/subjects/:id/file` and `/subjects/:id/lecture` sit outside `SubjectLayout`
  on purpose — a full-page document takes the whole content area — so the
  subject's title block and tab strip are not above them, and a lecture opened
  from Home, the sidebar's Recent group or the ⌘K palette named no subject and
  led back to nothing. `app/src/components/subjects/SubjectCrumbs.tsx` is that
  trail and only that: the subject, then the tab whose list holds the thing.
  For a lecture the tab is always Lectures; for a file it is read off the
  file's `category` (`category_from_path` in `app/src-tauri/src/paths.rs`), so a page crumbs to Modules
  and an Ed thread to Discussion, while `home` and `syllabus` crumb to the
  subject alone rather than repeat the Overview the subject crumb already
  points at. It is a Fragment ending in its separator, the shape
  `ProjectCrumbs` established ([projects.md](./projects.md)), and it resolves
  the subject itself because its two callers have an id and skipped the layout
  that would have loaded one. The lecture page had no header at all before
  this; the row is `h-11`, and fullscreen takes the player `fixed inset-0`
  over it. The crumbs are **buttons with a
  `data-tab-href` (`app/src/lib/newTabClicks.ts`), not `Link`s**: a plain
  click goes to `navigateActive` (`app/src/lib/tabRouters.ts`), and the
  attribute is what opens the crumb in a tab of its own. A `Link` addresses
  the pane's own router and so slips past the departure rules the shell keeps
  at that one door — which is how a crumb became the only way out of a playing
  lecture that never asked (see the player section below) — and it hands
  ⌘-click to the anchor's default, which reloads the webview at the crumb's
  path. `ProjectCrumbs` and the task page's project crumb are built the same
  way.
- **One search, two fields.** `app/src/lib/search.ts` is what ⌘K
  (`CommandPalette.tsx`) and the new-tab page's own field
  (`app/src/pages/NewTabPage.tsx`) both ask; `app/src/components/search/SearchList.tsx`
  draws the rows for both and `app/src/hooks/useSearch.ts` runs the query for
  both. Adding a kind of thing to find is a change in one file. It builds
  *data*: an icon is named rather than rendered and a row carries a `target`
  rather than a closure, because the two surfaces navigate differently — the
  palette drives the shell from outside every router, the new-tab field drives
  the pane it is drawn in — and `openSearchItem` is the one place that dispatch
  lives.
  It finds subjects, files, lectures, **projects, tasks**, the app's own
  routes, and the **words inside parsed documents**. Titles come from SQLite on
  every keystroke (`searchLibraryFiles` / `searchLibraryLectures` in
  `app/src/lib/db.ts`, `searchProjects` / `searchTasks` in
  `app/src/lib/projects.ts`), all sharing one matching rule — every typed word
  must appear somewhere in the haystack, in any order, so "algorithms graph"
  finds `graph-algorithms.pdf` — and the file haystack flattens `-`/`_` to
  spaces and appends the subject code, because a file is on disk as a slug but
  reads as a title (`humanizeSlug`) and "comp30026 workshop" should be one
  query rather than a filter plus a query. Ties break towards this term's
  coursework and then towards what was opened last, which is what makes the
  empty field a list of where you just were.
- **Searching inside documents is lexical, and that is the point.**
  `searchPageText` in `app/src/lib/db.ts` reads `pages_fts`, an FTS5 index over
  `pages.markdown` (migration 35; the SQL and its triggers are
  `retrieval::PAGES_FTS_SQL`, next to the table they index). It answers in
  milliseconds off the same SQLite, which is what a field you are still typing
  in needs; the semantic index deliberately stays out of it, because that is a
  cloud embedding round-trip per query ([retrieval.md](./retrieval.md)) and
  belongs to a question you ask Chat. One row per *file*, not per page —
  `MIN(bm25(…))` picks the page that scored and SQLite takes the bare columns
  beside it from that same row — and a file that already matched by title is
  dropped from the text section, because a snippet earns its line only for a
  document the title search missed. The matched prose is `snippet()`'s, fenced
  in two control characters so the markdown it is cut out of can contain any
  printable delimiter; `snippetParts` splits on them and `tidyMarkdown` strips
  the heading hashes and table pipes that would otherwise eat the few
  characters there is room for. Only *parsed* documents are in the index —
  that is the honest limit of this search, not a bug to route around.
- **A field that finds nothing still goes somewhere.** What you typed is
  either a web address or something to look up, and the last section offers it
  as one or the other — through `normalizeAddress` in `app/src/lib/browser.ts`,
  so the search field and the browser's address bar cannot disagree about what
  counts as a URL or which engine a query goes to. It sits last, so it is the
  whole answer when the library had none and out of the way when it did.
  Picking it calls `openUrlInFocusedPane`, which is where a page lands in a
  split half rather than becoming a tab.
  Enter goes there in the current tab and ⌘↵ in a new one, the rule the
  sidebar's Recent
  rows already follow; a file opens as its **full page**, not in the side
  panel, because the panel closes itself the moment the route is not its
  subject's.
  The store exists only because the two ways in are far apart: the palette
  hears the menu event itself, and the magnifier in the sidebar's header is
  the visible handle that teaches the shortcut. That handle sits in the header
  rather than in the nav below it because search is the one thing in that list
  that is not a place, and it took the slot the sidebar's own collapse button
  used to hold — a control for a persistent setting that only faded in on
  hover, with the title bar's always-visible toggle
  (`app/src/components/tabs/TopTabBar.tsx`) doing the same job one row up.
- **The sidebar's Recent group is the router's trail, not the strip's**, and
  it is deliberately a *still* list. `app/src/stores/recentTabsStore.ts` is
  fed by each pane's navigation effect (`app/src/components/tabs/TabPane.tsx`
  — the one place that sees every move a router makes) and
  `app/src/components/sidebar/RecentNavGroup.tsx` lists the first five. Three
  rules, all of them Notion's, are what stop it shuffling under you while you
  work:
  - **A visit lands only after you settle.** `recordRecentTab` holds the path
    for a couple of seconds, per pane, and the pane's next move cancels it —
    so a subject you clicked through to reach a lecture, or a redirect like
    `/settings` → `/settings/canvas`, never becomes a row.
  - **A row is of a *thing*, not of a path** (`recentKey`). A subject's eight
    section tabs share one row that follows you between them, as do settings'
    pages; a file, a lecture, a project and a task are pages of their own and
    get their own. Without it one subject filled the group with rows all
    called "INFO30006".
  - **A page already in the trail never moves.** Returning to it refreshes
    when it was last seen, in place; only a new page is inserted, at the top.
    Eviction then has to go by *least recently seen* rather than by position,
    since the bottom row is no longer the stalest.

  Only the path is stored: the title and icon come from `tabInfo`
  (`app/src/components/tabs/tabInfo.tsx`), shared with the tab strip, so a
  recent page is named exactly as its tab is and a renamed subject follows on
  its own. Browser tabs stay out — `/browse/<id>` names a native page whose id
  dies with it — and so do Home and `/new`, the two places a tab sits when it
  has not been sent anywhere. `/new` lists this very trail, so a row leading
  back to it would be a row leading nowhere.
- **A subject's sidebar row lights on the subject, not on everything under
  it** (`end` on the `NavLink` in `SubjectsNavGroup`). A lecture or a file is
  a page of its own with its own tab, and lighting the subject row for it
  said you were somewhere you weren't.
- **External links open in the in-app browser, not in Safari.** One
  capture-phase click handler in `app/src/layouts/AppLayout.tsx` catches
  every `<a href="http…">` in the app — markdown links included — and hands
  it to `openExternal` (`app/src/lib/browser.ts`), so no call site needs to
  know; ⌘-click still hands the URL to the real browser. The handler has
  already cancelled the link's own navigation by then, so `openExternal`
  owns getting it somewhere: a tab that fails to open falls back to the real
  browser and says why in the console, rather than leaving the link inert.
- **Browser tabs are tabs in the same strip, and their route never moves.**
  A browser tab's path is `/browse/<id>`, where the id names a native page
  WebView that Rust owns (`app/src-tauri/src/browser.rs` — an iframe cannot
  work, Canvas refuses to be framed). `app/src/pages/BrowserPage.tsx` draws
  the toolbar across the top of the content card and leaves an empty
  slot under it; the page is a WKWebView parked over that slot. Rust owns
  the tab list and pushes a `browser-state` snapshot on every change;
  `useBrowserTabs` (mounted once in `AppLayout`) mirrors it into
  `browserStore` and reconciles it with `tabStore` — **per pane**, not per
  tab: a page no pane is showing opens as a tab in front, and a pane whose
  page is gone closes (the tab, for a main pane; only the half, for a split
  one). Panes, because a split half can hold a page too, which is half of what
  splitting is for — a page adopted into a split gets no tab of its own, and a
  split navigated away from its page hands that page back to the strip on the
  next snapshot rather than stranding it. Page
  navigations change the tab's URL in Rust and nothing else, which is what
  killed the first version's loop (page load → router → re-layout → title →
  router again). A browser tab is kept pinned to its page on the way
  *in*, by `navigateActive` (`app/src/lib/tabRouters.ts`): a shell click that
  would take it anywhere else opens a tab of its own instead. While a browser
  tab is in front the strip's arrows drive the page's history, not the
  router's — and they grey out honestly, because the snapshot carries
  `can_back`/`can_forward` read off the page itself.
- **What a page knows about itself, only the page can say.** Whether the back
  list has anywhere to go, whether a find matched, what the zoom is: none of it
  follows from the URL, and Tauri exposes an API for only the last. So
  `browser.rs` reads and drives them on the WKWebView through `with_webview` —
  which hands nothing back, since it dispatches to the main thread and returns.
  Every one of them is therefore a *push*: the answer is written into the tab
  and broadcast (or, for a find, emitted as `browser-find`), never returned to
  the command that asked. The toolbar in `BrowserPage` is the consumer:
  back/forward, reload, the address bar, a zoom pill that appears **only when
  the zoom is not 100%** and resets it when clicked, find, and open-externally.
- **Every browser shortcut is a menu item, and that is not a style choice.**
  `browser_place` calls `set_focus()` on the page, so while you are browsing
  the app's own webview receives no key events at all — and macOS gives the
  menu bar first refusal on ⌘-keys even when it does. A `keydown` listener in
  `BrowserPage` therefore worked only in the sliver where the app happened to
  have focus, which is exactly not when you reach for ⌘R. ⌘R, ⇧⌘R (reload
  ignoring the cache), ⌘L, ⌘[, ⌘], ⌘F, ⌘G, ⇧⌘G and ⌘=/⌘−/⌘0 are all items in
  `app/src-tauri/src/menu.rs`; the frontend owns what each means, so ⌘[ routes
  through the same `go` the strip's arrow does and ⌘R is a no-op away from a
  browser tab rather than a menu entry that greys itself out (which would mean
  telling Rust which tab is in front — a second copy of a truth the frontend
  already owns).
  **⌘= / ⌘− / ⌘0 have two scopes and one pair of keys**: on a browser tab they
  zoom the *page* (`pageZoom` on the WKWebView), everywhere else the window,
  and `AppLayout` picks between them because it is the side that knows what is
  in front. ⌘+ is ⇧⌘= and muda binds the physical key, so the shifted one
  never reaches the menu — `AppLayout` keeps a `keydown` for that one alone and
  routes it identically.
- **The address bar suggests from a history of one row per URL.**
  `browser_history` (migration 36) counts visits and keeps the last time, and
  `app/src/lib/browserHistory.ts` ranks by **frecency** — visits halved every
  seven days — with a boost for a host whose start you are typing. Matching is
  the ⌘K palette's rule, reused rather than reinvented: every word you typed
  must appear somewhere in the URL or title, in any order, with `-`/`_` read as
  spaces. SQL narrows to a recency-ordered shortlist and the ranking runs in
  JavaScript over it, because frecency wants an exponential and SQLite has no
  `exp()`.
  **Writing happens in `useBrowserTabs`, from the snapshot**, and two rules keep
  it honest. A visit is recorded when the page has *finished* loading, never
  when it commits — a commit fires for every hop of a redirect chain, and one
  Canvas SSO bounce would leave four rows for one destination. And
  `historyUrl` drops the fragment always and the **whole** query as soon as any
  part of it looks like a credential: Echo360's playback URLs are signed, and a
  lecture watched in a browser tab would otherwise put a live token in a
  dropdown. Whole, not the offending parameter — half a credential is still a
  credential, and a table that has swallowed tokens cannot be un-swallowed
  without a migration.
  **The list is a popover over a still of the page.** A popover over the page
  itself is not possible — the DOM renders beneath a native view — and simply
  hiding the page while the list is up blanked the card the moment you typed a
  character. So focusing the address bar asks Rust for a PNG of the page as it
  stands (`browser_snapshot`, `takeSnapshotWithConfiguration:`), the slot paints
  that image, the live page steps aside behind it, and the dropdown is drawn
  over the image like any other popover. The page is frozen while you type and
  identical to what was there; if no still arrives the list opens anyway and the
  page goes, which is the floor rather than the plan.
- **Tabs carry the site's own icon.** WebKit has no public favicon API, so
  `browser.rs` fetches one beside each page load — `/favicon.ico` first, then
  the document's `<link rel~="icon">` if that came back as nothing — sniffs the
  bytes rather than trusting a 200 (servers answer that path with their HTML
  404 page), and pushes it as `browser-favicon`, keyed by **host**. Its own
  event, not the snapshot: the snapshot goes out on every page-load edge and
  would carry kilobytes of base64 with it each time. Once per host per run;
  `browser_favicons` keeps what was found so the next run has icons before any
  page loads, which is also what lets the history list in Settings show them
  for sites no tab is on.
- **A page webview has to say it is Safari.** `PAGE_USER_AGENT` in
  `app/src-tauri/src/browser.rs` is set on every page, because WKWebView's
  default UA stops at `AppleWebKit/605.1.15 (KHTML, like Gecko)` — nothing sets
  `applicationNameForUserAgent`, so there is no `Version/… Safari/…` suffix and
  UA-sniffing sites read it as an engine they do not know. Measured: google.com
  answered 86 KB of no-JavaScript fallback under the default and 221 KB of the
  real page under a Safari string. The engine is Safari's either way, so only
  the version number is a claim; keep it roughly current, because a stale one
  starts reading as an old browser again. The scraper's HTTP client keeps its
  own string (`app/src-tauri/src/okta.rs`) so that bumping this cannot disturb
  a working SSO flow.
- **A page webview also has to claim a window size.** A page is a *child*
  view inside the main window, so it has no window of its own and WebKit
  reports `outerWidth`/`outerHeight` as `0`. That zero is not cosmetic:
  `outerWidth / innerWidth` is how a page detects browser zoom, and the
  degenerate ratio makes a canvas renderer fall back to its minimum scale.
  Measured on Google Docs: an `816x1056` CSS page tile came back with a
  `408x528` backing store — a quarter of the resolution a 2x display wants,
  stretched 4x on the way to the screen. Only the canvas was soft, because
  WebKit rasterizes DOM text at the layer scale whatever a canvas does, so
  the toolbar stayed crisp and it read as an app-wide blur. An
  `initialization_script` in `app/src-tauri/src/browser.rs` reports the
  viewport's own size for both, which is roughly what a full-window browser
  would say; the same tile then comes back `1632x2112`.
- **A native page cannot interleave with the DOM.** Anything drawn over the
  slot — a sidebar popover, a tooltip reaching in, a dialog — would render
  beneath the page, so `BrowserPage` watches `document.body` for portals
  whose rect lands on the slot and hides the page until they are gone. A tab
  going to the background hides it for the same reason, and that one needs
  saying out loud: panes stay mounted, so leaving a browser tab is no longer
  an unmount — `BrowserPage` takes the page down when `useTabActive` goes
  false, or it would stay parked over whichever tab came forward. The address
  bar's suggestion list is the third, and the three are computed as one
  `hidden` rather than as three effects racing to place the same page. Only
  that third one leaves a **still** behind — `browser_snapshot` returns raw
  PNG bytes through `tauri::ipc::Response`, the frontend holds them as a blob
  URL for one editing session, and the image goes out of the slot when Rust
  says the live page is back rather than when the list closes, because between
  those two is a frame with neither. Anything the *toolbar itself* needs room
  for takes it by being a row of the toolbar — the find bar — which shrinks
  the slot instead. The
  slot is reported as insets from the window edges (CSS pixels times the
  page zoom from `--app-zoom`), re-measured by a `ResizeObserver` when the
  sidebar toggles or the zoom changes; window resizes are Rust's alone. The
  page rounds its own bottom corners to the card's inner radius, since the
  card cannot clip it.
- The calendar reads its own tables and never the scraper: `loadCalendar` in
  `app/src/lib/calendar.ts` pulls `calendar_events`, `lectures`, `local_events`
  and the open `project_tasks` in one go and the page filters in memory, so
  month and week paging is arithmetic rather than queries. See
  [calendar.md](./calendar.md) for where the rows come from and why the tasks
  are read live instead of copied.
- **Tables are full-bleed, with their own header and footer.** `SyncPage`
  gives its body no padding: `SyncHistoryTable` and `PipelineTable`
  (`app/src/components/sync/`) each own a `h-full` column — a fixed column
  header, a scrolling body, and a pinned `TablePagination` footer — so the
  rows run edge to edge inside `AppLayout`'s card and only the rows scroll.
  The header sits **outside** the scroll container rather than sticky inside
  it: `index.css` gives `::-webkit-scrollbar` an explicit width, which makes
  the bar a classic one taking a 6px gutter out of its scroller's full
  height, and a header inside that scroller gets the bar drawn down its own
  right edge. The header's wrapper re-creates the gutter with `pr-1.5` and
  the body carries `scrollbar-gutter: stable` so it is reserved even with
  nothing to scroll — drop either and the header falls 6px out of column.
  Row gutters (`px-5`) are the table's, not the page's. `usePagedRows` in
  `app/src/components/ui/TablePagination.tsx` holds the page state and clamps
  it, so rows disappearing under a live sync can't strand the view past the
  last page.
- The Sync page's two tables are **sibling tabs, not a dropdown**
  (`ViewTabs`, the non-routed twin of `SubjectLayout`'s nav): the inactive
  view stays legible as a greyed-out label instead of hiding inside a menu.
  The tab strip holds nothing but the tabs, on the rule the active one
  underlines; every control lives in the toolbar below it. What the view is
  scoped to sits on the left (the subject picker and the what-to-sync
  popover), and how it is going plus what you can do about it on the right
  (last-synced or live progress, then Sync now / Cancel; Resume all / Clear
  finished for the pipeline). That toolbar has a **fixed height**: sized to
  its contents it measured taller under the subject button than under the
  pipeline's badges, so switching tabs jolted the table below.
- The pipeline ledger no longer collapses finished files into a group — rows
  are ranked (running, queued, paused, failed, done) and paged, so live work
  is on page one and the footer counts what is behind it.
- **The ingest pipeline is three stages: `download → parse → embed`.**
  `app/src/stores/pipelineStore.ts` holds one row per PDF-backed file and
  `app/src/components/sync/PipelineTable.tsx` draws it — a stage dot each, one
  progress figure for whichever stage is moving, a timeline of the steps when a
  row is expanded. It was four once (a fast local parse, then a quality one,
  then a sidecar embed); the fast tier is gone for good and the third stage
  that came back is not the one that left — it is a metered cloud call a file
  reaches only after its parse has landed.
  **The third stage is conditional**, which none of the others are: with no
  Voyage key stored there is no embedder, so `embedStage` in the store is
  false, the dot is not drawn and a parsed file is complete at two. Every
  derived view (`statusOf`, `isComplete`) takes that flag rather than assuming,
  and defaults it to false — which is exactly the two-stage table this page
  described before. `indexStore` owns the flag and sets it.
  The stage is also **seeded from page coverage, never from
  `files.embed_status`**: that column is a sticky flag with no memory of which
  model wrote the vectors, so after an engine change it claims `'done'` over a
  library where nothing is searchable. `getEmbedCoverage` in
  `app/src/lib/db.ts` counts current-space page vectors against the file's page
  rows, the same question `getUnembeddedPdfs` asks. The column is read for one
  thing only — a *failure*, which leaves no trace anywhere else and would
  otherwise come back after a restart as a row that is merely waiting.
  The `parse-status` event Rust emits carries
  `relative_path`, `subject_id`, one of `queued | running | quality | error`,
  plus `pages_done`/`total_pages` while running, `position` while queued, and
  on an error a displayable `error` with three discriminants — `kind`,
  `retryable` (could retrying *this file* work) and `latching` (does this
  condemn every other file too). `app/src/hooks/useBackendEvents.ts` is the
  only reader; it writes the status to `files.parse_status` and carries the
  discriminants into `app/src/stores/parseStore.ts` (per file, plus the latch)
  and `pipelineStore` (per row).
  **`"quality"` is the terminal success and is not a tier.** The name outlived
  the two-tier parse: every already-parsed row in the library says
  `parse_status = 'quality'`, and it is what Rust's "already done" check reads,
  so renaming it would invalidate the library. `"fast"` is gone from the
  vocabulary entirely. The store calls the *field* `parse`, since there is only
  one parse to name — the field and the wire string are different things.
  **`embed-status` is the same event one stage over**
  (`app/src-tauri/src/embed/events.rs`): same field names, same three
  discriminants, `queued | running | done | error` — and the success word is
  plain `"done"`, because unlike `'quality'` it was never written into a
  library's worth of rows. `useBackendEvents` reads it into the same row, into
  `files.embed_status`, and into `indexStore` for the settings page's bar. Its
  `running` is the page counter *inside* one document, which is the thing the
  app had no way to show before: a document is one blocking call, and on the
  free Voyage programme that is ~2.8 pages a minute.
- **A finished parse queues the embed behind it**, so a sync runs
  download → parse → embed end to end. The hop is in `useBackendEvents`, on
  the terminal `parse-status`, and it reads the file row because the queue
  needs a `file_id` where the event carries a path. It is gated on `ready`.
  What it deliberately does *not* do is enumerate the **backlog**: draining 166
  already-parsed files is hours of metered work, and the estimate on the
  settings page is written to be read before that starts. Auto-embed covers
  what arrives from now on; the Index button covers what was already there, and
  a paused row's ▶ covers one file.
- **A file says whether it has markdown, and why not.** One engine is
  selected and nothing catches its failures — a parse is never re-tried on the
  other engine — so a failure means that file has no
  markdown — no search, no `@`-mention, no Markdown view — until something
  changes. `app/src/lib/parseState.ts` is the one place that turns a status
  plus the failure discriminants into a state and its words; every surface
  renders that, so a row and a document can never say different things.
  Seven states, three of them ordinary (`parsed`, `parsing`, `queued`) and
  four that used to be invisible: `not parsed` (quiet — an unparsed file is
  not a failure and colouring it red is a lie), `failed`, `can't parse` (the
  failure said `retryable: false`, so the sweep will never re-kick it) and
  `on hold` (a `latching` cause — no token, a rejected token, a spent quota —
  which condemns the whole library, not this file). That last split is the one
  the UI must never fudge; unknown discriminants, which is every failure
  inherited from a previous session, stay their own case rather than being
  rounded to either end. `app/src/components/files/ParseState.tsx` draws it
  twice: a word per row in `app/src/pages/subject/DownloadsPage.tsx`, where
  three of those states used to render as the empty string and so read as
  "fine", and `MarkdownUnavailable` in place of the PDF ↔ Markdown toggle in
  `app/src/pages/subject/FilePage.tsx` and
  `app/src/components/panel/FilePanel.tsx`, where the toggle used to simply not
  be there. Its popover carries the backend's own sentence, and a button to
  `/settings/library` for the one cause a student can fix — a token. A quota
  gets no button, because waiting is the only move.
- Background job progress surfaces **only** in the sidebar (driven by the
  stores fed from `useBackendEvents`) — no toasts, no bottom bars.
- **The history arrows follow React Router's index, not
  `window.history.length`.** `TopTabBar` keeps the current index (from
  `history.state`) *and* the top of the stack, moving the top only on a
  `PUSH`, and it recomputes on `location.key` so a navigation that keeps the
  path still counts. `history.length` cannot stand in for the top: it counts
  entries a reload left behind and never shrinks. While a browser tab is in
  front the arrows drive the page's own history through Rust instead, and
  read `can_back`/`can_forward` off the snapshot — the page's back/forward
  list, which `browser.rs` reads on the WKWebView after every load. They used
  to stay lit unconditionally, because from outside a native page there was
  nothing to ask.
- **Window shortcuts are menu items, not key handlers.** macOS hands the menu
  bar every ⌘-key before a webview sees it, so ⌘T (new tab), ⌘W (close tab)
  and ⌘K (the palette) live in `app/src-tauri/src/menu.rs` and reach the
  frontend as `menu-new-tab` / `menu-close-tab` / `menu-search` events. That
  routing is a feature: they work while a browser tab's native page holds
  focus and the app's own webview is receiving no keys at all — which for the
  palette is the point, since ⌘K is how you get back out of a browser tab. It is also why the menu is built by
  hand — Tauri's default spends ⌘W on Close Window, which moves to ⇧⌘W here
  — and why the Edit submenu must stay: without it ⌘C/⌘V stop working in
  every text field. The browser's own shortcuts joined them for exactly this
  reason and are listed above under the in-app browser.
- **Zoom scales the window, not a div** — and on a browser tab, the page
  rather than the window (`zoomBy` in `AppLayout` picks between them).
  `app/src/layouts/AppLayout.tsx`
  drives the webview's own page zoom (⌘=/⌘−/⌘0, persisted in
  `localStorage`), so the whole document — tab strip included — is laid out
  at one scale and every measurement stays in a single coordinate space.
  A CSS `zoom` container was the earlier shape and had to go: inside one,
  WebKit reports pointer coordinates in visual pixels but element rects in
  layout pixels, so Radix popups near a window edge misjudged the room below
  them and drag maths drifted by the zoom factor. The `--app-zoom` CSS var
  survives only for chrome measured in device pixels — the tab strip's
  traffic-light gap divides by it. Fullscreen hides the traffic lights, so
  the gap goes with them: `app/src/hooks/useWindowFullscreen.ts` watches the
  window (the green button and ⌃⌘F arrive as a resize) and the strip drops
  back to the ordinary inset. The lights' *vertical* placement is not the
  strip's to make: AppKit owns those buttons, so it is `trafficLightPosition`
  in `app/src-tauri/tauri.conf.json`, tuned so their centres land on the
  strip's centre at the default zoom. That ties three numbers together — the
  strip's height, `DEFAULT_ZOOM`, and that `y` — so moving any one of them
  means re-measuring; `app/src/components/tabs/TopTabBar.tsx` carries the
  arithmetic.
- **The background parse sweep is the recovery path, and it has to know when
  to stop.** `app/src/hooks/useQualitySweep.ts` periodically queries the DB for
  files in selected subjects whose `parse_status` is not yet `quality` and
  re-requests a few at a time, for files that missed their parse — app closed
  mid-queue, parsing down during a sync, a parse died. Every parse is now a
  metered cloud call, with no local fallback beneath it, so a loop that
  re-submits whatever failed is a quota fire: it would resubmit every
  permanently-broken file every 15 minutes forever, and one bad token would
  march the whole library through the same error a batch at a time. Hence the
  two gates, both reading the error discriminants above: a file whose last
  failure said `retryable: false` is never re-kicked (an *unknown*
  retryability still is, or the recovery path dies — so a bad file costs one
  attempt per launch, not four an hour), and a `latching` failure stands the
  sweep down entirely until a parse gets somewhere again, which any
  hand-driven route does. The one condition that heals on a clock rather than
  an action — a daily quota — is why the latch eventually allows a single
  probe file rather than a batch.
- `app/src/components/lectures/LecturePlayer.tsx` streams video from the
  Rust media HTTP server via `mediaSrc()` in `app/src/lib/media.ts` — not
  `convertFileSrc`, which WebKit's media stack rejects (see
  [architecture.md](./architecture.md)). Its scrub bar is a local `SeekBar`
  rather than the shadcn Slider, kept because `offsetX / offsetWidth` needs
  one measurement where Radix mixes `clientX` with `getBoundingClientRect`.
- **A click that puts a panel away is not also a click on the video.** The
  player is the one place in the app where a bare click on the background does
  something, so it is the one place that has to tell the two apart: clicking
  off the source or layout panel used to close it *and* toggle playback.
  Radix defers its outside-dismissal to the `click` rather than the
  pointerdown, and handles it on `document`, so during the target phase — the
  video's own handler, and the scrub bar's pointerdown before that — the panel
  is still on screen. Its presence is therefore the whole test, and
  `panelOnScreen()` in `app/src/components/lectures/LecturePlayer.tsx` is it.
  The `[data-state=open]` in that selector matters: a closing panel stays
  mounted for its exit animation, and a click landing in those 150ms is real.
  Picking a source or a layout closes its own panel
  (`app/src/components/lectures/SourceControls.tsx`) — you came to switch and
  the panel has nothing left to say — except on the row that starts a
  download, which stays up because the percentage is in it.
- **The scrub bar previews the frame under the pointer**
  (`app/src/components/lectures/ScrubPreview.tsx`), the way YouTube does. It
  is a second muted `<video>` on the same localhost source rather than a
  pre-rendered sprite sheet — the file is already on disk and the media server
  serves ranges, so seeking a spare decoder beats generating and storing a
  storyboard per recording. Two things it must get right: **seeks are gated
  one at a time** (a pointer sweep fires a move per frame, and assigning
  `currentTime` mid-seek makes WebKit drop the earlier target, so a move while
  a seek is in flight only parks its time and `seeked` starts the next one —
  the preview lands where the pointer stopped, not somewhere along the way),
  and the decoder is **mounted on first hover**, not with the player, so an
  untouched bar costs no metadata fetch. Undownloaded lectures have no source
  and get the time readout alone.
- **The controls are over the frame, not under it** — one scrim across the
  bottom of the video with the scrub bar along its top, the way a video player
  is expected to look. They fade after ~2s of pointer idle while playing and
  come back on any movement; paused, they stay. Three things pin them open and
  are tracked separately because they end separately: the pointer resting on
  the bar, the speed panel being open (a bar that faded while its own popover
  was up would leave the panel anchored to nothing), and a volume drag in
  progress. Because the bar is on the frame its palette is fixed
  white-on-black rather than themed — which is why its buttons are a local
  `ControlButton` and not the shadcn ghost `Button`, whose hover is a
  near-white surface that disappears on video. The speed panel follows the bar
  off the bar: it overrides the shared `PopoverContent` to dark glass
  (translucent black + `backdrop-blur`) with the same white-on-frame contents,
  so the slide stays visible through it instead of a sheet of app-white
  landing on the video. Volume
  (`app/src/components/lectures/VolumeControl.tsx`) is not a panel at all: a
  speaker button that mutes, with a horizontal slider that grows out of it on
  hover and collapses when the pointer leaves — YouTube's shape. It sits left
  of the timestamp so the only thing the widening pushes is the time; every
  button keeps its place. The slider also stays out while it is dragged (a
  drag wanders off the strip it started on, and pins the bar the way an open
  panel does) and while it holds focus, so the keyboard can reach it. An "on"
  toggle (captions, transcript) is marked by an underline under the icon,
  since white-on-frame reads the same lit or unlit.
- **The video elements outlive the route.** A tab switch is a navigation, and
  a navigation unmounts the page — which used to take the `<video>`, and the
  lecture, down with it. The elements are owned by
  `app/src/lib/lecturePlayback.ts` instead: the player adopts them into its
  frames on mount and hands them back to an off-screen host (positioned away,
  *not* `display: none`, which is where a browser feels entitled to stop
  playback) on unmount, so a lecture keeps playing while you are in another tab
  and picks up on screen where it actually is when you come back — expanding
  the side panel into a page included, in this tab or a new one. This is a DOM node in the visible webview, not a
  hidden WebView; the suspension problem that keeps scraping in Rust does not
  apply. Because the element outlives the player, so does the progress writing:
  the module saves every 5s of playback and immediately on pause, seek, end and
  `pagehide`, then fires `LECTURE_PROGRESS_EVENT` for mounted lists to refresh
  on. The side panel's player has no list to call back into, so its
  `onRefresh` fires `LECTURES_CHANGED_EVENT` (`app/src/lib/lectures.ts`) and
  whichever list is mounted re-fetches — the same shape, one level up.
  **Only the player in front is the player.** Panes stay mounted, so a second
  lecture tab behind this one is a second player over the same elements —
  `LecturePlayer` adopts them, and listens for the space bar, only while
  `useTabActive` is true. With both of them live, one keypress toggled play
  twice and nothing happened, and every tab switch re-seated the picture in
  whichever pane had mounted last. Same lesson as the browser page above.
  **Playback belongs to a tab, and to one of its two players.** The module
  records which tab the player was mounted in and whether that player is the
  page or the peek, and the tab half is what separates "switched away" from
  "left": going to another tab leaves the lecture playing behind a tab you can
  return to, while closing that tab or navigating it elsewhere would leave it
  playing with nothing owning it. Every one of those asks first —
  `confirmLeavingLecture` (`app/src/stores/leaveLectureStore.ts`) raises one
  dialog mounted in `AppLayout`, saying the place is already saved — and stops
  playback on confirm. They ask **at the door, before anything is committed**
  — the tab strip's ×, and `navigateActive` / `goInActiveTab` in
  `app/src/lib/tabRouters.ts` — and they tell a tab switch from a departure by
  asking whether the tab being acted on is the one that owns playback (the
  strip activates the destination tab *before* it navigates, so a switch is
  already "some other tab" here). The navigation case used to be a
  `useBlocker` inside the player, and that was the wrong shape: a router
  blocker is answered by whichever component holds that blocker's key, so a
  block nobody answers drops the navigation with **nothing on screen** — a
  sidebar click that silently does nothing, in a tab that can then never
  navigate again. Asked at the door there is no blocked router to strand.
  A paused lecture never prompts, and the side panel's own close button is an
  outright stop — which is why the stop hangs off that control and not off an
  unmount effect, since the panel unmounts its body whenever another tab comes
  forward.
  **Every door has to be the door.** Two exits used to miss the rule
  entirely. The breadcrumb row is drawn inside a pane, so it was `Link`s onto
  that pane's own router — the guard never ran, and a crumb click left the
  lecture running off screen with no player anywhere and nothing to come back
  to; it goes through `navigateActive` now, like the sidebar. And the peek's
  player sat outside every pane, so `useTabId` gave it the detached default
  (tab `0`, which no tab has) and a playing peek was owned by nobody: closing
  its tab neither stopped it nor asked. `SidePanel` puts its body under a
  `TabContext.Provider` for the tab the peek was opened from. The host half of
  the owner is what keeps the **history arrows** honest: they are guarded too,
  but only for the *page's* player, because an arrow deliberately leaves the
  side panel open — a peek is still on screen and playing after one, so a
  prompt there would be a false alarm. The old worry about guarding arrows —
  that one might fire on the way *toward* the lecture — cannot happen:
  ownership is only true while the tab is showing the lecture, so both arrows
  lead away from it.
- **Two streams, one lecture, one clock.** A capture can be a Presenter screen
  *and* a room camera ([sync.md](./sync.md)), so `lecturePlayback.ts` keeps one
  element **per source** and nominates one of them the **leader**: it carries
  the audio, it is the clock every readout counts against, and it is what
  writes progress. The others are muted followers, walked back onto the
  leader's `currentTime` on every play, pause, seek and rate change, plus a 1s
  timer that only runs while more than one source is on screen. Both files are
  trimmed identically, so "in sync" is simply the same `currentTime` — there is
  no offset to carry — and drift under ~0.35s is left alone, because a seek is
  a re-buffer and stuttering the picture to fix something nobody can see is a
  bad trade. Past that the correction is a **trim to the follower's playback
  rate**, not a seek: a follower that cannot quite hold real time — which is
  what coming back to a backgrounded tab leaves you with — would otherwise
  earn a decoder flush every tick, invisible on a slide and a picture that
  stutters once a second on the room camera. Only a gap over ~1.5s, or a
  paused leader (no rate to ride), is worth the seek. The leader is whichever
  source is in the **main frame**, so the picture you are watching is the
  audio you hear.
- **A parked element keeps a real size.** Between players the elements sit in
  an off-screen host rather than a hidden subtree, because a hidden subtree is
  where a browser feels entitled to stop a media element. That host is
  480×270, not the 1×1 it started as: the elements are `w-full h-full`, so a
  pixel-sized host is a pixel-sized picture, and WebKit sizes the decode path
  to the picture it is asked for — park a playing lecture in one and it comes
  back to the player as mush. Nothing off screen is painted, so the size costs
  nothing. The player also re-aligns on `visibilitychange`, since a hidden
  page is a throttled page and both decoders come back however far apart you
  left them.
- **The player restates its layout; the module works out the diff.**
  `syncLectureSources(lecture, plan, tabId)` takes the whole arrangement —
  which sources, from which files, into which host elements, leader first — and
  is called again whenever any of that changes. That puts the two awkward
  moments in one place: a source *joining* (it loads and the follower sync
  walks it in) and the leader *changing* (a different file and a different
  decoder, so the position and the playing/paused state are carried across by
  hand). It is also why restoring `progress_seconds` moved out of the player's
  `loadedmetadata` handler: only the module can tell a fresh lecture, which
  should resume at the saved second, from a source switch mid-lecture, which
  should land on the second you were actually watching rather than the last one
  written.
- **The source and layout controls only exist where there is a choice.** The
  per-frame pill (`app/src/components/lectures/SourceControls.tsx`, top-left,
  revealed on frame hover and hidden with the control bar rather than only on
  pointer-out) is rendered only for a capture Echo360 publishes two streams
  for; the layout control on the bar likewise. Picking a source in *either*
  frame swaps the pair — the two frames always show the two streams, so one
  number (`mainSource`) says everything. Which is why the *second* frame's
  pill needs the mirrored half of that swap: picking Source 2 there means
  Source 2 in that frame and so Source 1 in the main one, and handing it the
  main frame's handler inverted the panel — the row you ticked was the row you
  did not get. A stream that has not been downloaded
  is still listed, because the pill is where you find out the camera exists and
  so is where you ask for it; the two-frame layouts stay listed but disabled
  until it is on disk, since hiding them would mean the feature only ever
  appears to someone who already found the download somewhere else.
- **The two-frame geometry is fractions of the video area, not pixels**
  (`app/src/hooks/useSourceLayout.ts`) — the player is a side panel one moment
  and a fullscreen overlay the next, and an inset pinned at "320px from the
  left" means something different in each. The PIP's height is never stored:
  the box carries `aspect-ratio`, read from the inset picture's own
  `videoWidth`/`videoHeight`, so a corner drag can only ever produce a similar
  rectangle and the ratio is locked by construction rather than by arithmetic
  kept right in three places. A fraction is not a size, though, so the width
  also has a floor in pixels (160×90, the height reaching the width through
  that same aspect) applied where the area is measured: the same 26% is a
  legible inset over a fullscreen lecture and a postage stamp in the side
  panel. Docking the panel needed no new case here for exactly that reason: a
  docked width is just another area size, and the pixel floor already covers
  it.
  The cap still wins on an area narrower than the floor itself. The inset's
  four resize handles fade with the control bar rather than on frame hover
  alone — white pips left on the picture after the bar has gone read as
  furniture stuck to it — and pin themselves for the length of a drag, since a
  handle that vanishes under the pointer resizing with it is the one moment
  they must not go. The stacked view divides with `flex-grow` on a
  zero basis rather than percentage heights, so the two screens share what is
  left *after* the divider and the split can never add up to more than the
  frame. The inset sits at `z-20`, under the control bar's `z-30`, so it can
  never cover the scrub bar.
- **The element's duration wins over the catalogue's.** Echo360's lesson
  duration comes from its scheduling data and the recording it serves runs a
  few seconds past it, so a clock counting against `lectures.duration_seconds`
  ended a lecture reading `1:55:00 / 1:54:46`. The player takes `video.duration`
  of the leader at `loadedmetadata` and counts everything — the clock, the
  scrub bar, the within-30s "complete" mark — against that, falling back to the
  DB value until it arrives or when there is no downloaded video. The lecture's own
  metadata line keeps the catalogue figure, which is what the lectures list
  shows too.
- **Fullscreen is the window's, not the element's.** `requestFullscreen()`
  silently did nothing: WKWebView keeps element fullscreen behind a private
  preference wry only sets under Tauri's `macos-private-api` feature, and the
  rejected promise was swallowed. Enabling that feature would have worked and
  broken something worse — WebKit displays only the fullscreen element's
  subtree, while every Radix popup in the player (the speed panel, the
  tooltips) is portalled to `document.body`, outside it. So the button calls
  `setFullscreen` on the Tauri window (hence
  `core:window:allow-set-fullscreen` in
  `app/src-tauri/capabilities/default.json`) and the player promotes itself to
  a `fixed inset-0 z-50` overlay over the shell: the document is intact, so
  the popups still work. The two fullscreens nest rather than being one
  switch: the *window's* is macOS fullscreen with the sidebar and tab strip
  still there, the *player's* is the overlay that covers them. Entering the
  player's takes the window with it; leaving the player's lifts only the
  overlay, so the furniture comes back on a still-fullscreen window; leaving
  the window's (green button, ⌃⌘F — heard through `onResized`) leaves both.
  Entering the window's on its own does *not* raise the overlay. It is
  **off in the side panel**
  (`allowFullscreen={false}`) — the panel is furniture beside a page that
  stays mounted, and an element-fullscreen player inside it would have to
  escape a Radix portal to do anything; expand promotes the lecture to the
  lecture page first.
- **The dock is off in the side panel too** (`allowDock={false}` in
  `app/src/components/panel/LecturePanel.tsx`), and that is a judgement rather
  than a limitation. A dock is a panel beside the video, and the side panel
  already *is* that panel: the responsive rules below keep it legible down to a
  narrow window, but inside a 520px column the honest answer is not a smaller
  dock — it is no dock, with the expand control in the panel's header as the
  way to the one that fits. The button and the T key go with it, since a
  control that cannot do anything reads worse than no control, and
  `TranscriptPanel` is not mounted at all: mounted-but-closed is how the dock
  *slides*, and a dock that can never open has nothing to slide while its Chat
  tab would load a thread and build a composer for every lecture a panel peeks
  at. The preference is untouched, so the lecture page opens with the dock
  exactly as it was left.
- **The player's furniture moves out of the way of the slide.** Captions are
  a draggable overlay (`app/src/components/lectures/CaptionOverlay.tsx`)
  because a lecture slide usually has text where a bottom-centred caption
  lands; its position is stored as a fraction of the *free* space inside the
  video box (`left: x%` paired with `translateX(-x%)`), so it survives a
  resize or a longer line without measuring the caption. It is `w-max`, not
  shrink-to-fit: an absolutely-positioned box fits into `container - left`, so
  a shrink-to-fit caption narrowed as it travelled right — and the drag, which
  measures the width once at grab time, then drifted from the cursor. It also
  rides up while the control bar is showing under it, and the drag adds that
  lift back before mapping the pointer to a position. The transcript slides
  open and shut on the sidebar's shape and duration — outer box animating one
  dimension to zero, inner box holding its full size so the text is clipped
  rather than re-wrapped. The transition is always on and switched *off* for
  the resize drag (`resizing` out of `useTranscriptDock`), where every frame
  sets a new size and an ease would leave the edge trailing the pointer.
  Arming it the other way round — an effect turning the transition on when the
  panel opens — silently does nothing: the effect runs after the paint that
  already moved the box, so there is nothing left to animate.
  The transcript is a panel docked to any edge — drag its header, drop on the brand-tinted
  preview band — and resized from the divider between it and the video
  (`app/src/hooks/useTranscriptDock.ts`,
  `app/src/components/lectures/TranscriptPanel.tsx`). The dock side is
  expressed as the player's flex direction (`flex-col-reverse` /
  `flex-row-reverse` for top / left), which keeps the divider between the two
  panes in every arrangement. The caption position persists in `localStorage`
  of its own; the dock side and size are player preferences (below).
- **Player preferences are the person's, not the lecture's.**
  `app/src/stores/playerPrefsStore.ts` holds speed, captions on/off,
  transcript shown/hidden, the dock's tab, tab *order*, side and size, and the
  two-source arrangement (layout, which stream is in the main frame, the PIP
  box, the stacked split), as one global set persisted under a single
  `localStorage` key — set 1.5×, a left-docked transcript and a camera inset
  once and every recording opens that way. The only per-lecture playback state
  is the position, and that is a DB column (`lectures.progress_seconds`), not a
  preference. Speed and volume are applied in effects keyed on the *leader
  element*, not just the value: `playbackRate` and `volume` are per-element and
  reset on load, so a source switch has to re-apply both to the decoder that
  just took over the audio.
- **The dock is three readings of one recording.** The panel's header is a
  `ViewTabs` strip — Chapters, Transcript, Chat — and which one is in
  front is a player preference (`dockTab`) like the side it is docked to. So is
  **what order they sit in** (`dockTabOrder`): the tabs are dragged with each
  other, which is why `ViewTabs` grew an opt-in `onReorder` rather than a
  second strip component. `orderDockTabs` reads a stored order tolerantly — a
  tab it has never heard of is dropped and one that is missing is appended — so
  a fourth tab needs no migration (a stored `recap` or `read`, from the two
  shapes that came between, is dropped the same way), and `reorderDockTabs`
  folds a drag done on a lecture with no Transcript tab back into the full
  order without shunting the hidden one to the end. Three labels are a squeeze
  at the 220px minimum, so the strip
  scrolls sideways with its scrollbar hidden rather than pushing the close
  button out of the header, and a fade at either end says so while there is
  more that way — the same gradient-not-`backdrop-filter` shape the transcript
  list's own edges use, 24px wide because it is covering a word rather than a
  paragraph. The
  drag-to-dock gesture is unchanged; the tabs keep the pointerdown to
  themselves, since `startDockDrag` captures the pointer and a click needs both
  of its ends on one target — and so does the **X at the end of the header**,
  which folds the dock away the way the control bar's button and T do. It is
  there because the bar is over the video and fades with it: a dock docked left
  on a paused lecture had its only close control on the far side of the player.
  The bar's button wears `SidebarSimple`, the app's fold-away mark from the
  sidebar and the chat's conversations column, **turned to face the edge the
  dock is on** (`DOCK_ICON_FACING`) — it stopped being the transcript's button
  when the panel grew tabs, and a page icon could not say which edge.
  **Chapters sits first because it is the shape of the hour**
  (`app/src/components/lectures/ChaptersPanel.tsx`): twelve cards taken in at
  once, which is the one thing a six-hundred-row list cannot do, so it is a
  plain scroller with `scrollIntoView({ block: "nearest" })` on the playing
  card rather than the transcript's follow machinery. **Transcript is the
  words, in either of two registers** — *Standard*, the cue list, and
  *Enhanced*, the reading copy's lines, one sentence each, pinned to its
  second, with the spoken maths set as maths
  (`app/src/components/lectures/ReadingList.tsx`). A picker in the search row
  and not a fourth tab
  (`app/src/components/lectures/TranscriptModePicker.tsx`), because the second
  is the first made readable: the same recording in the same order with the
  same seek on click, so the only thing that changes is the row source. That
  picker is also the enhance job's control — the source switcher's pattern,
  where the row for the thing not on disk yet starts the job that fetches it —
  and `modeInFront` falls back to the cues on a lecture that has no copy and
  no run, so the job is asked for in one place rather than from an empty
  panel. `transcriptMode` stores which one, beside `dockTab`. Both registers are the
  same list. ~600 lines and ~2500 cues are the same order of rows, so the
  follow machinery lives once, in
  `app/src/components/lectures/FollowList.tsx` — a row-agnostic list that takes
  a count, a key per row, the row playback is at and a `renderRow`, and owns
  the snap, the band rule, nudge / unfollow / soft-resume, the idle re-sync
  with its ring, the edge fades and the reopen-scroll — and each register keeps
  only its own mapping into row space, keying a row by what it *is* (a cue
  index, a line index) so a measured height survives a search. The two share
  one `following` flag in the player, since only one is mounted at a time.
  Lines arrive while the job that writes them is still running — it commits per
  window — so the list and the footer's running line are shown together. Each
  panel's footer holds one Regenerate for its own job; the menu of two that
  briefly sat there went with the tab that had both jobs in it. See
  [chapters.md](./chapters.md); Chat is [harness.md](./harness.md).
- **Chat is the tab every recording has, and that is why the dock is
  unconditional.** Transcript is dropped from the strip when there are no cues
  on disk, and Chapters can only show a job's state until one has run — but a conversation needs
  neither a file nor a run, so `hasDock` is gone from
  `app/src/components/lectures/LecturePlayer.tsx` and the panel is always
  mounted. Two things followed. The control bar's dock button names **the tab
  in front** (`tabInFront`, exported from `TranscriptPanel` so the strip and
  the button cannot disagree) instead of guessing from what the recording has;
  and T's third branch — fetch a transcript for a lecture whose dock would
  otherwise be empty — now fires on the *preference* being Transcript, since no
  such lecture is left and that is still the only place the player offers to
  fetch one.
- **The dock has a floor per tab.** 220px suits a cue and a chapter card; an
  agent's reply is markdown with fenced code in it and a composer underneath,
  so `useTranscriptDock` raises the minimum while Chat is in front (300 wide,
  260 tall). It is a floor on the *drawn* size and never rewrites the
  preference, so leaving the tab hands the transcript back the panel it had.
  The rows themselves are untouched: they are the Chat page's, and there is one
  timeline.
- **And a ceiling from the box it is in, because one preference has to serve
  three sizes of player.** The same stored width belongs to a fullscreen
  overlay, a full page and a docked side panel, and only the resize drag was
  ever clamped against the container — so a width dragged out in fullscreen was
  honoured whole in a panel a third as wide and left the video a sliver of
  black beside it. `useTranscriptDock` watches the player with a
  `ResizeObserver` and caps the drawn size at the container less the room the
  video keeps (`KEEP_W` / `KEEP_H`), which is the drag's own clamp applied to
  the sizes that never went through a drag. Past a point a cap is not enough:
  a container narrower than the panel's floor *plus* that room cannot hold both
  side by side, so a left or right dock **draws along the bottom** instead,
  where the floor to clear is a height even a narrow box has. The drop targets
  narrow to two bands while that holds, so the edge previewed under the pointer
  is the edge the drop can deliver. Nothing is rewritten either way — widening
  the window puts the dock straight back on the edge it was dropped on.
- **The transcript follows playback until the reader takes it over.** The
  list auto-scrolls to keep the playing cue in the middle band, then hands
  control over — a brand pill fades in at the bottom of the panel to go *Back
  to live*, and pressing play does the same thing implicitly. Following is
  tracked by pointer *intent* (wheel, touch drag, a press on the scrollbar)
  rather than the `scroll` event, because the follow-scroll fires `scroll` too
  and the two are indistinguishable after the fact.
- **Taking it over is a two-stage handover, not a hair trigger.** The first
  scroll only *nudges*: the list stops auto-scrolling so nothing snaps out from
  under the hand, but following stays on and no pill appears. Only a scroll
  that pushes the playing cue right out of the frame ends following — a glance
  that leaves the cue visible needs no way back, so it is offered none. The
  out-of-frame test runs in `scroll`, not in `wheel`, because a wheel event
  still reads the pre-scroll `scrollTop`. The rule runs in reverse too:
  scrolling the cue back into view *is* the *Back to live* press, so it is
  taken as one — softly, without the snap-to-centre, since the cue is already
  under the eye. The pill wears its own countdown: an SVG stadium outline whose
  dash offset drains over the eight seconds (`RING_STROKE`, driven by hand
  through the Web Animations API rather than a state flag, so restarting it on
  every wheel tick costs no render), so the pill going bare is the warning that
  the list is about to jump back. Either state re-syncs on its own
  after `IDLE_RESYNC_MS` (8 s) of the list being left alone; every scroll
  pushes that timer back, so it measures the hand stopping rather than the
  first touch. Only *resuming* following clears that timer — the unfollow edge
  has to leave it running, since the scroll event that ends following is the
  same one the last wheel tick armed the countdown from, and cancelling there
  left the pill sitting with a dead ring and no way back except by hand.
- **Transcript search narrows the list rather than walking it.** The field at
  the top of the panel filters to matching cues and marks the matched run in
  place, so the results read as a list of timestamps you can jump from. The
  window is therefore onto a `rows` array of cue indices, not onto `cues`: a
  row index and a cue index stop being the same number, and `rows[i]` is the
  only bridge between the two — the virtualizer is told row space, playback is
  cue space. Searching also suspends following (the playing cue may not have a
  row at all) and the query change re-runs `virtualizer.measure()`, since the
  height cache is keyed by row index and every row index just changed meaning.
  The needle is matched by `indexOf`, not a regular expression: it is whatever
  was typed, and a transcript is full of `.`, `(` and `?`. What must *not*
  happen on a query change is `virtualizer.measure()`: `getItemKey` keys the
  height cache by cue index, so heights already survive the reshuffle, and
  wiping the cache after the new rows have reported theirs drops every row back
  onto the estimate — two-line cues then overlap the ones below.
- **The list fades at both edges** when there is content past them, as a
  gradient rather than a `backdrop-filter`: a blur layer over a scrolling
  virtualised list next to a decoding video is the compositing cost the rest of
  the player is arranged to avoid. Two details keep the gradient from reading
  as a cut. It holds solid `background` for its first few pixels before it
  begins to ramp — the top one butts against the opaque search row, and text
  that is already half-visible a pixel below solid white reads as clipped, not
  faded — then spends the rest of its height dissolving, so the hold never
  thickens into a band of white padding. And it ends at `background/0` rather
  than `transparent`, which is *transparent black*: interpolating to it drags
  the middle of the ramp grey and leaves a dirty smear across the cues.
- **The transcript list is virtualised, and has to be.** A 2-hour Echo360
  recording is ~2500 cues (measured across the local library; the longest is
  2552), so rendered in full the panel is over 12,000 nodes for WebKit to lay
  out and paint in a scroller sitting next to a decoding video — that was the
  floor on scrolling smoothness no matter how little React did. It windows to
  ~40 rows via `@tanstack/react-virtual` with measured (not assumed) row
  heights, since cues wrap to two lines often enough. The follow-scroll lives
  in `TranscriptPanel` rather than the player because the virtualizer is the
  only thing that knows where a cue sits: it snaps with `scrollToIndex` when
  resuming from far away (the target may never have been measured) and eases
  with a plain `scrollTo` when merely tracking the next cue.
- **What else keeps the player smooth**, and should stay: the follow-scroll
  runs off the active-cue index rather than `timeupdate` (re-issuing a smooth
  scroll every 250 ms cancels and retargets it forever, so it never lands),
  and the dock drag coalesces `pointermove` into one `requestAnimationFrame`
  with its `localStorage` write debounced.
- **Playback speed is YouTube's control** (`SpeedControl.tsx`): continuous in
  0.05 steps between −/+ nudges, with the common speeds as one-tap presets.
  `clampSpeed` in the prefs store snaps every route in — slider, nudge,
  restored preference — to a step inside the range, because both a fractional
  slider and repeated `+ 0.05` accumulate float dust that would otherwise
  reach the badge and the `===` that lights a preset. Two shapes are load-
  bearing: the trigger is an icon plus a **fixed-width** badge (the bare
  number in a hug-width pill re-laid out the whole row on every step), and the
  presets are a `grid-cols-6`, so they cannot wrap to a second line.
- Markdown rendering (Canvas bodies, parsed PDFs, Ed threads, agent replies)
  goes through `app/src/components/markdown/MdComponents.tsx` with KaTeX for
  math; PDF markdown quality therefore shows up directly in Chat results and
  file views. That module owns the maths half too — KaTeX's stylesheet, the
  `MATH` gate that decides whether a string is worth the plugins, and
  `normalizeMath`, which rewrites `\(…\)` / `\[…\]` into `$…$` / `$$…$$`
  because CommonMark eats the backslash escape before remark-math ever sees
  the delimiter. A ```mermaid fence is drawn rather than shown as source —
  above, and `Mermaid.tsx` beside this module. See
  [harness.md](./harness.md) for why both matter most in chat. An inline `<code>` that is *nothing but* a library path is drawn as a
  `FileChip.tsx` instead and opens the file, so a mention looks the same when
  the agent quotes it back as it did in the composer; a backticked command,
  flag or snippet stays code, and code keeps the monospace a chip does not
  have. **Images render inline**: an `![…](…)` (or a fenced attachment path)
  pointing into the library is resolved to an asset URL — a bare
  `courses/…`/`agents/attachments/…` src is not loadable by the webview — and
  opens full size on click, while a src with no scheme and no match keeps its
  alt text rather than leaving a broken-image glyph in the middle of a reply.
  `InlineMd` refuses pictures either way, since its output sits inside a
  button.
- **In-markdown links resolve locally when they can.** `FileViewer`
  (`app/src/components/files/FileViewer.tsx`) matches `../`-relative links
  and raw Canvas `/courses/…/files/<id>` / `/pages/<slug>` URLs against the
  subject's `files` rows (by `canvas_id`, path, or `source_url` — the scrape
  records the slug a page was fetched under, since Canvas keeps serving a
  renamed page's old URL) and opens the local copy in the same panel; only
  unresolvable links open externally, marked with an arrow-square-out icon.
