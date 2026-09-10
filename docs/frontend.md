# Frontend

React 19 + Vite + Tailwind v4, hash-routed, Notion-style layout. UI
conventions (palette, shadcn, icons, no-toasts) are in the root `CLAUDE.md` —
this page is the structure.

## Where

| Piece | Location |
| --- | --- |
| Router + event bridge | `app/src/App.tsx` |
| Shell: sidebar + top tab strip | `app/src/layouts/AppLayout.tsx`, `app/src/components/sidebar/`, `app/src/components/tabs/TopTabBar.tsx` |
| App menu (⌘T / ⌘W and friends) | `app/src-tauri/src/menu.rs` |
| Per-subject layout (underline tabs) | `app/src/layouts/SubjectLayout.tsx` |
| Subject tab pages | `app/src/pages/subject/` |
| Chat (agent loop, streamed) | `app/src/pages/ChatPage.tsx`, `app/src/stores/chatStore.ts` |
| Calendar (month / week / upcoming) | `app/src/pages/CalendarPage.tsx`, `app/src/components/calendar/`, `app/src/lib/calendar.ts` |
| Provider/model pickers (settings + composer) | `app/src/components/llm/` |
| Sync page + runner | `app/src/pages/SyncPage.tsx`, `app/src/lib/syncRunner.ts` |
| Settings | `app/src/layouts/SettingsLayout.tsx`, `app/src/pages/settings/` |
| Parse backend, memory budget + sidecar health | `app/src/pages/settings/LibraryPage.tsx` |
| Peek panel (file/lecture preview) | `app/src/components/peek/` |
| In-app browser (route, tab mirror, API) | `app/src/pages/BrowserPage.tsx`, `app/src/hooks/useBrowserTabs.ts`, `app/src/stores/browserStore.ts`, `app/src/lib/browser.ts`, `app/src-tauri/src/browser.rs` |
| Viewers | `app/src/components/files/PDFViewer.tsx`, `app/src/components/files/FileViewer.tsx`, `app/src/components/lectures/LecturePlayer.tsx` |
| shadcn components (source, editable) | `app/src/components/ui/` |
| Table chrome: view tabs, footer pagination | `app/src/components/ui/ViewTabs.tsx`, `app/src/components/ui/TablePagination.tsx` |
| Zustand stores | `app/src/stores/` |
| Hooks | `app/src/hooks/` |
| DB access (tauri-plugin-sql) | `app/src/lib/db.ts` |

## Routes

