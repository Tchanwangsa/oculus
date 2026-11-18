# UI system

The design rules every page follows, and the WebKit and CSS traps behind them.
Read this before any UI work.

## Where

| Piece | Location |
| --- | --- |
| Tokens, base resets, utilities (`page-scroll`, `skip-offscreen`, `text-shimmer` and `block-shimmer` for content still at work, scroll fades) | `app/src/styles/colors.css`, `app/src/styles/dark.css`, `app/src/styles/base.css`, `app/src/styles/utilities.css`, `app/src/styles/scroll-fade.css` (`app/src/index.css` only imports them) |
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
| Right-click menus: rows, a muted icon column, hints, and a note under greyed items saying why | `app/src/components/ui/context-menu.tsx` |
| The colour control: a swatch that opens a pad, hue and opacity strips and hex text (`opaque` drops the opacity strip), or with a `palette` a grid of colours, their shades and the hex code with the pad behind Custom | `app/src/components/ui/color/`, `app/src/lib/ui/color.ts` |
| Identity icons, shared picker and colour palette | `app/src/components/ui/icon/`, `app/src/lib/ui/iconPreferences.ts` |
| A titled block that folds, its state kept by the caller | `app/src/components/ui/CollapsibleSection.tsx` |
| Study widgets in markdown replies | `app/src/components/markdown/openui/components/` |
| Agent-written widgets and the kit they are built from | `app/src/components/markdown/widget/`, `app/widget-kit/` |
| Proposal cards, typed editors and result links | `app/src/components/markdown/openui/proposals/` |

## Study widgets use the reply's primitives and semantic tokens

