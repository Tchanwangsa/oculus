# Shell, tabs and the side panel

The shell around every page: a strip of tabs, a router per pane, a side panel
beside each tab's page, every window shortcut, ⌘F, and search.

## Where

| Piece | Location |
| --- | --- |
| Route table | `app/src/routes.tsx` |
| Shell: sidebar rail, tab strip, floating card, zoom | `app/src/layouts/AppLayout.tsx`, `app/src/components/sidebar/`, `app/src/components/tabs/TopTabBar.tsx` |
| Tabs, side panels, per-pane routers, titles from paths | `app/src/stores/tabStore.ts`, `app/src/lib/sideStack.ts`, `app/src/components/tabs/TabPane.tsx`, `app/src/components/tabs/SidePanelHeader.tsx`, `app/src/components/tabs/PaneHeader.tsx`, `app/src/lib/tabRouters.ts`, `app/src/components/tabs/tabInfo.tsx` |
| Window shortcuts (all menu items) | `app/src-tauri/src/menu.rs` |
| ⌘-click → a new tab, app-wide | `app/src/lib/newTabClicks.ts` |
| Search (⌘K and the new-tab field), the new-tab page's Recent list | `app/src/lib/search.ts`, `app/src/lib/searchFilters.ts`, `app/src/components/search/SearchList.tsx`, `app/src/stores/recentTabsStore.ts` |
| Opening beside (file rows, citations, lecture rows) and a cited spot | `app/src/lib/openFile.ts`, `app/src/pages/subject/FilePage.tsx`, `app/src/hooks/useLocateHighlight.ts` |
| ⌘F: routing, find in rendered DOM, the find bar | `app/src/lib/find.ts`, `app/src/hooks/useDomFind.ts`, `app/src/lib/findText.ts`, `app/src/components/ui/FindBar.tsx`, `app/src/components/ui/PageFind.tsx`, `app/src/components/documents/editor/useEditorFind.ts` |
| Subject page: the nav column, subject switcher and `/subjects` redirect | `app/src/layouts/SubjectLayout.tsx`, `app/src/components/subjects/SubjectNav.tsx`, `app/src/pages/SubjectsRedirect.tsx` |
| Settings: the nav column, its search and section jumps | `app/src/layouts/SettingsLayout.tsx`, `app/src/components/settings/SettingsNav.tsx`, `app/src/lib/settingsSearch.ts` |

## Each pane has its own router, and the path is its only state

`app/src/routes.tsx` is the route table, and each **pane** — a tab's main page,
or an item in its side panel — builds its own memory router over it
(`app/src/components/tabs/TabPane.tsx`). Routers live in a registry keyed by
pane id (`app/src/lib/tabRouters.ts`) and outlive the mount, so a side panel
item sent to the back keeps its history. Paths are available synchronously for
matching and ⌘-click; page modules load through route `lazy` only when
visited, except Home, `/new` and the `/subjects` redirect, which are in the
startup bundle. A pane opening on any other page shows `LoadingFill` while its
module loads.
`TabPane` is memoised: a path change keeps unrelated tab objects stable, so
those panes skip shell-driven renders. Display clocks pause in inactive panes
and refresh when the pane returns; downloads and agent jobs continue globally.
The shell sits above all of them and navigates through `navigateActive` /
`goInActiveTab` in `app/src/lib/tabRouters.ts`, which resolve to the focused
pane and hold the departure rules: keep a browser tab pinned to its page, ask
before leaving a playing lecture.

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
  crumbs, ⌘-click, restored tabs and `tabInfo` all key off the path; `RailItem`'s
  `match` lights one sidebar item for both. A project is top-level because it may
  have no subject; a filed task sits under its project, an unfiled one at
  `/tasks/:taskId`, so `tabInfo` tests the task route first. The model is
  [projects.md](./projects.md).
- **`SubjectLayout` resolves the subject once** and hands it down as outlet
  context; tab pages must not re-fetch it. Its nav column
  (`app/src/components/subjects/SubjectNav.tsx`) holds the icon picker, the
  code as a switcher to another subject, and a row per tab. A switch
  navigates the pane's router to the same top-level tab of the other subject;
  anything deeper lands on that tab's default. Files is one tab whose sub-tabs are
  routes (`files/downloads`, `files/uploads`, `files/documents`), and its
  `useFilesTab()` context *extends* the subject, because `useOutletContext`
  reads the nearest Outlet and a different shape would make `useSubject()` lie.
- **`/subjects` is a redirect, not a page** (`SubjectsRedirect`): it replaces
  itself with the Overview of the last subject `SubjectLayout` resolved
  (`localStorage`, `oculus-last-subject`), else the first current subject,
  else the first past one. A remembered id no longer in the list is skipped,
  so a stale-id bounce from `SubjectLayout` cannot loop. It renders only with
  no subjects synced, as an empty view pointing at Sync. Every other way to a
  subject is the switcher, ⌘K or a crumb.
