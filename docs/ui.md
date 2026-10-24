# UI system

The design rules every page follows, and the WebKit and CSS traps behind them.
Read this before any UI work.

## Where

| Piece | Location |
| --- | --- |
| Tokens, base resets, utilities (`page-scroll`, `skip-offscreen`, `text-shimmer` for text still at work, scroll fades) | `app/src/styles/colors.css`, `app/src/styles/dark.css`, `app/src/styles/base.css`, `app/src/styles/utilities.css`, `app/src/styles/scroll-fade.css` (`app/src/index.css` only imports them) |
| Themes, appearance prefs and the frame | `app/src/lib/ui/themes/` (`samplesLight.ts`, `samplesDark.ts` for the palettes, `derive.ts` for the tokens, `store.ts` for the prefs) |
| Settings → Appearance: mode, theme cards, accent, window frame, code font | `app/src/pages/settings/AppearancePage.tsx`, `app/src/components/settings/appearance/` |
| Platform detection and shortcut labels | `app/src/lib/platform.ts` |
| Text selection scopes | `app/src/styles/base.css`, `app/src/lib/ui/selectScope.ts` |
| shadcn primitives (flat) | `app/src/components/ui/` |
| Composites built on them | `app/src/components/ui/layout/`, `app/src/components/ui/table/`, `app/src/components/ui/search/`, `app/src/components/ui/lightbox/` |
| Persisted view state and collapsed groups | `app/src/hooks/ui/useStoredState.ts` |
| Subject page width, loading rows and empty views | `app/src/components/subjects/SubjectPage.tsx`, `app/src/components/ui/layout/PageParts.tsx` |
| Page headers, titles and breadcrumb parent popovers | `app/src/components/tabs/PaneHeader.tsx` (`PageHeader`, `PaneTitle`, `PaneTrail`), `app/src/components/ui/Breadcrumb.tsx` |
| Page navigation and its responsive shell | `app/src/components/ui/layout/SideNav.tsx`, `app/src/components/ui/layout/SideNavShell.tsx`, `app/src/hooks/ui/useSideNavPanel.ts` |
| Settings pane layout and stacking rows | `app/src/styles/settings.css`, `app/src/layouts/SettingsLayout.tsx`, `app/src/components/settings/SettingsNav.tsx` |
| Drag gestures | `app/src/hooks/gestures/usePointerDrag.ts`, `app/src/hooks/gestures/useCardDrag.ts`, `app/src/hooks/gestures/useFileDrop.ts` |
| Subject grouping and persisted collapsed groups | `app/src/lib/ui/subjectGroups.ts`, `app/src/hooks/shell/useCollapsedGroups.ts` |
| Flat divided lists, table chrome | `app/src/components/ui/layout/PageParts.tsx`, `app/src/components/ui/table/GridTable.tsx`, `app/src/components/ui/table/TableGroup.tsx`, `app/src/components/ui/table/ViewTabs.tsx`, `app/src/components/ui/table/TablePagination.tsx` |
| Toolbar filters: pill popovers, checklists, applied-filter chips | `app/src/components/ui/filter/` |
| The colour control: a swatch that opens a pad, hue and opacity strips and hex text (`opaque` drops the opacity strip), or with a `palette` a grid of colours, their shades and the hex code with the pad behind Custom | `app/src/components/ui/color/`, `app/src/lib/ui/color.ts` |
| Identity icons, shared picker and colour palette | `app/src/components/ui/icon/`, `app/src/lib/ui/iconPreferences.ts` |
| A titled block that folds, its state kept by the caller | `app/src/components/ui/CollapsibleSection.tsx` |
| Study widgets in markdown replies | `app/src/components/markdown/openui/components/` |
| Proposal cards, typed editors and result links | `app/src/components/markdown/openui/proposals/` |

## Study widgets use the reply's primitives and semantic tokens