`createHashRouter` in `app/src/App.tsx`: `/chat`, `/calendar`, `/subjects`,
`/subjects/:subjectId` (SubjectLayout → overview / modules / downloads /
lectures / announcements / assignments / discussion), `/subjects/:subjectId/file`
and `/lecture` (peek promoted to a full Notion-style page, outside
SubjectLayout on purpose), `/sync`, and `/settings/*`. Legacy routes
(`/lectures`, a subject's `files` tab) redirect.

## How it connects

- Settings → Library owns parse preferences (`parse` in the `settings`
  table). Saves update SQLite and `sidecar_set_limits` in order; no restart
  is needed. The memory input has a 5 GB floor and recommends 8 GB. Health
  polling displays whole-tree memory, peak and recoveries inline, with no
  toasts. Backend changes affect new parse requests.
- Choosing MinerU cloud or Automatic opts into uploading PDFs to MinerU's
  PRC-hosted service; Local only is the default. Token save/remove invokes
  Rust keychain commands, never DB writes. Automatic is cloud-first when a
  token is available, with local fallback; see [sidecar.md](./sidecar.md).
  Saving checks the token against MinerU first, so a bad or expired one is
  named at that moment; a token MinerU later refuses mid-parse shows as
  Expired, read from health rather than polled.
- **Backend events are the write path.** `app/src/hooks/useBackendEvents.ts`
  (mounted once in `App.tsx`) listens for scrape/parse/auth Tauri events,
  upserts SQLite through `app/src/lib/db.ts`, and updates the stores. Pages
  read from the DB and stores; they do not talk to the scraper directly.
- In the app it is the **frontend** that owns scrape-table writes (the Rust
  engine only emits events); the CLI writes the same rows itself via
  `app/src-tauri/src/store.rs`. Change a table's shape and both writers must
  move together, plus the migration in `app/src-tauri/src/lib.rs`.
- **`sync_runs` is the only sync clock.** A subject's `last_synced_at` is not
  stored — `getSubjects` in `app/src/lib/db.ts` (and the CLI's
  `store::subjects`) derives it from the latest *completed* run whose
  `subject_codes` include the subject, so the subject list can never disagree
  with the history table and interrupted runs never count as a sync.
- `SubjectLayout` resolves the subject once and hands it to tab pages via
  outlet context — tab pages must not re-fetch it.
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
- `ChatPage` renders **one** composer in one of two places: centred under the
  hero while the chat is empty, docked at the bottom once there is a
  transcript. It is a single element moved between branches, not two, so the
  textarea keeps its ref, focus and draft text across the switch.
- Files and lectures open in the **peek panel** (`peekStore` +
  `app/src/components/peek/PeekPanel.tsx`); "expand" navigates to the
  full-page route. The top tab strip is `tabStore` +
  `app/src/components/tabs/TopTabBar.tsx`, Notion-style. It replaces the
  native title bar, so the strip and the empty space after the last tab carry
  `data-tauri-drag-region` to keep the window movable — which only works
  because `app/src-tauri/capabilities/default.json` grants
  `core:window:allow-start-dragging`; `core:default` does not include it, and
  without it the attribute silently does nothing.
- **The sidebar's Recent group is the router's trail, not the strip's.**
  `app/src/stores/recentTabsStore.ts` records a path on every navigation —
  from the same effect in `TopTabBar` that tracks the active tab, the one
  place that sees every move — and `app/src/components/sidebar/RecentNavGroup.tsx`
  lists the last five. Only the path is stored: the title and icon come from
  `tabInfo` (`app/src/components/tabs/tabInfo.tsx`), shared with the tab
  strip, so a recent page is named exactly as its tab is and a renamed
  subject follows on its own. Browser tabs stay out — `/browse/<id>` names a
  native page whose id dies with it. A row is active on path *and* query, or
  two files of one subject would both light up.
- **A subject's sidebar row lights on the subject, not on everything under
  it** (`end` on the `NavLink` in `SubjectsNavGroup`). A lecture or a file is
  a page of its own with its own tab, and lighting the subject row for it
  said you were somewhere you weren't.
- **External links open in the in-app browser, not in Safari.** One
  capture-phase click handler in `app/src/layouts/AppLayout.tsx` catches
  every `<a href="http…">` in the app — markdown links included — and hands
  it to `browser_open_url`, so no call site needs to know; ⌘-click still
  hands the URL to the real browser.
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
  router again). Two rules in `tabStore.trackNavigation` keep a browser tab
  pinned to its page: navigating away from it (the sidebar) opens a new tab
  instead, and history landing on a page already open elsewhere switches to
  that tab. While a browser tab is in front the strip's arrows drive the
  page's history, not the router's.
- **A native page cannot interleave with the DOM.** Anything drawn over the
  slot — a sidebar popover, a tooltip reaching in, a dialog — would render
  beneath the page, so `BrowserPage` watches `document.body` for portals
  whose rect lands on the slot and hides the page until they are gone. The
  slot is reported as insets from the window edges (CSS pixels times the
  page zoom from `--app-zoom`), re-measured by a `ResizeObserver` when the
  sidebar toggles or the zoom changes; window resizes are Rust's alone. The
  page rounds its own bottom corners to the card's inner radius, since the
  card cannot clip it.
- The calendar reads its own tables and never the scraper: `loadCalendar` in
  `app/src/lib/calendar.ts` pulls `calendar_events` plus `lectures` in one go
  and the page filters in memory, so month and week paging is arithmetic rather
  than queries. See [calendar.md](./calendar.md) for where the rows come from.
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
- **Tab shortcuts are menu items, not key handlers.** macOS hands the menu
  bar every ⌘-key before a webview sees it, so ⌘T (new tab) and ⌘W (close
  tab) live in `app/src-tauri/src/menu.rs` and reach `TopTabBar` as
  `menu-new-tab` / `menu-close-tab` events. That routing is a feature: they
  work while a browser tab's native page holds focus and the app's own
  webview is receiving no keys at all. It is also why the menu is built by
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
- `app/src/hooks/useQualitySweep.ts` periodically queries the DB for files in
  selected subjects whose `parse_status` is not yet `quality` and re-requests
  the pipeline for a few at a time (the sidecar skips whatever already
  exists). This is the recovery path for files that missed their quality
  pass — app closed mid-queue, sidecar down during a sync, parse died.