- **`/subjects/:id/file` and `/subjects/:id/lecture` sit outside
  `SubjectLayout`** so a document takes the whole card; `SubjectCrumbs` gives
  them a trail back, as buttons with `data-tab-href` rather than `Link`s so a
  click goes through `navigateActive`.

## The sidebar is an icon rail

`app/src/components/sidebar/Sidebar.tsx` is a 52px column of icon buttons,
each named by a tooltip on its right: Search (opens ⌘K), then Home, Chat,
Calendar, Tasks and Subjects, and Sync and Settings pinned to the foot under a
short hairline. A `RailItem` is a button with `data-tab-href` that goes
through `navigateActive`, since the sidebar sits outside every pane's router,
and it lights on its path or anything under it. Background jobs show on their
item's corner: a spinner on Chat while an agent turn runs, on Sync while
indexing, and a brand dot on Subjects while any subject has never-opened files
(`newFilesStore`). ⌘B and the title bar's button collapse it to zero width.

## Settings has its own nav column

`/settings` is one route with a page per nav row; `SettingsLayout` puts
`SettingsNav` beside the page, the rows grouped from `SETTINGS_PAGES`. Its
search reads a hand-kept list in `app/src/lib/settingsSearch.ts`, because the
pages are lazy and unmounted: entry titles are the on-screen text, and each
names the `Section` title it jumps to. Rename a section or row and update the
list. A jump travels as router state, and the layout scrolls the section's id
(`settingsSectionId`) into place once the page renders it.

## The shell owns tabs, side panels and every window shortcut

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
- **Undo and Redo are menu items that emit, not the predefined ones.** The
  predefined items hand ⌘Z to WebKit's own undo stack, which a note's
  CodeMirror history never fills. While a browser page holds focus Rust sends
  it the stock `undo:` / `redo:`; otherwise `menu-undo` / `menu-redo` go to
  `routeEdit` (`app/src/lib/editRouting.ts`): a note editor's history
  (`undoRouting.ts`, which also remembers the last focused note and acts only
  while it is visible), else `execCommand` on the focused input.
- **Select All is a custom item too**, because WebKit's `selectAll:` with no
  field focused selects the whole window. A focused browser page gets the
  native `selectAll:`; otherwise `menu-select-all` goes to `routeSelectAll`:
  the focused control's own ⌘A (replayed as a keydown, which CodeMirror and a
  table's cell block take; a maths field's `select()`), else a focused input's
  contents, else the dialog or popover holding focus, else the target ⌘F
  would search (`currentFindTarget` in `app/src/lib/find.ts`) — never the
  whole window.
- **⌘1–⌘8 are strip positions** (a missing slot is a no-op), ⌘9 is the last
  tab, and ⇧⌘T pops `tabStore`'s `closed` stack back to the old index. A
  browser tab is remembered by URL, recorded by `TopTabBar` *before* Rust
  destroys the page.
- **⌥⌘T and the strip's far-right button toggle the side panel**
  (`toggleSide`). `AppTab extends PaneState`: the main pane carries the tab's
  id, and everything below a tab — router, playing lecture, Recent entry — is
  keyed by pane id. `tab.focus` (`"main"` / `"side"`) is set in the capture
  phase, so it has moved before the clicked thing reacts; `focusedPane` is
  what every shell navigation resolves through. ⌃Tab / ⌃⇧Tab are menu items
  that cycle the side panel's items, only while it has focus; ⌘1–9, ⌘W and
  ⇧⌘T always act on the strip.
- **Strip tabs are fixed-width** (`TAB_W` down to `TAB_MIN_W`), and the column
  between them (`SEPARATOR_W`) always renders because the reorder maths counts it.
- **The history arrows follow the focused pane's own history.** A memory
  router has no `window.history` index, so `track` in `app/src/lib/tabRouters.ts`
  keeps each router's location keys and a cursor for `canBack`/`canForward`;
  on a browser tab they read Rust's snapshot (`can_back`/`can_forward`).
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
  and a listed page refreshes in place. Browser tabs, Home, `/new` and the
  `/subjects` redirect stay out.

## The side panel is a stack of items beside the page

Left is the tab's main page; right is its side panel (`tab.side`, pure logic in
`app/src/lib/sideStack.ts`), a stack of up to `SIDE_CAP` (8) panes. ⌥⌘T on a
closed panel opens one on `/new`.

- **`openBeside` is what file rows, citations and lecture rows call**
  (`openFileSmart`, `openCitation`, `LecturesPage`, `ContinueSection`): it
  pushes onto the front tab's stack and focuses the panel. A plain `Link`
  navigates its own pane; ⌘-click is still a new tab.