OpenUI widgets use the same inline markdown and maths engine as replies,
`CitationCode` / `CitationLink` for PageRef, and the shared Radix Slider.
`Frame` supplies a rounded card with a hairline border, a wrapping title and
counter, and a footer of small pill controls. Quiz choices are ordinary
keyboard-focusable toggle buttons. Chart draws one SVG series with semantic
chart, border and muted-text tokens; controls use primary, brand, success and
destructive tokens. Nothing inside the widget sets a fixed pixel width, so
the cards reflow with the chat pane and page zoom. The runtime and its state
contract are in [harness.md](./harness.md#openui-fences-draw-interactive-study-widgets).

`Rich` repairs JSON's form-feed, tab, backspace and carriage-return escapes
before a letter, plus newline escapes inside dollar-delimited maths. It cannot
recover a lost backslash or distinguish an intended line break inside maths;
the agent must still double TeX backslashes in strings.

## Proposal cards show a decision before a write

Each proposal or job has its own OpenUI fence. The same rounded frame holds
editable creation rows, before → after changes, deletions and confirmed
results. Five rows fit a page; paging retains edits and inclusion ticks.
Subtasks group beneath their existing parent's live title, status and due
date. A single small proposal starts as one line; its title opens the editor.
Project creation splits into Attributes and Tasks tabs: Attributes holds the
name field and the project page's property pills (subject, due, start, tags,
with tags named against the task tag registry and created only on Create),
then the calendar event and brief. A result list longer than six rows ends in
Show more. A proposed task leads with the tasks table's status glyph, set to
the column Create would use (its own, its parent's, else the board's first)
and editable there; a created task leads with its live status, which moves
the task as the table's glyph does, and a created project with its icon.
Proposal headings use `role="heading"` so the surrounding reply's markdown
heading styles cannot add spacing or override the card's title type.

`DateTimeField` preserves date-only values as local calendar days, without
inventing a clock time. Proposal date pickers offer All day; turning it off
chooses an explicit time. Status names and column choices come from the real
board, including custom done columns. A stale change starts excluded and has
a warning tooltip with its live values; explicitly ticking it uses those
values as the new guarded baseline. Reverting every field removes that row
from the changes to apply.