OpenUI widgets use the same inline markdown and maths engine as replies,
`CitationCode` / `CitationLink` for PageRef, and the shared Radix Slider.
`Frame` supplies a rounded card with a hairline border, a wrapping title and
counter, and a footer of small pill controls. Quiz choices are ordinary
keyboard-focusable toggle buttons. The explain charts draw SVG marks with semantic
chart, border and muted-text tokens; controls use primary, brand, success and
destructive tokens. Nothing inside the widget sets a fixed pixel width, so
the cards reflow with the chat pane and page zoom. The runtime and its state
contract are in [harness.md](./harness.md#openui-fences-draw-interactive-study-widgets).

`text-shimmer` sweeps a neutral band (foreground over muted text), never the
brand: indigo over body text reads as a link. Its `animation-play-state` is
`var(--shimmer-state, running)`, which `useVisibleAnimation` sets to `paused`
while the text is off screen, in a hidden tab or a hidden window
([harness.md](./harness.md#the-timeline-never-re-renders-per-token)).

Streaming OpenUI cards use `text-shimmer` and `block-shimmer` for unresolved
content. New sections fade in only while streaming; a committed-row remount
has no entrance animation. Both shimmer and entrance motion respect
`prefers-reduced-motion`, and placeholder sizes use CSS without measuring
visual viewport rectangles.

A ` ```widget ` fence runs in a sandboxed frame, so it can't use these
components; its kit (`app/widget-kit/kit.css`) redraws the same card, pills
and controls from the app's tokens, written into the frame for the current
theme. It never scrolls: the frame is its content's height, and taller content
is paginated with the kit's `Pages` ([widgets.md](./widgets.md)).

`Rich` repairs JSON's form-feed, tab, backspace and carriage-return escapes
before a letter, plus newline escapes inside dollar-delimited maths. It cannot
recover a lost backslash or distinguish an intended line break inside maths;
the agent must still double TeX backslashes in strings.

## Proposal cards show a decision before a write

Each proposal or job has its own OpenUI fence. `CardFrame`
(`app/src/components/markdown/openui/proposals/CardFrame.tsx`) draws every
proposal, job and result list the same way. The header is the kind's glyph
(muted, `kinds.ts`), a Manrope title and its subtitle chips, and at the right
the phase's chip (`CardChip.tsx`) beside any `meta` and `tools`. One inset
hairline separates the header from the body, and inset hairlines divide the
body's rows; there is no footer band. A footer appears only when there is a
decision (`actions`) or something to say beside it: the selection count, the
version choice, the follow-up note. Errors sit in a tinted callout inside the
card, never as a toast. Indigo means "act" or "in flight": the primary
button, a brand chip while work runs, and the card's top edge.
`cardState.ts` names the phase:

| Phase | Chip | Body and edge |
| --- | --- | --- |
| streaming | "Writing", shimmering, with the title | locked and dimmed; buttons in place, disabled |
| uncommitted | clock, "Waiting for the reply" | locked and dimmed |
| loading | spinner, "Loading" | locked and dimmed; the top edge sweeps |
| pending | none: the question and its buttons are the state | editable; a small single change is the compact one-row form |
| applying | brand spinner, "Creating…", "Applying…", "Undoing…", a job's "Starting…" | locked and dimmed, the top edge sweeps; a running job keeps its body live for its progress block instead |
| created | green "Done" (a job's "Complete"), Undo beside it | the results |
| failed | red "Not loaded", "Not saved" or "Failed" | solid red top edge; the callout says why: a row that never loaded offers Try again in it, a refused Apply stays editable, a job's error leads its body |
| undone | "Undone", title muted | results, still opening where the item remains |
| discarded, superseded | "Discarded" or "V1 · replaced" with the newer version's link | header only, dashed border on the page background |

The sweep is `progress-sweep` (`app/src/styles/utilities.css`): only the
background moves, as with `text-shimmer`, and reduced motion shows a still
tint. Before a fence's props resolve, the card is `CardSkeleton`: the kind's
glyph, "Drawing new tasks…" and two placeholder lines.

A started job's body (`JobProgress.tsx`) says what it is doing with its counts
and percent (tabular), a full-width bar (filled, or sweeping while the job
gives no percent), then its elapsed time, the Sync history link and Cancel;
once it ends, its result and how long it took. A sync's line is worded as the
Sync page words it. A job names what it acts on as chips: subjects as
`SubjectLabel` (the subject's icon and short code, as task cards and the
project page show them, never the Canvas code), files and lectures by name.

A card the agent gave a follow-up (`then`) names it on its deciding buttons
("Sync and continue", "Discard and continue"), and after the student decides
a muted `note` with a bent arrow, in the footer or under a
collapsed card's title, says what the follow-up did: continues when the job
finishes, continues after the current reply, continued below, or why it never
will (`proposals/followUp.ts`,
[harness.md](./harness.md#a-card-can-continue-the-agent)).

Locked bodies are `inert`, so nothing in a card that is not ready or is busy
takes a click. `useCardStreaming` reads OpenUI's `useIsStreaming`; outside a
`Renderer` it is false. Rows take the cheat sheet's grammar: a checkbox, the
title, then one line of `h-5` chips (date, estimate, subject), with a row's
Edit and Remove shown on hover or focus (`ROW_TOOLS`). A change reads as a
label column and `old → new`: the old value muted and struck, the new one in
the foreground, a date shift as a chip. Markdown changes keep complete before
and after documents in red and green panels so tables, code and maths render
correctly; source fields show a line diff. Markdown bodies, briefs, notes,
descriptions, prompts and answers edit in the shared Live-mode `NoteField`
with its formatting toolbar, maths tools, pictures and library mentions.
Drafts update on each keystroke so Apply reads the current text and cancelling
an editor cannot write its old text back on unmount. A deletion strikes its title
and says what goes with it. Five rows fit a page; paging retains edits and
inclusion ticks. Subtasks group beneath their existing parent's live title,
status and due date. A single small proposal starts as one line; its title
opens the editor. Project creation splits into Attributes and Tasks tabs:
Attributes holds the name field and the project page's property pills
(subject, due, start, tags, with tags named against the task tag registry and
created only on Create), then the calendar event and brief; a note proposal
uses the same subject pill. A result list longer than six rows ends in Show
more; each result leads with its kind (a task's live status glyph, which moves
the task as the table's does, a project's icon, a tag's colour). A proposed
task leads with the tasks table's status glyph, set to the column Create
would use (its own, its parent's, else the board's first) and editable there.
Proposal headings use `role="heading"` so the surrounding reply's markdown
heading styles cannot add spacing or override the card's title type, and the
card's text blocks are `div`s so the reply's paragraph margins don't reach
them.

`DateTimeField` preserves date-only values as local calendar days, without
inventing a clock time, and both sides of a change show the year only when it
isn't this one. Proposal date pickers offer All day; turning it off chooses
an explicit time. Status names and column choices come from the real board,
including custom done columns. A stale change starts excluded and has a
warning tooltip with its live values; explicitly ticking it uses those values
as the new guarded baseline. Reverting every field removes that row from the
changes to apply. A tag colour shows as its swatch and name, and a tag
deletion says how many tasks lose the tag.

Create, Apply, Discard and Undo show errors in the card. Confirmed results
link to the actual items and deletion results are struck through, with no
Open action. A valid amendment collapses its predecessor to its version
number and a newer-version link, and dots changed fields against the
student's saved card. Pending cards offer a V1/V2 selector once a second
version exists (before the buttons in a compact card, at the footer's left
otherwise), a Modified indicator and Reset; their snapshot and draft contract
is in [harness.md](./harness.md#proposal-context-is-read-at-dispatch-and-confirmation-lives-in-rust).
Decided cards keep their state through reloads and multiple views; their
database and send contracts are in
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
  and `weight` props. The canvas's tool rail is the one exception: its
  tools are coloured drawings in `app/src/components/canvas/toolArt/`
  ([drawing-canvas.md](./drawing-canvas.md#the-bars-are-docked-beside-the-drawing)).
- **A CLI agent's mark is in its vendor's colours** (`ProviderMark`, files in
  `app/src/assets/providers/`), the one exception to the single accent:
  Claude's clay fill, Antigravity's gradient arch, OpenAI's blossom in full
  `foreground` (never muted), and opencode's light or dark file switched on
  `.dark`. Files with mask or filter ids (Antigravity, opencode) are `<img>`s,
  since inline copies in every row would share those ids. Callers size it
  and never tint it; `mono` draws it in `currentColor` for a spot that must.
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
- **A new chat's headline** (`DraftHero`) takes Studio's hero type
  (`font-display`, 26px, semibold, as New video's question) with the subject
  as a pill inside the sentence. Whether the sentence fits on one line
  is measured from a hidden copy against the column's own box with a
  `ResizeObserver`, never against `window` or a viewport breakpoint, which page
  zoom and a split pane make wrong; when it does not, the pill drops to a
  line of its own.
- **Choosing from a list in a modal takes the ⌘K palette's frame**: a dialog
  hung from the top (`top-[14%]`, no padding, no close button), a 44px search
  field under a hairline, muted section headings over two-line rows with the
  highlight on `accent`, and a footer of `KeyHint` key caps
  (`app/src/components/search/KeyHint.tsx`). `NewThreadDialog` asks for a new
  thread's subject this way.
- **The composer's `@` and `/` menu hangs off its trigger character**
  (`ComposerMenu`): a `fixed` popover portalled to the body, measured over the
  range from the `@` or `/` to the caret (a collapsed range can measure zero
  in WebKit) and clamped to the root element's box, never `window` sizes. It
  opens below when the full menu fits there, otherwise on the roomier side;
  filtering to a loading or empty line keeps that placement. Its
  class names one position: `cn` keeps the last of two, so a `relative` after
  `fixed` puts the popup in the page's flow, under the window. Rows
  are a glyph, a name, a muted description and, for a skill, an Oculus or
  Personal pill badge. An `@` folder row leads with a disclosure arrow, a
  `span` since the row is itself a button; opened, its glyph is the open
  folder and what it holds sits under it, each level 14px further in, with
  file rows leaving the arrow's column blank so glyphs line up. The arrow
  swaps between two icons and never rotates, so nothing transitions. A search
  or empty state is one muted line inside it. A
  picked skill is an inline chip in the sentence (`SkillChip`, brand-tinted,
  `display: inline` so the line keeps its height). An attached file is a chip
  too (`AttachmentChip`), drawn as a mention is: its file glyph and the name
  it was attached by, cut in the middle past 40 characters, with its
  thumbnail in the hover preview for a picture. The shelf of thumbnails above
  the box asks before removing a picture the sentence names, in a muted line
  under it with Remove and Keep — never a dialog. A mention or attachment whose
  file is gone is muted, its glyph struck through, with "This context is no
  longer available." as its tooltip (`Unresolved`), in the box and in a sent
  bubble. The ⌘S stash is a "Stash"
  pill above the box opening a popover; its eviction notice is a line inside
  that popover, not a toast ([harness.md](./harness.md#the-empty-box-earlier-messages-and-the-stash)).
- **A compact composer rests as one pill** (the subject Overview's ask bar):
  the hint on one line, the agent's mark with its caret, the mic and send.
  Used, it opens into the full box in place, with no animation; sent, it
  morphs into the thread's composer
  ([harness.md](./harness.md#the-subject-overviews-ask-bar-starts-a-thread)).
- **The composer says a turn's state in words, not only colour**
  (`SendControls`): Stop is an icon until pressed, then a disabled "Stopping"
  pill with a brand spinner; a held queue with an empty box turns the send
  arrow into a filled "Resume thread" pill. While a turn runs the main button
  is the action a send takes, said before the press: the arrow for "Steer
  message", a list-plus for "Queue message", named in its tooltip, and
  flipped live while ⌘ is held. Waiting messages say "Held" or
  "Waiting" on an always-visible line above the bubble, with Move earlier,
  Move later and a "Send now" pill, so nothing there fades in on hover. A
  steer is the student's bubble inside the run, and a line right under it
  (not under its hover row) says it is sending, or warns in `text-warning`
  that it may not have arrived ([harness.md](./harness.md#steering-from-the-composer)). Notices about
  a held queue or an agent ignoring Stop are `BannerLane` cards, and a failed
  command is said in the card or on a line by the control, never in a toast
  ([harness.md](./harness.md#one-turn-at-a-time)). An agent's request is
  answered from a card of the same look just above the box (`RequestBar`:
  Decline, Approve, More), its refusal on a line inside it; the run then
  reads "Waiting for approval", steady, never shimmering. An agent's
  question opens the same card as a panel (`QuestionPanel`): one question at
  a time with its "1/2" count in `tabular-nums`, radio or checkbox rows with
  a key cap each (digits pick), the box below as the typed answer, and
  "Dismiss question without answering" as a muted text link beside the
  answer button; the run reads "Waiting for your answer"
  ([harness.md](./harness.md#in-the-chat)).
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
- **Settings, subjects, Chat, Drive, Tasks and Studio share one nav column**, `SideNav`: a 208px column
  on the window's `background` beside the page, a pinned header, rows with a
  16px icon and the active row filled, never an underline strip, and
  `SideNavSearch`, the pill search field under the header. Rows that are not
  links take its exported row classes. `SideNavShell` and `useSideNavPanel`
  own the responsive behavior ([shell.md](./shell.md#page-navigation-overlays-narrow-panes)).
  The subject, Chat, Drive, Tasks, Studio and Revision columns are drag-resizable and
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
  `SideNavSearch` takes a `className` for its wrapper where it shares a row
  (Chat's column puts the new-chat button beside it).
- **A multi-line list row reads as three levels** (T3's sidebar; Chat's
  thread rows, search results and recent list, `threads/rowType.ts`): a
  11.5px muted meta line, a 13.5px title (`font-medium text-foreground`
  when it wants attention, `text-foreground/80` when read), and a 12.5px
  preview in `text-muted-foreground/85`, so a read title stays a clear step
  above the preview in every theme. Rows are tight inside (6px top, 7px
  bottom, no line gap) and 2px apart; a Subject group's heading has 16px
  above and 4px below.
- **Subject tabs share `SubjectPage` for width and gutters** under
  `SubjectLayout`'s header row (the toggle, the subject crumb, the tab's
  name) and beside the subject's nav column, `SubjectLoading` for skeleton
  rows and `SubjectEmpty` for the empty view. Per-tab content and actions
  stay with the page. Files is the one tab that draws its own header row:
  Drive's browser, whose 48px header holds the toggle, the trail from Files,
  the folder search and New.
- **View preferences use `useStoredState`**, with readers that own defaults and validation; `useStoredSet` keeps collapsed group keys. Storage failures leave the live view usable.
- **No toasts, no bottom progress bars** — background jobs surface in the
  sidebar, as a spinner or dot on their rail item
  ([shell.md](./shell.md#the-sidebar-is-an-icon-rail)); browser downloads use
  their [toolbar list and Save dialog](./viewers.md#downloads-wait-for-a-destination-while-their-bytes-arrive).
  No placeholder UI, section-header icons or filler; stat cards only where
  the figures are the point (Home's Activity).
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
  for leaving the window, and for Drive's entries, whose drags cross from the
  browser to the tree in the nav column (`app/src/lib/drive/drop.ts`).
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