- `app/src/components/lectures/LecturePlayer.tsx` streams video from the
  Rust media HTTP server via `mediaSrc()` in `app/src/lib/media.ts` — not
  `convertFileSrc`, which WebKit's media stack rejects (see
  [architecture.md](./architecture.md)). Its scrub bar is a local `SeekBar`
  rather than the shadcn Slider, kept because `offsetX / offsetWidth` needs
  one measurement where Radix mixes `clientX` with `getBoundingClientRect`.
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
- **The video element outlives the route.** A tab switch is a navigation, and
  a navigation unmounts the page — which used to take the `<video>`, and the
  lecture, down with it. The element is a singleton owned by
  `app/src/lib/lecturePlayback.ts` instead: the player adopts it into its frame
  on mount and hands it back to an off-screen host (positioned away, *not*
  `display: none`, which is where a browser feels entitled to stop playback)
  on unmount, so a lecture keeps playing while you are in another tab and picks
  up on screen where it actually is when you come back — expanding the peek
  into its own tab included. This is a DOM node in the visible webview, not a
  hidden WebView; the suspension problem that keeps scraping in Rust does not
  apply. Because the element outlives the player, so does the progress writing:
  the module saves every 5s of playback and immediately on pause, seek, end and
  `pagehide`, then fires `LECTURE_PROGRESS_EVENT` for mounted lists to refresh
  on. **Playback belongs to a tab.** The module records which tab the
  player was mounted in, and that is what separates "switched away" from
  "left": going to another tab leaves the lecture playing behind a tab you can
  return to, while closing that tab or navigating it elsewhere would leave it
  playing with nothing owning it. Both of those ask first —
  `confirmLeavingLecture` (`app/src/stores/leaveLectureStore.ts`) raises one
  dialog mounted in `AppLayout`, saying the place is already saved — and stop
  playback on confirm. The navigation case is a `useBlocker` in the player; it
  tells a tab switch from a departure by whether the tab that owns playback is
  still the active one when the navigation starts (the strip activates the
  destination tab *before* it navigates). A paused lecture never prompts, and
  the peek's own close button is an outright stop.
- **The element's duration wins over the catalogue's.** Echo360's lesson
  duration comes from its scheduling data and the recording it serves runs a
  few seconds past it, so a clock counting against `lectures.duration_seconds`
  ended a lecture reading `1:55:00 / 1:54:46`. The player takes `video.duration`
  at `loadedmetadata` and counts everything — the clock, the scrub bar, the
  within-30s "complete" mark — against that, falling back to the DB value
  until it arrives or when there is no downloaded video. The lecture's own
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
  **off in the peek panel**
  (`allowFullscreen={false}`) — a peek is a panel over a page that stays
  mounted behind it; expand promotes the lecture to its own tab first.
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
  transcript shown/hidden, and the transcript's dock side and size, as one
  global set persisted under a single `localStorage` key — set 1.5× and a
  left-docked transcript once and every recording opens that way. The only
  per-lecture playback state is the position, and that is a DB column
  (`lectures.progress_seconds`), not a preference. Speed is applied to the
  element in an effect keyed on the source as well as the value, because
  `playbackRate` resets when a new source loads.
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
- Markdown rendering (Canvas bodies, parsed PDFs, Ed threads) goes through
  `app/src/components/markdown/` with KaTeX for math; PDF markdown quality
  therefore shows up directly in Chat results and file views.
- **In-markdown links resolve locally when they can.** `FileViewer`
  (`app/src/components/files/FileViewer.tsx`) matches `../`-relative links
  and raw Canvas `/courses/…/files/<id>` / `/pages/<slug>` URLs against the
  subject's `files` rows (by `canvas_id`, path, or `source_url` — the scrape
  records the slug a page was fetched under, since Canvas keeps serving a
  renamed page's old URL) and opens the local copy in the same peek; only
  unresolvable links open externally, marked with an arrow-square-out icon.