- **A push names a thing, not a path**: an item with the same `recentKey` comes
  to the front and is navigated, so re-citing an open PDF moves to the new
  spot. The list is most recently *opened* first, and fronting an item doesn't
  reorder it, so ⌃Tab walks a stable list; past the cap the least recently
  *viewed* item goes.
- **A cited spot rides in router state** (`{ locate }`), not the URL, since
  quotes are long. `FilePage` hands it to `FileViewer`; its `seq` makes
  re-citing the same spot jump again. A new tab carries it too:
  `addTab(path, state)` seeds the first router entry (`AppTab.entryState`), so
  ⌘-click on expand, a citation or a file chip keeps the cited spot. It is not
  stored in `oculus-tabs`.
- **The header is a switcher** — up to three stacked item icons, then `+N`,
  opening a list with a × per item — then expand and ×, which closes the panel
  and clears the stack. A page with its own top row (file, lecture, task,
  project, chat thread) builds it on `PaneHeaderRow`
  (`app/src/components/tabs/PaneHeader.tsx`), which in the side panel claims
  the header slot `TabPane` provides and draws the switcher at the row's start
  and expand and × after the page's buttons. Claims land in a layout effect,
  so any other page — or one still on `LoadingFill` — shows the standalone
  `SidePanelHeader` (switcher, front item's title, controls). In a claimed
  row `PaneTrail` makes crumbs and title a sideways scroller with faded edges,
  the subject crumb drops its icon, and every crumb but the subject (or, on a
  task, the project) folds behind a "…" in place, which expands inline for
  that page view. Expand
  (`expandSide`) makes the front item the main page, or a new tab on ⌘-click,
  and removes it with `handover`: a lecture it owned is claimed for the main
  pane (or the new tab), so whatever comes to the panel's front next waits and
  the expanded lecture plays on, and a browser page stays open.
- **Only the front item is mounted**; the rest are a path and a router. A
  lecture sent to the back pauses; a browser item sent to the back has its
  native view taken down as it unmounts, and a removed one has its page
  closed, or `useBrowserTabs` would adopt it as a strip tab.
- **The width is per tab**: a new panel takes half (`SIDE_RATIO`), and a drag
  of the seam is stored with the tab in `oculus-tabs`, beside the items' paths;
  a stored `split` pane restores as a one-item panel.
- **Nothing floats over a pane**: a native browser page covers anything drawn
  above it, so the controls sit in the header and the focus marker on the seam.
- **Shell navigation leaves the panel open**: the sidebar, ⌘K and crumbs go to
  the focused pane.

## ⌘F reaches one registered find

Edit ▸ Find, Find Next and Find Previous emit `menu-find*`, which `AppLayout`
hands to `routeFind` (`app/src/lib/find.ts`). Every mounted find registers a
target: a PDF viewer, a browser tab, a note editor, and a DOM find over each
pane — a tab's main page and its side panel's front item alike.

- **One target answers**: of those on screen, the innermost holding focus,
  else under the pointer, else where the last pointerdown or focusin landed,
  else the focused pane's page-level target (the side panel's front item
  while the panel has focus). Focus in a dialog or popover outside every
  target (the palette) reaches none. While a native browser page holds the
  keyboard `document.hasFocus()` is false and only the last step applies; a
  browser tab is its pane's page-level target.
- **A page hands ⌘F to its one find**: when the pointer, the last click or
  the pane fallback picks a page-level target or a DOM find, and exactly one
  non-DOM target sits directly inside it (no other target in between), that
  one answers — so the pointer over a PDF page's header searches the PDF.
  Focus never hands off: a text field on a page with one editor (a task's
  title) is how the rest of that page stays searchable.
- **Every pane has a page find** (`PageFind`, mounted by `TabPane`'s `Pane`):
  a floating `FindBar` at the pane's top-right (below a row holding the side
  panel's header), outside the page's scroller, over `useDomFind`. It is what
  searches chat, rendered files, tasks, subjects and the calendar. The
  standalone side panel header sits outside its item's pane, so it is not
  searched.
- **DOM find paints with the CSS Custom Highlight API** (`find-match` /
  `find-current` in `index.css`, shared by every instance), never the DOM.
  Text nodes join into one string with a separator at each block boundary
  (`app/src/lib/findText.ts`), so a match never crosses blocks. It skips
  `[data-find-skip]`, other targets' roots, collapsed `<details>` and
  undrawn elements; `FollowList` is marked because it is virtualised and its
  panels have their own search.
- **A DOM change under the root re-searches after 150ms**, keeping the
  current match (a live `Range`), so a streaming reply stays lit. A step
  scrolls the match into view only when it is out of view, rescaling rects
  for page zoom.
- **`FindBar` is a toolbar row or a floating card** (`variant`), with an
  optional replace row (`replace`) for editors. A note editor's bar is
  `useEditorFind` ([editor.md](./editor.md#find-and-replace-is-the-editors-own)).

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
