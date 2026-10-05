# UI system

The design rules every page follows, and the WebKit and CSS traps behind them.
Read this before any UI work.

## Where

| Piece | Location |
| --- | --- |
| Tokens, base resets, utilities (`page-scroll`, scroll fades) | `app/src/index.css` |
| Text selection scopes | `app/src/index.css`, `app/src/lib/selectScope.ts` |
| shadcn primitives | `app/src/components/ui/` |
| Persisted view state and collapsed groups | `app/src/hooks/useStoredState.ts` |
| Subject page width, loading rows and empty views | `app/src/components/subjects/SubjectPage.tsx`, `app/src/components/ui/PageParts.tsx` |
| Drag gestures | `app/src/hooks/usePointerDrag.ts`, `app/src/hooks/useCardDrag.ts`, `app/src/hooks/useFileDrop.ts` |
| Subject grouping and persisted collapsed groups | `app/src/lib/subjectGroups.ts`, `app/src/hooks/useCollapsedGroups.ts` |
| Table chrome | `app/src/components/ui/GridTable.tsx`, `app/src/components/ui/ViewTabs.tsx`, `app/src/components/ui/TablePagination.tsx` |

## One colour, two fonts, and a floating card

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
  isn't a heading element takes `font-display`. A reply's headings
  (`.md-compact`) take the body font.
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
- **Only a pane's page selects, as in a native app.** The body is
  `user-select: none`; text tags select again only inside a select scope — a
  pane's root in `TabPane` (`data-select-scope`), a dialog, a popover — and
  never inside a button or tab, so the sidebar, tab strip and side panel
  header never highlight. A drag-select stays in the scope it starts in:
  `app/src/lib/selectScope.ts` turns every other scope off while the button is
  held, so a drag across the split divider stops at its own pane. Give a new
  surface that renders outside a pane `data-select-scope` if its text should
  be copyable.
- **Dark mode is a `.dark` class on `<html>`** written only by `applyTheme`
  (`app/src/lib/theme.ts`); `@custom-variant dark` follows the class, not the OS.
- **Full pages scroll through `page-scroll`**, which reserves the scrollbar
  gutter so a page that starts overflowing doesn't jog sideways.
- **Every scroller fades its overflowing edges through `useScrollFade`**
  (`app/src/hooks/useScrollFade.ts`; `syncScrollFade` in
  `app/src/lib/scrollFade.ts` for non-React callers like the maths palette).
  It marks the element `data-scroll-fade="x|y|xy"` and `index.css` turns that
  into a mask, so no overlay and no background colour. Unchanged edge states
  skip style writes. Set `--scroll-fade` for a ramp other than 24px, and keep
  the scroller flush against what it sits on — padding between them leaves a
  visible gap under the fade.
- **Tables are full-bleed in `GridTable`**, header outside the scroller (or the
  bar runs down it). Alternate views are sibling `ViewTabs`/`PillTabs`, not a
  dropdown, over a fixed-height toolbar so switching never jolts the rows.
- **Subject tabs share `SubjectPage` for width and gutters**, `SubjectLoading` for skeleton rows and `SubjectEmpty` for the empty view. Per-tab content and actions stay with the page.
- **View preferences use `useStoredState`**, with readers that own defaults and validation; `useStoredSet` keeps collapsed group keys. Storage failures leave the live view usable.
- **No toasts, no bottom progress bars** — background jobs surface in the
  sidebar only. No placeholder UI, section-header icons, stat cards or filler.
- Slugs display through `humanizeSlug`, Canvas codes through `displayCode`
  ("MULT20015", not "MULT20015_2026_SM2"), both in `app/src/lib/format.ts`.

Subject glyphs load the full Phosphor catalogue (`loadIconCatalogue` in
`app/src/components/subjects/SubjectIcon.tsx`, cached for every glyph) only
for a stored custom icon or an opened picker; default Books stay in the
startup bundle. With a custom icon stored, `main.tsx` awaits the catalogue
before the first render so the icon never paints as a Book. The picker grid
mounts on open and loads through the same cache.

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
- **Never nest a `<button>` in a row that is a button** — WebKit drops the inner
  clicks; use `RowAction` (`app/src/components/lectures/LectureRow.tsx`).