Create, Apply, Discard and Undo show errors in the card. Confirmed results
link to the actual items and deletion results have no Open action. A valid
amendment collapses its predecessor to its version number and a newer-version
link, and dots changed fields against the student's saved card. Pending cards
offer a V1/V2 selector, Modified indicator and Reset; their snapshot and draft
contract is in [harness.md](./harness.md#proposal-context-is-read-at-dispatch-and-confirmation-lives-in-rust).
Decided cards keep their state
through reloads and multiple views; their database and send contracts are in
[harness.md](./harness.md#proposal-context-is-read-at-dispatch-and-confirmation-lives-in-rust).

Revision uses these same cards and Live-mode fields for cheat entries, a focused question surface for reviews, and a widget grid for artifacts. Nothing is added to every reply or to a text selection; the agent offers entries as a card. See [revision.md](./revision.md).

## One colour, two fonts, and a floating card

Quiet and neutral by default: dead-grey whites (no warm or blue cast —
anything else fights the accent), muted indigo `#5e6ad2` as the one colour,
Manrope for headings and Inter for everything else, Notion-style layout. Other
themes are opt-in and keep the same structure.

- **The shell frames a floating document.** The sidebar's icon rail and the
  tab strip sit on the window's ground with no fill or divider; content is an
  inset rounded `card` with a hairline border
  (`app/src/layouts/AppLayout.tsx`). Tabs and rail icons are pills and rounded
  squares on that ground. A sidebar divider or a rule under the strip breaks the
  effect — the card's border is the separation.
- **The ground is `--color-frame`, painted once on `#root`** (`base.css`); the
  shell and its chrome have no fill of their own. Opaque it is `background`;
  on macOS the frame pref makes it see-through (`FRAME_ALPHA` in
  `app/src/lib/ui/themes/apply.ts`) and asks Rust for glass behind the webview
  with `native_set_glass`. If that call fails the frame goes back to opaque.
  The card, popovers and dialogs stay opaque.
- **Shortcut labels follow the desktop platform** through `shortcut` in
  `app/src/lib/platform.ts`: Command glyphs on macOS, Ctrl/Alt/Shift names on
  Windows. The shell reserves the traffic-light inset only on macOS; Windows
  caption controls sit after the side-panel toggle, outside the tab strip's
  measured width ([shell.md](./shell.md#the-shell-owns-tabs-side-panels-and-every-window-shortcut)).
- **Buttons and chips are pills** (`rounded-full` in
  `app/src/components/ui/button.tsx`); rectangles are for segmented toolbars
  that override the radius at the call site.
- **Three primitives are a notch below stock shadcn**, whose sizes suit a 16px
  page where this app's body is 14px: `button.tsx` `default` is `h-8`;
  `dialog.tsx` is `p-5`/`rounded-xl` with a 16px title and 13px description;
  `tooltip.tsx` adds `max-w-64` and `break-words` so a long page title or file
  name wraps. Call sites don't override these — don't restore what
  `shadcn add` generates.
- **Monospace is for code and terminal output only** — `font-mono` appears in
  `app/src/components/markdown/MdComponents.tsx` and the code-font field
  (`app/src/components/settings/appearance/CodeFontRow.tsx`); the note editor's theme
  (`app/src/components/documents/editor/theme/`) gives `--font-mono` to code
  and LaTeX source, and `CodeEditor` (`app/src/components/code/`) and the Ed workspace's terminal
  (`XtermView` in `app/src/components/lessons/workspace/terminal/`) are
  monospace throughout. A command's output (`AnsiText`, the one-line
  `TerminalLine` on a running install or update) keeps its colours via
  `app/src/lib/format/ansi.ts`, which drops every escape but SGR. Timestamps, counts, IDs and badges take the body font,
  with `tabular-nums` when digits hold a column.
- Headings get Manrope from an `h1–h4` rule in `@layer base`; a title that
  isn't a heading element takes `font-display`. A reply's headings
  (`.md-compact`) take the body font.
- **shadcn/ui** lives in `app/src/components/ui` (config `app/components.json`,
  primitives from the unified `radix-ui` package). Add with
  `bunx shadcn@latest add <name>`, then swap `lucide-react` for
  `@phosphor-icons/react`.
- **Icons are Phosphor.** One Phosphor lacks is drawn in
  `app/src/components/icons/` on its `IconBase`, so it takes the same `size`
  and `weight` props.
- **Inline references have content colours** through `reference-*` tokens:
  red PDFs, purple pages and notes, blue documents and discussions, green
  tasks, amber projects, pink lectures and teal lessons. These glyphs identify linked
  content; the shell keeps its indigo accent ([viewers.md](./viewers.md)).
- **Colours go through the semantic tokens in `app/src/styles/colors.css`** (dark values in `dark.css`). In
  shadcn's vocabulary `accent` is the quiet hover surface, not the brand. The
  indigo has two tokens: `primary` is the *fill* (buttons, active underline,
  today), `brand` is the *accent* (links, selection, in-flight progress,
  new-item chips) — split so the accent can be retuned without restyling every
  button.
- **A theme is a small palette** (`ThemeSpec`: ground, card, popover, text,
  muted text, border, accent, nine syntax colours); `derive` mixes it in OKLab
  into every `--color-*` token but the chart slots and shadows, which stay the
  stylesheet's per mode. Oculus Light and Oculus Dark are the stylesheet
  itself and write nothing. Every token value is a resolved hex colour, never
  `var()` or `color-mix()`: mermaid reads them through `getComputedStyle`.
  `app/tests/lib/ui/themes.test.ts` holds each theme to 7:1 text, 4.5:1 muted
  text and button labels, and 3:1 syntax on `surface-raised`.
- **Only content selects, as in a native app.** The body is
  `user-select: none`; text selects again only inside a `data-selectable`
  block — a file's Markdown or PDF pages, a chat message (`CompactMd`, the
  question bubble, thinking), `CodeText`, a note's editor (maths fields
  included), `CodeEditor`, a document version, an error's
  detail — and never on a button or tab inside one. Page chrome (titles,
  cards, lists, tables, the sidebar) never highlights. Mark a new surface
  `data-selectable` only if its text is worth copying. A drag-select stays in
  the scope it starts in — a pane's root in `TabPane` (`data-select-scope`), a
  dialog, a popover: `app/src/lib/ui/selectScope.ts` turns every other scope off
  while the button is held, so a drag across the split divider stops at its
  own pane. DOM find briefly opts only matched text parents into selection so
  WebKit paints custom highlights without making a whole row selectable.
- **Appearance is one stored pref** (`AppearancePrefs` in localStorage
  `oculus-appearance`): light/dark/system, a theme per mode, an accent
  override, the frame and a code font.
  `applyAppearance` (`app/src/lib/ui/themes/apply.ts`), run from `main.tsx`
  before the first render, is the only writer of the `.dark` class and
  `data-theme` on `<html>`; it writes the theme's tokens inline there and
  clears the ones it wrote last time. `@custom-variant dark` (in
  `app/src/index.css`) follows the class, not the OS. Change prefs through
  `setAppearance`; read them with `useAppearance`. Code that bakes colours in
  (mermaid) redraws on `subscribeTheme`, not just on `.dark`.
- **A theme preview sets that theme's tokens inline on its wrapper**
  (`previewStyle` in `app/src/components/settings/appearance/`) and draws with
  plain token utilities, never `dark:` variants, so a dark card stays dark on
  a light page. Settings → Appearance's theme cards do this; each mode's grid
  shows one row (the chosen theme always in it) until "See all" opens it,
  remembered per mode.
- **Full pages scroll through `page-scroll`**, which reserves the scrollbar
  gutter so a page that starts overflowing doesn't jog sideways.
- **Settings respond to their pane's width**, including split panes and page
  zoom. `settings.css` stacks `settings-row` labels and controls below 34rem
  and uses the shared navigation drawer described below. The page scroller
  uses `useScrollFade`. Use
  `settings-controls` for bounded fields and `settings-actions` for wrapping
  button groups. Descriptions are short hints; errors and confirmation copy
  retain the detail needed to act. Dense tables scroll sideways with readable
  column widths; `GridTable`'s optional `minWidth` keeps header and rows together.
- **A long list's rows can skip layout off screen with `skip-offscreen`**
  (`content-visibility: auto`, remembering each row's drawn height), so a
  resize lays out only what is visible. The row clips its paint to its box:
  4px of padding on each edge, cancelled by the margin, keeps focus rings in, but
  anything drawn past its sides is cut, and popups must portal. The chat
  timeline uses it ([harness.md](./harness.md#the-timeline-mounts-its-newest-rows-and-holds-the-view-still)).
- **Every scroller fades its overflowing edges through `useScrollFade`**
  (`app/src/hooks/ui/useScrollFade.ts`; `syncScrollFade` in
  `app/src/lib/ui/scrollFade.ts` for non-React callers like the maths palette).
  It marks the element `data-scroll-fade="x|y|xy"` and `app/src/styles/scroll-fade.css` turns that
  into a mask, so no overlay and no background colour. Unchanged edge states
  skip style writes. Set `--scroll-fade` for a ramp other than 24px, and keep
  the scroller flush against what it sits on — padding between them leaves a
  visible gap under the fade.
- **Tables are full-bleed in `GridTable`**, header outside the scroller (or the
  bar runs down it). Alternate views are sibling `ViewTabs`/`PillTabs`, not a
  dropdown, over a fixed-height toolbar so switching never jolts the rows.
- **Row lists use `ListCard` as a flat divided stack**, without a card border
  around the rows. A page adds a card only when the content itself calls for one.
- **Filters are pills over popovers** (`FilterPopover`, brand-tinted while
  narrowing); a multi-select is a `FilterChecklist` (an option may carry the
  glyph its rows draw, before the label, with the count on the right), and the
  applied values sit under the toolbar as `FilterChips` — one line that scrolls
  with faded edges, never wraps. Inside a popover, `ChipRow` + `ToggleChip` do
  the same for a row of toggle chips; a long single-choice list sits in
  `FilterList` (fixed max height, `overflow-y: scroll` so the gutter is always
  reserved) with `FilterFold` for a collapsed group such as Archived. Rows
  and fold triggers draw keyboard focus inside their bounds, so the list
  cannot clip the highlight when the popover focuses its first option.
- **Settings, subjects, Chat, Tasks and Studio share one nav column**, `SideNav`: a 208px column
  on the window's `background` beside the page, a pinned header, rows with a
  16px icon and the active row filled, never an underline strip, and
  `SideNavSearch`, the pill search field under the header. Rows that are not
  links take its exported row classes. `SideNavShell` and `useSideNavPanel`
  own the responsive behavior ([shell.md](./shell.md#page-navigation-overlays-narrow-panes)).
  The subject, Chat, Tasks, Studio and Revision columns are drag-resizable and
  fold away to nothing (`useResizablePanel`, width and fold remembered) from
  ⌘⌥B, from `SideNavCollapseToggle` at the start of the
  page's 48px header row, beside its title, or by dragging it narrow. The
  toggle takes the panel itself; `useSideNavPanel` binds ⌘⌥B and names it in
  the panel's `shortcut`, so every toggle's tooltip shows it. Labels
  fade as the width eases shut (`SIDE_NAV_FOLDS`). Drag samples are coalesced
  into one width update per animation frame; release applies the last sample
  even before that frame arrives. The width and fold are saved to storage
  only after dragging ends, so synchronous storage stays out of the resize
  loop. Unmounting cancels the pending frame and clears the drag cursor.
- **Subject tabs share `SubjectPage` for width and gutters** under
  `SubjectLayout`'s header row (the toggle, the subject crumb, the tab's
  name) and beside the subject's nav column, `SubjectLoading` for skeleton
  rows and `SubjectEmpty` for the empty view. Per-tab content and actions
  stay with the page.
- **View preferences use `useStoredState`**, with readers that own defaults and validation; `useStoredSet` keeps collapsed group keys. Storage failures leave the live view usable.
- **No toasts, no bottom progress bars** — background jobs surface in the
  sidebar only, as a spinner or dot on their rail item
  ([shell.md](./shell.md#the-sidebar-is-an-icon-rail)). No placeholder UI,
  section-header icons or filler; stat cards only where the figures are the
  point (Home's Activity).
- Slugs display through `humanizeSlug`, Canvas codes through `displayCode`
  ("MULT20015", not "MULT20015_2026_SM2"), both in `app/src/lib/format/format.ts`.

Subjects and projects share `CatalogueIcon` and the searchable `IconPicker`
under `app/src/components/ui/icon/`. The catalogue loads only for a stored
custom glyph or an opened picker; Book and Cube defaults stay in the startup
bundle. `main.tsx` awaits that catalogue before its first render when either
identity store has a custom icon. The grid mounts only while open and shares
the same cache. Palette and partial-reset rules live in
`app/src/lib/ui/iconPreferences.ts`; subject preferences keep their existing
`oculus-subject-icons` key.

Every `BreadcrumbItem` requires an icon, shown both inline and in the parent
popover, including side panels. Subject categories use their navigation row's
glyph; subject and project crumbs retain their custom identity icons.

## Gotchas

- **Drag in-window on pointer events via `usePointerDrag`** (tab strip,
  `ViewTabs`, the sidebar rail, chat groups, boards and tables via `useCardDrag`); HTML5 DnD only
  for leaving the window.
- **HTML5 drag needs `dataTransfer.setData()`** or WebKit cancels it silently,
  and `preventDefault()` on `dragstart` cancels it outright — see the drag-out
  in `dragAsMarkdown` (`app/src/lib/markdown/selection.ts`), wired up in
  `app/src/components/harness/timeline/Timeline.tsx`.
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
  despite `PhysicalPosition` (`app/src/hooks/gestures/useFileDrop.ts`).
- **No `md:` text size on a base field** — variants emit after plain utilities,
  so `md:text-sm` beats every call-site size; `input.tsx`/`textarea.tsx` carry
  one `text-[13px]`.
- **Every base reset in `app/src/styles/base.css` goes inside `@layer base`** — unlayered CSS
  beats every utility, killing `border-*` and `select-*` app-wide.
- **WebKit only reads `-webkit-user-select`** — the unprefixed property is
  ignored, and the dev server doesn't autoprefix (only the production build
  does), so hand-written CSS writes both and inline styles set
  `webkitUserSelect`. The `select-*` utilities already emit both.
- **Page zoom, never CSS `zoom`** — in a CSS-zoomed subtree WebKit mixes visual
  pointer coords with layout rects, breaking popups and drags.
- **No native colour inputs** — `<input type="color">` opens the OS panel
  and has no alpha; use `ColorField` (`app/src/components/ui/color/`), whose
  pad and strips measure the pointer as a fraction of their own box, so page
  zoom cannot skew them. Its popover stays inside the viewport with a 12px
  collision inset and scrolls when height is limited. Controls on a left edge
  can open it to the right with start alignment.
- **No native date/time inputs** — segmented hover, an OS picker in Buddhist-era
  years, and a half-typed value reads empty. Use
  `app/src/components/projects/fields/DateTimeField.tsx`; bare `type="time"` is fine.
- **`scrollbar-gutter` is a no-op in WebKit** — reserve the gutter with
  `overflow-y: scroll` (`page-scroll`, `GridTable`, the lightbox).
- **Icons jitter half a pixel when a layer is promoted or dropped** — recurring.
  At a fractional page zoom (1.1, 1.3) WebKit re-snaps content by half a device
  pixel whenever an element gains or loses its own compositing layer, so icons
  step on a hover, a fade or an expand while the text beside them stays put.
  Known triggers: a scroller that only starts to overflow after something
  expands (`SwitcherList` in `app/src/components/subjects/nav/SubjectSwitcher.tsx`), and anything that transitions
  `opacity` or `transform` — a hover-reveal button (`MessageActions` in
  `app/src/components/harness/timeline/`, `ThreadRow`'s delete), a caret that fades and rotates, the
  label fade `SIDE_NAV_FOLDS`. All carry `will-change`, which keeps the layer on
  from the start. Whole-pixel row heights do not help. **Every new
  `transition-opacity` / `transition-transform` on an icon, or on a row holding
  one, gets `will-change-[opacity]` (or `[opacity,transform]`)** — grep for
  `transition-` before finishing UI work, and check at zoom 1.1 and 1.3, not 1.0.
  A rounded `overflow-hidden` ancestor also needs `will-change-transform`
  when its children composite: the share-link field keeps its clip promoted
  through hover and the native share button's disabled fade. Promoting only
  the buttons or their SVGs leaves the ancestor's clip free to re-snap.
- **Never `transition-all`** — inactive tabs hide with `visibility: hidden`
  (`app/src/components/tabs/TabPane.tsx`), and `all` transitions the inherited
  `visibility` too: going hidden it stays `visible` for the whole duration, so
  the element ghosts over the next tab. List the properties instead
  (`transition-[color,background-color,…]`, as `Button` does).
- **Centre overflowing content with auto margins and `flex: none`**, not
  `justify-content: center`, which makes the start-edge overflow unreachable.
- **Never nest a `<button>` in a row that is a button** — WebKit drops the inner
  clicks; use `RowAction` (`app/src/components/ui/RowAction.tsx`).
- **A chip's × is `ChipRemove`** (`app/src/components/ui/ChipRemove.tsx`), the
  tab strip's close button as a component: muted to foreground over a subtle
  fill, a colour change only. `TagChip` and the task page's `FileChip` use it, as
  a sibling of the chip's open action; its `group` reveal fades in on a wrapper
  that already has `will-change`, so hovering does not move the icon. Never
  fade the button's own `opacity`.
