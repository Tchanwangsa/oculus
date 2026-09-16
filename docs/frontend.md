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
| ⌘K palette (search + go to) | `app/src/components/palette/CommandPalette.tsx`, `app/src/stores/paletteStore.ts` |
| Per-subject layout (underline tabs) | `app/src/layouts/SubjectLayout.tsx` |
| Subject tab pages | `app/src/pages/subject/` |
| Home (the launcher: composer, Today, Continue, Projects) | `app/src/pages/HomePage.tsx`, `app/src/components/home/` |
| Chat (a CLI agent's thread list, timeline, composer) | `app/src/pages/ChatPage.tsx`, `app/src/components/harness/`, `app/src/stores/harnessStore.ts`, `app/src/lib/harness.ts` |
| Calendar (month / week / upcoming) | `app/src/pages/CalendarPage.tsx`, `app/src/components/calendar/`, `app/src/lib/calendar.ts` |
| Projects (index, one board, a subject's tab) | `app/src/pages/ProjectsIndexPage.tsx`, `app/src/pages/ProjectPage.tsx`, `app/src/pages/subject/ProjectsPage.tsx`, `app/src/components/projects/`, `app/src/stores/projectsStore.ts`, `app/src/lib/projects.ts` |
| Overlap packing, shared by the week grid and the project timeline | `app/src/lib/lanes.ts` |
| Agent/model/reasoning picker (settings rows + composer) | `app/src/components/harness/ModelPicker.tsx` |
| Sync page + runner | `app/src/pages/SyncPage.tsx`, `app/src/lib/syncRunner.ts` |
| Settings | `app/src/layouts/SettingsLayout.tsx`, `app/src/pages/settings/` |
| Library counts, the MinerU token + the privacy statement | `app/src/pages/settings/LibraryPage.tsx` |
| Embedding backend, the index run, the re-index it costs | `app/src/components/settings/EmbeddingSection.tsx`, `app/src/components/settings/ReindexConfirmDialog.tsx`, `app/src/stores/indexStore.ts`, `app/src-tauri/src/embed/commands.rs` |
| Side panel (file/lecture preview) | `app/src/components/panel/`, `app/src/stores/sidePanelStore.ts` |
| In-app browser (route, tab mirror, API) | `app/src/pages/BrowserPage.tsx`, `app/src/hooks/useBrowserTabs.ts`, `app/src/stores/browserStore.ts`, `app/src/lib/browser.ts`, `app/src-tauri/src/browser.rs` |
| Viewers | `app/src/components/files/PDFViewer.tsx`, `app/src/components/files/FileViewer.tsx`, `app/src/components/lectures/LecturePlayer.tsx`, `app/src/components/lectures/ChaptersPanel.tsx` |
| Parse state: the words, and the row badge / viewer notice | `app/src/lib/parseState.ts`, `app/src/components/files/ParseState.tsx` |
| shadcn components (source, editable) | `app/src/components/ui/` |
| Table chrome: view tabs, footer pagination | `app/src/components/ui/ViewTabs.tsx`, `app/src/components/ui/TablePagination.tsx` |
| Zustand stores | `app/src/stores/` |
| Hooks | `app/src/hooks/` |
| DB access (tauri-plugin-sql) | `app/src/lib/db.ts` |

## Routes

The route table is `app/src/routes.tsx`, and each tab builds its own memory
router over it (`app/src/components/tabs/TabPane.tsx`) — the shell is above all
of them, so there is no one router to name. `/` is **Home**; it used to
redirect to `/chat`. Then `/chat`, `/calendar`, `/projects`,
`/projects/:projectId` and `/projects/:projectId/tasks/:taskId`, `/subjects`, `/subjects/:subjectId` (SubjectLayout →
overview / modules / downloads / lectures / announcements / assignments /
discussion / projects), `/subjects/:subjectId/file` and `/lecture` (the side
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
`?n=` for the same reason (`taskHref`). It sits under its project rather than
at `/tasks/:id` because the page cannot draw anything without the project: a
status is a column on *that* board, and `moveTask` has to be handed an id the
board actually has. Both pages re-`navigate(…, { replace: true })` to their own
href after a rename, so the tab you are looking at re-titles itself rather than
waiting to be reopened — which is the cost of titling from the path, paid at
the one moment it shows.

## How it connects

- **Settings → Library states the parser rather than offering it.** MinerU
  cloud is the only one, so there is no backend control and no memory budget —
  both described the Python sidecar. What is left is the library counts, the
  MinerU token, and the privacy boundary written as a fact: every PDF goes to
  MinerU and its PRC-hosted OSS storage.
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
  cannot offer what the backend would refuse. `Engine::Local` is real in the
  seam and has no server to talk to yet, so it is listed and disabled with that
  sentence under the control rather than hidden.
- **The same section starts and stops the index run**
  (`app/src/stores/indexStore.ts`). Until it existed the index could only be
  built by `oculus index` from a terminal: `embedFile` and `embedPending` were
  both written and wired to Rust and neither had a caller, so a library could
  sit permanently unsearchable with nothing in the app admitting it. One run at
  a time — `embedPending` is serial because the backend paces itself against
  the account's per-minute ceiling, so a second loop would only split the same
  tokens between two — and it can legitimately last hours, which shapes the
  rest: progress names the file it is on rather than only a percentage, the
  state lives in the store so it survives navigating away, and **stopping is
  cooperative and lands between files**, never mid-document, because abandoning
  one mid-flight would waste the quota already spent on its pages. Per the
  house rules there is no toast and no bottom bar: the page that owns the index
  shows the detail, and a spinner on the sidebar's Sync row is the whole of its
  presence elsewhere.
- **The stats rows distinguish the two spaces, and the labels had to change to
  do it.** `IndexStats.model` is the space this build *writes* — `stats` reads
  it off the seam's constants so it can answer before a key exists — not the
  space the stored vectors are in. Shown as "Vector space" it claimed the
  library held Voyage vectors while every one of them was Qwen, so it is
  "Search space" now, with `pages_stale` and `stale_models` carrying the stored
  side and naming the model that wrote them.
- Token save/remove invokes Rust keychain commands, never DB writes. Saving
  checks the token against MinerU first, so a bad or expired one is named at
  that moment, and it lifts the app-wide parse latch — the in-process client
  keeps no rejection state of its own, so nothing else would
  ([parsing.md](./parsing.md)). A token MinerU refuses mid-parse shows as
  Expired, read from that latch in `parseStore` rather than polled: the state
  is session-scoped, because the next parse reads the keychain afresh.
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
  **sideways scroller**, not a row that gets clipped: eight tabs already crowd
  the centred column and the card is narrower still with the side panel docked
  open, so the row scrolls with its bar hidden, a fade over each live edge (the
  sidebar's affordance) and the active tab scrolled into view. The `-mb-px`
  that lands the active underline on the header's border sits on the scroller,
  not on the tabs: `overflow-x` clips on both axes, so inside it the underline
  would go with it.
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
  the chat timeline and the player's recap notes
  (`app/src/components/markdown/MdComponents.tsx`), and its sizes are baked in
  as utilities. A reply and a note are a smaller register than a lecture page,
  so both go through `CompactMd`, which wraps them in `.md-compact`, and
  `app/src/index.css` scales the type down under that class alone. That block
  is the one rule in the file that is deliberately **unlayered** — layer order
  beats specificity, so the same rules inside `@layer base` would lose to the
  very utilities they exist to override. Base *resets* still belong in the
  layer; this is the inverse case.
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
  textarea keeps its ref, focus and draft text across the switch. Its
  controls are `app/src/components/harness/ModelPicker.tsx` — agent, model and
  reasoning level in a single trigger, with the vendor marks from
  `ProviderMark.tsx` beside them — and `SubjectSelect.tsx`, which scopes the
  thread to one subject or leaves it library-wide. `@` in the textarea opens a
  file menu narrowed to that subject and writes the picked file's library path
  into the message; nothing is read or attached in the frontend. The menu
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
  over it. The crumbs are **buttons through `navigateActive`, not `Link`s**:
  a `Link` addresses the pane's own router and so slips past the departure
  rules the shell keeps at that one door — which is how a crumb became the
  only way out of a playing lecture that never asked (see the player section
  below).
- **The ⌘K palette searches titles, not pages.** `CommandPalette.tsx` matches
  subjects, files, lectures and the app's own routes, from SQLite, on every
  keystroke — `searchLibraryFiles` / `searchLibraryLectures` in
  `app/src/lib/db.ts`. It deliberately does *not* reach the semantic index:
  that is a cloud embedding round-trip
  ([retrieval.md](./retrieval.md)) and belongs to a question you ask Chat, not
  to a field you are still typing in. The two searches share one matching
  rule — every typed word must appear somewhere in the haystack, in any order,
  so "algorithms graph" finds `graph-algorithms.pdf` — and the haystack
  flattens `-`/`_` to spaces and appends the subject code, because a file is
  on disk as a slug but reads as a title (`humanizeSlug`) and "comp30026
  workshop" should be one query rather than a filter plus a query. Ties break
  towards this term's coursework and then towards what was opened last, which
  is what makes the empty field a list of where you just were. Enter goes
  there in the current tab and ⌘↵ in a new one, the rule the sidebar's Recent
  rows already follow; a file opens as its **full page**, not in the side
  panel, because the panel closes itself the moment the route is not its
  subject's.
  The store exists only because the two ways in are far apart: the palette
  hears the menu event itself, and the sidebar's Search row is the visible
  handle that teaches the shortcut.
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
  dies with it — and so does Home, which every empty tab starts on.
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
  the address bar across the top of the content card and leaves an empty
  slot under it; the page is a WKWebView parked over that slot. Rust owns
  the tab list and pushes a `browser-state` snapshot on every change;
  `useBrowserTabs` (mounted once in `AppLayout`) mirrors it into
  `browserStore` and reconciles it with `tabStore` — a tab Rust has that the
  strip lacks opens in front, a strip tab whose page is gone closes. Page
  navigations change the tab's URL in Rust and nothing else, which is what
  killed the first version's loop (page load → router → re-layout → title →
  router again). A browser tab is kept pinned to its page on the way
  *in*, by `navigateActive` (`app/src/lib/tabRouters.ts`): a shell click that
  would take it anywhere else opens a tab of its own instead. While a browser
  tab is in front the strip's arrows drive the page's history, not the
  router's.
- **A native page cannot interleave with the DOM.** Anything drawn over the
  slot — a sidebar popover, a tooltip reaching in, a dialog — would render
  beneath the page, so `BrowserPage` watches `document.body` for portals
  whose rect lands on the slot and hides the page until they are gone. A tab
  going to the background hides it for the same reason, and that one needs
  saying out loud: panes stay mounted, so leaving a browser tab is no longer
  an unmount — `BrowserPage` takes the page down when `useTabActive` goes
  false, or it would stay parked over whichever tab came forward. The
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
- **The ingest pipeline is two stages: `download → parse`.**
  `app/src/stores/pipelineStore.ts` holds one row per PDF-backed file and
  `app/src/components/sync/PipelineTable.tsx` draws it — two stage dots, one
  progress figure, a timeline of the two steps when a row is expanded. It was
  four (a fast local parse, then a quality one, then an embed); MinerU cloud
  is the only parser now and embeddings are gone, so a file is complete the
  moment its parse lands.
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
- **A file says whether it has markdown, and why not.** MinerU cloud is the
  only parser and nothing catches it, so a failure means that file has no
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
  front both arrows stay enabled and drive the page's own history through
  Rust instead — what a native page has ahead of it isn't knowable from
  outside.
- **Window shortcuts are menu items, not key handlers.** macOS hands the menu
  bar every ⌘-key before a webview sees it, so ⌘T (new tab), ⌘W (close tab)
  and ⌘K (the palette) live in `app/src-tauri/src/menu.rs` and reach the
  frontend as `menu-new-tab` / `menu-close-tab` / `menu-search` events. That
  routing is a feature: they work while a browser tab's native page holds
  focus and the app's own webview is receiving no keys at all — which for the
  palette is the point, since ⌘K is how you get back out of a browser tab. It is also why the menu is built by
  hand — Tauri's default spends ⌘W on Close Window, which moves to ⇧⌘W here
  — and why the Edit submenu must stay: without it ⌘C/⌘V stop working in
  every text field.
- **Zoom scales the window, not a div.** `app/src/layouts/AppLayout.tsx`
  drives the webview's own page zoom (⌘+/⌘−/⌘0, persisted in
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
  transcript shown/hidden, the dock's tab, side and size, and the
  two-source arrangement (layout, which stream is in the main frame, the PIP
  box, the stacked split), as one global set persisted under a single
  `localStorage` key — set 1.5×, a left-docked transcript and a camera inset
  once and every recording opens that way. The only per-lecture playback state
  is the position, and that is a DB column (`lectures.progress_seconds`), not a
  preference. Speed and volume are applied in effects keyed on the *leader
  element*, not just the value: `playbackRate` and `volume` are per-element and
  reset on load, so a source switch has to re-apply both to the decoder that
  just took over the audio.
- **The dock is four readings of one recording.** The panel's header is a
  `ViewTabs` strip — Chapters, Recap, Transcript, Chat — and which one is in
  front is a player preference (`dockTab`) like the side it is docked to. Four
  labels do not fit the 220px minimum, so the strip scrolls sideways with its
  scrollbar hidden rather than pushing the close button out of the header. The
  drag-to-dock gesture is unchanged; the tabs keep the pointerdown to
  themselves, since `startDockDrag` captures the pointer and a click needs both
  of its ends on one target — and so does the **X at the end of the header**,
  which folds the dock away the way the control bar's button and T do. It is
  there because the bar is over the video and fades with it: a dock docked left
  on a paused lecture had its only close control on the far side of the player.
  The bar's button wears `SidebarSimple`, the app's fold-away mark from the
  sidebar and the chat's conversations column, **turned to face the edge the
  dock is on** (`DOCK_ICON_FACING`) — it stopped being the transcript's button
  when the panel grew tabs, and a page icon could not say which edge. The chapter list is not the transcript's follow
  machinery: twelve rows fit the panel, so the current card is a
  `scrollIntoView` and nothing else, where ~2500 cues earn a virtualizer, a
  two-stage handover and a countdown ring. The Recap tab is the same list an
  order of magnitude denser and read as prose rather than as navigation — every
  note's body is on screen, and notes arrive while the job that writes them is
  still running. See [chapters.md](./chapters.md); Chat is
  [harness.md](./harness.md).
- **Chat is the tab every recording has, and that is why the dock is
  unconditional.** Transcript is dropped from the strip when there are no cues
  on disk, and Chapters can only show a job's state — but a conversation needs
  neither a file nor a run, so `hasDock` is gone from
  `app/src/components/lectures/LecturePlayer.tsx` and the panel is always
  mounted. Two things followed. The control bar's dock button names **the tab
  in front** (`tabInFront`, exported from `TranscriptPanel` so the strip and
  the button cannot disagree) instead of guessing from what the recording has;
  and T's third branch — fetch a transcript for a lecture whose dock would
  otherwise be empty — now fires on the *preference* being Transcript, since no
  such lecture is left and that is still the only place the player offers to
  fetch one.
- **The dock has a floor per tab.** 220px suits a cue and a chapter title; an
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
  the delimiter. See [harness.md](./harness.md) for why that matters most in
  chat.
- **In-markdown links resolve locally when they can.** `FileViewer`
  (`app/src/components/files/FileViewer.tsx`) matches `../`-relative links
  and raw Canvas `/courses/…/files/<id>` / `/pages/<slug>` URLs against the
  subject's `files` rows (by `canvas_id`, path, or `source_url` — the scrape
  records the slug a page was fetched under, since Canvas keeps serving a
  renamed page's old URL) and opens the local copy in the same panel; only
  unresolvable links open externally, marked with an arrow-square-out icon.
