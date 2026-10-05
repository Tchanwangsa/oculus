# Shell, tabs and the side panel

The shell around every page: a strip of tabs, a router per pane, a docked
side panel, every window shortcut, and search.

## Where

| Piece | Location |
| --- | --- |
| Route table | `app/src/routes.tsx` |
| Shell: sidebar, tab strip, floating card, zoom | `app/src/layouts/AppLayout.tsx`, `app/src/components/sidebar/`, `app/src/components/tabs/TopTabBar.tsx` |
| Tabs, split panes, per-pane routers, titles from paths | `app/src/stores/tabStore.ts`, `app/src/components/tabs/TabPane.tsx`, `app/src/lib/tabRouters.ts`, `app/src/components/tabs/tabInfo.tsx` |
| Window shortcuts (all menu items) | `app/src-tauri/src/menu.rs` |
| ⌘-click → a new tab, app-wide | `app/src/lib/newTabClicks.ts` |
| Search (⌘K and the new-tab field), Recent group | `app/src/lib/search.ts`, `app/src/lib/searchFilters.ts`, `app/src/components/search/SearchList.tsx`, `app/src/stores/recentTabsStore.ts` |
| Side panel (file/lecture peek) | `app/src/components/panel/`, `app/src/stores/sidePanelStore.ts` |

## Each pane has its own router, and the path is its only state

`app/src/routes.tsx` is the route table, and each **pane** — a tab, or one half
of a split tab — builds its own memory router over it
(`app/src/components/tabs/TabPane.tsx`). Paths are available synchronously for
matching and ⌘-click; page modules load through route `lazy` only when
visited, except Home and `/new`, which are in the startup bundle. A pane
opening on any other page shows `LoadingFill` while its module loads.
`TabPane` is memoised: a path change keeps unrelated tab objects stable, so
those panes skip shell-driven renders. Display clocks pause in inactive panes
and refresh when the pane returns; downloads and agent jobs continue globally.
The shell sits above all of them and navigates through `navigateActive` /
`goInActiveTab` in `app/src/lib/tabRouters.ts`, which resolve to the focused
pane and hold the departure rules: close the peek, keep a browser tab pinned
to its page, ask before leaving a playing lecture.

- **A tab is titled from its path alone** (`tabInfo`, shared with Recent), so a
  title not in the path rides in the query — `?n=` from `projectHref` /
  `taskHref`, `?t=` on `/lecture` — and a page re-`navigate`s to its own href
  (`replace: true`) after a rename.
- **`/chat` carries the thread, not just its name**: `?t=<id>&n=<title>`
  (`chatHref`). Every tab has its own router, so two Chat tabs hold two threads
  and a restored tab or Recent row reopens its own; a bare `/chat` is the
  history page ([harness.md](./harness.md#each-chat-tab-owns-its-conversation-in-its-route)).
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
  gives the menu bar every ⌘-key before the app's webview sees it. They reach
  the frontend as `menu-*` events and the frontend owns what each means.
  Close Window is ⇧⌘W, and the Edit submenu must stay or ⌘C/⌘V die in every
  field.
- **A browser page gets ⌘-keys before the menu**, as in Safari, so a site's
  own ⌘Z/⌘A/⌘F (Google Docs) work; a key the page doesn't take falls through
  to the menu. The app's tab and window keys (⌘T, ⌘W, ⌘N, ⌘K, ⌘L, ⌘1–9 and
  their ⇧/⌥ variants) never go to the page. The monitor is Rust's
  (`app/src-tauri/src/keys.rs`).
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
  chip is the one exception, carried by `openCitation` in
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

- File and lecture bodies load lazily when first opened; the always-mounted
  frame owns pane identity and the slide/width lifecycle while they load.
- `open()` takes no pane id — background panes are `inert`, so a click can only
  come from the front. A list re-fetching an open row calls `sync()`, which
  names its pane.
- **It unfolds on a count of `open` calls** (`opens`), so re-opening the item
  already showing still unfolds a folded panel.
- **Shell navigation closes it** (`navigateActive` → `closeActivePanel`).
  Expand goes to the full page in the same tab, committed at the end of a width
  sweep; the frame stays mounted at zero width so open and close animate.
- ⌥⌘S/⌘B/⌥⌘B test `e.code`, because ⌥ rewrites the key's character on macOS.

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

## Gotchas

- **A `Link`'s ⌘-click reloads the whole webview** — leave modified clicks to
  `newTabClicks.ts`.
- **A window shortcut skips a key an editor already took** — `AppLayout`'s
  ⌘B (sidebar) checks `defaultPrevented`, since the note editor's ⌘B is bold.
  ⌘K is a menu item, so it never reaches a keymap: the palette's `menu-search`
  handler asks `linkInFocusedNote` first, and a focused note makes a link.
  `app/src/lib/noteShortcuts.ts` holds callbacks registered by mounted editors,
  so listening for the menu does not load CodeMirror.
- **`data-tauri-drag-region` needs `core:window:allow-start-dragging`**
  (`app/src-tauri/capabilities/default.json`) or it silently does nothing.
