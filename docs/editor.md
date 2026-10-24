# Note editor

Files → Drive creates notes through New file → Note, alongside
[drawing canvases](./drawing-canvas.md). A student document is a markdown file
edited in CodeMirror 6, in a Live
(rendered) or Raw mode; the same editor is a form field for markdown kept in a
database row. A document's row and file are in
[frontend.md](./frontend.md#uploads-and-documents-are-ordinary-library-files);
maths is [editor-maths.md](./editor-maths.md).

## Where

| Piece | Location |
| --- | --- |
| The view, title and mode; header controls | `app/src/components/documents/DocumentEditor/`, `app/src/components/documents/DocumentControls.tsx` |
| Extensions: live preview, Raw mode, code, tables, widgets, commands | `app/src/components/documents/editor/` |
| One session per note | `app/src/lib/notes/documentSessions.ts` |
| Find and replace | `app/src/components/documents/editor/chrome/find.ts`, `app/src/components/documents/editor/chrome/useEditorFind.ts` |
| A note's versions, its History list and bar | `app/src/lib/notes/documentVersions.ts`, `app/src/components/documents/HistoryPanel/` |
| A version drawn on the page, and the word diff marking it | `app/src/components/documents/VersionView/`, `app/src/lib/notes/versionDiff/` |
| The note editor as a form field (task body, project About, a chat paste) | `app/src/components/documents/NoteField.tsx` |
| The Rust editor core, and its shadow of every note editor in dev builds ([editor-core.md](./editor-core.md)) | `app/editor-core/`, `app/src/components/documents/editor/shadow/` |

## The note editor is CodeMirror 6 over the file's exact text

It lives in `app/src/components/documents/editor/`; nothing is re-serialised.
`DocumentEditor/` owns the view, title and mode and loads only for a
student document; its lightweight header controls live in
`app/src/components/documents/DocumentControls.tsx`. `FilePage` keys the
whole editor by row id: image attachments belong to one note, while a
rename keeps its editor. The note and History's version share one column,
`NoteColumn.tsx`, fixed at the note's full width: a narrower pane scrolls it
sideways rather than re-wrapping the text. **Live** mode renders markdown in place —
headings, marks, links, lists and checkboxes, quotes, rules, code, pictures,
maths and mermaid diagrams — and shows a construct's source while the selection touches
it (a heading, quote or list marker while the caret is on its line); maths
is edited in a visual field instead
([editor-maths.md](./editor-maths.md)). **Raw**
is the same view with those decorations swapped out by a `Compartment`
for `chrome/rawMode.ts`: monospace source with line numbers, markdown and
frontmatter YAML coloured with the `--color-syntax-*` tokens.
Under the title, `DocumentMeta` shows subject, created (`first_seen_at`),
last updated (`modified_at` or the session's last save) and a word count.

- **Fenced code is parsed in its own language**
  (`app/src/components/documents/editor/syntax/codeLanguages.ts`): the info string's
  first word picks a `@codemirror/language-data` grammar, loaded lazily on
  first use; an untagged fence is guessed with highlight.js, which scores only
  the languages of a small subset whose pattern matches the sample. The guess
  is cached, and the fence left plain when no score is confident.
  Highlight classes stop at a nested grammar's edge, so block code's font is a
  `cm-code-text` mark in both modes. Live mode hides the fences behind a
  header row (the label, which reveals the fence to retag it, and Copy) until
  the selection touches the block. Colours are the `--color-syntax-*` tokens.
  A ```mermaid fence that owns its lines draws as a diagram (`MermaidWidget`
  in `live-preview/widgets/media.ts`, through `mermaidRender.ts`, shared with `Mermaid.tsx`);
  while the selection touches it, the source shows with the diagram under it,
  redrawn in place as it is typed and kept at its last good drawing while the
  source does not parse.
- **A pipe table is a grid whose cells edit in place**
  (`app/src/components/documents/editor/live-preview/table/`): its DOM and input handlers
  use `app/src/components/documents/editor/live-preview/tableModel.ts` for source-preserving
  edits. A cell writes back only its own source range, Tab/Enter move between
  cells, and the frame scrolls sideways. A double click on a word selects it
  natively; a triple click, or a double click off the text, selects the whole
  cell (`press.ts`), since the native selection would take its padding too.
  A cell holding markup draws it like the rest of the note — marks, code,
  links, file chips and `$…$` maths (`table/rich.ts`, parsed with the note
  grammar): the cell's own element stays the editor, invisible over the
  drawing, and focusing it shows the source until it blurs.
  A drag across cells (or Shift with a click or ↑/↓, or any arrow once a cell's
  text is all selected, which selects just that cell first) selects a block, which
  the scroller holds focus for: Delete empties it, and on a block already
  empty removes the whole rows (never the header) or columns it spans, or
  the table. ⌘C (or Edit → Copy, on the clipboard event, with `beforecopy` claimed since WebKit would leave Copy off with no text selected) copies it as tab-separated text and an HTML table, and ⌘V
  into a cell or a selected block fills cells from there with a pasted grid
  (an HTML table or tab-separated rows), growing the table when it reaches
  past it (`tableGrid.ts`, `pasteGrid` in `table/edits.ts`). A spreadsheet's
  or web page's table pasted into the note's text becomes a pipe table
  (`livePreview/gridPaste.ts`). Edge handles for the row and column
  under the pointer select them on a click and move them on a drag; a moved
  row keeps its bytes, a moved column rewrites rows with outer pipes, and the
  header row never moves (a body row may be short of cells).
  Rows are split from the line text, since lezer emits no node for an empty
  cell. In Live mode it never shows its source: its range is atomic, and
  `tableKeys` (`live-preview/livePreview/tables.ts`) hands the caret in — ↑/← from the line under
  it into the last row, ↓/→ from the line over it into the header, Backspace
  and Delete likewise (deleting an empty line in between when no text would
  end up against the table). Arrows at the grid's edges, and Esc, hand it back.
  CodeMirror's cursor there would be as tall as the table, so it is hidden and
  the table draws a short blinking one of its own at that edge
  (`tableCaret`, `livePreview/tables.ts`).
  Text typed, pasted or dropped at a table's edge goes on its own line, after a
  blank line below it, since GFM reads a line straight under a table as a row.
  Pasted text that ends in a table gets a blank line after it the same way
  (`livePreview/tableTail.ts`), wherever it lands. The blank lines beside a table (and a rule) draw as a
  small gap, not a full line, unless the caret is on one (`shrinkBlanksAround`
  in `livePreview/blocks.ts`). Lezer reads a line with a pipe in it directly
  above a table as part of its paragraph, so that blank line above is needed too.
  **Leading YAML frontmatter** parses as `Frontmatter`
  (`app/src/components/documents/editor/syntax/frontmatter.ts`) and draws as a
  Properties card, revealing its source when touched. Only a `---` first line
  opens it, and its closing fence is found by looking up to 64 KiB ahead,
  which Lezer's fragment reuse cannot see: `noteLanguage`'s parser runs its
  fragments through `frontmatterFragments`, which drops the one from 0 when a
  change in that window could make or unmake the block, as the core's
  `incremental.rs` restarts at 0.
- **The note parser trims Lezer's fragments by a line at each edit**
  (`lineSafeFragments`, `app/src/components/documents/editor/core/fragments.ts`,
  wired in `core/language.ts`): `@lezer/markdown` reuses a block on the
  edited line though its indentation changed (`    a` losing a space stays
  code), and keeps a paragraph whose end hung on the line after next (a table
  header over a just-broken delimiter row). Each fragment touching a change
  starts a line later and ends a line earlier, so CodeMirror's incremental
  tree equals a fresh parse, as the core's `incremental.rs` does.
- **Toolbar buttons and shortcuts are plain CodeMirror commands**
  (`app/src/components/documents/editor/commands/`) that rewrite markdown
  and unwrap when already applied, so they work in both modes. An inline mark
  (bold, italic, strike, code, link) comes off only the selected text: the
  span splits around it (`unwrapWithin` in `commands/inlineMarks.ts`, one table of
  mark kinds), widening to a nested mark's edge rather than crossing it; a
  caret or a full selection drops the whole span. Whitespace at the selection's
  ends is ignored throughout (`trimmed`): ` **a** ` selected is the span `a`. Applying a mark across spans
  of the same mark merges them (`wrapWithin`): `**ac*hi*e**` with `hi*e`
  selected italicises to `**ac*hie***`. Σ is the
  exception inside maths, where it opens or closes the maths toolbox
  (`toggleMathTools`,
  [editor-maths.md](./editor-maths.md#maths-has-a-toolbox-opened-on-demand)). ⌘-click opens a
  link — library files through `libraryLinkTarget` and `openFileSmart`, web
  URLs in the in-app browser; a plain click edits it. Maths is `$…$` (pandoc's
  spacing rule), `\(…\)` (empty included), and `$$` / `\[` blocks on their own lines
  (`app/src/components/documents/editor/math/mathSyntax.ts`).

Editor commands, links, maths and completions share `ancestorAt` in
`app/src/components/documents/editor/syntax/syntax.ts`; each caller chooses its caret
bias and eligibility rules at a construct's boundary.

## Every editor of a note shares one session

Sessions (`app/src/lib/notes/documentSessions.ts`) are keyed by row id, because
tabs stay mounted, a note can be open in a page and its side panel at once, and Fast
Refresh remounts editors. The session holds the text, the text on disk, the
600 ms debounce and the one write in flight; writes loop until disk matches,
and blur, ⌘S, the last editor leaving, `pagehide` and `beforeunload` flush.
Only the first editor reads the file; later ones (another pane, a remount)
take the session's text, so no read races a write. Each local change is
replayed into the note's other views with the `syncedEdit` annotation and
outside their undo history, so copies stay identical while selection, scroll
and undo stay per view. A
detaching view leaves its state (caret and undo history, as JSON) on the
session and the next view starts from it (`lease.restore`) while the text
still matches, so a remount never empties ⌘Z. A
session is dropped a tick after no editor holds it and nothing is unsaved;
a failed save keeps it, and its text, until the next edit retries. A
rename holds writes until the move lands. `write_document` writes a hidden
`.<name>.md.<pid>-<nanos>.tmp` sibling, fsyncs and renames it over the
note, so a reader never sees a truncated file; the `.tmp` suffix keeps a
leftover out of `list_documents`.

## Find and replace is the editor's own

Find and replace in either mode is `chrome/find.ts`: a literal, case-insensitive
query, matches marked in the viewport, a step selects the next match, and
replacing goes through the note's history (Replace All is one undo step). A
match the find selected counts as touched in Live mode even while the find
bar holds focus, so its construct shows source (maths too, as TeX).

Its bar is `useEditorFind`, a find target like any other
([shell.md](./shell.md#f-reaches-one-registered-find)), shared by
`DocumentEditor` (a row stuck under its `Toolbar`, rooted at the whole page,
title included) and `NoteField` (a floating card in the field). ⌘F seeds the
query from a one-line selection; ⌥⌘F, caught on the root since the menu
doesn't own it, also unfolds replace, which a read-only editor never shows.
Closing clears the highlights and focuses the editor with the last match
selected.

## A note's versions live in the database, never on disk

- **`document_versions` holds the whole text per row**
  (`app/src/lib/notes/documentVersions.ts`), keyed by the file's row id so a rename
  keeps its history, and outside `documents/` so listing, search and agents
  never see them. **Checkpoints** are the student's "Save version" (numbered
  v1, v2… per note, optional label) and never pruned. **Snapshots** are the
  app's: the session asks for one through `SessionIO.snapshot` when its first
  read lands, after a save once 10 min have passed since the last, and with
  the disk text when it ends — never awaited, so a failed snapshot cannot
  hold up or fail a save. A snapshot is skipped when its sha-256 matches the
  note's newest version, and `versionsToPrune` keeps every snapshot from the
  last 24 h, then the newest per UTC day to 30 days.
- **An `open` snapshot is `external` only when the app didn't write that
  text**: `saveDocument` records an FNV hash (`quickHash`) in localStorage as
  the write is *issued*, synchronously, because on quit the reply may never
  arrive. No record reads as the app's own text, never `external`. A note
  deleted or renamed outside the app gets a new row (or none) from
  `reconcileDocuments`, so its history goes with the old row.
- **A version takes the note's place on the page while History is open**
  (`app/src/components/documents/VersionView/`): a read-only CodeMirror view
  drawn by Live mode's render-only half, `livePreviewRender()` (`livePreview()`
  is that plus the editing pieces), so pictures, maths, code and tables look as
  they do in the note. It is read-only three ways — `readOnly`, `editable`
  false and a change filter that refuses what widgets dispatch themselves, such
  as a task's checkbox — and a table draws as a static grid when its view isn't
  editable. The note stays mounted under it, hidden and `inert`, never
  unmounted, so scroll and caret survive; ⌘F searches the version, never the
  hidden note.
- **Changes are marked against a base**, the next-older version or the note now
  (*Compared with* in the version bar). `app/src/lib/notes/versionDiff/` is a
  Myers diff over word, space, newline and punctuation tokens, cleaned up for
  reading; two unrelated texts blow its work budget, so the middle is diffed by
  line and each changed block refined by word while budget lasts. Marks are an
  inserted tint, deleted text struck through inline or as a block, a margin bar
  per changed line, and a construct drawn as a widget tinted whole
  (`outerDecorations` inline, a block wrapper for block constructs); hunks are
  what ↑/↓ steps through.
- **History is laid out like Google Docs' version history, at any width**:
  the list is a sidebar docked right (resizable, never more than half the
  width, `PANEL_MAX_SHARE` in `HistoryPanel/constants.ts`), hidden or shown
  from its header or the bar's *Versions* toggle without leaving History, and
  remembered. The version sits in the note's column (`NoteColumn`), so it
  scrolls sideways under a narrow pane while the bar above stays put. The bar is
  there in every state — loading, empty and failed reads get one with only
  Back and *Versions* — and its Back arrow (or Esc) leaves History. The bar
  stays two rows: its title truncates and its action labels drop to icons as
  its own width shrinks (container queries). The list groups by day with
  size deltas and takes arrow keys. The version bar
  (`app/src/components/documents/HistoryPanel/VersionBar/`) holds the actions,
  change navigation and *Show changes*. The header's Save version (⇧⌘S, focused
  pane only; ⌘S stays a flush) flushes first so the checkpoint matches the file.
  **Restore as copy** is the default: a new note "<title> (v3)" in a new tab.
  **Replace current** takes a `restore` snapshot first, then swaps the text
  as one `isolateHistory` transaction through the note's view, so it saves like
  typing, reaches the other views and ⌘Z takes it back.

## Pictures, mentions and citations resolve to library files

- **A pasted, dropped or picked picture is written beside its note** in
  `documents/assets/`, on arrival, and linked relatively; `FileViewer` and the
  editor both resolve it with `libraryImageSrc` (`app/src/lib/files/libraryLinks.ts`).
  `is_document_rel` demands a `.md` one level down, so nothing can edit
  `assets/`.
- **`@` mentions a library file**
  (`app/src/components/documents/editor/chrome/mentions.ts`): at a word start,
  outside maths, code and frontmatter, the query after it (the chat's caps)
  searches every file, parsed or not, of the host's subject — or of the whole
  library when the host has none, each row then naming its subject — leaving
  out the note itself (`searchNoteLinkFiles`); an empty query lists recently
  opened files. A row's glyph and colour, and the type in its detail
  (`PDF`, `Discussion`, `File`…), are `fileReferenceAppearance`'s, the same as
  a task's related-file chip. It is a source in the one `autocompletion()`. Accepting writes
  the backticked library path the chat composer sends,
  `` `courses/<code>/files/week-3.pdf` `` (`syntax/mentionSyntax.ts`). The subject
  and note path reach the editor through `NoteHost`; an editor with no file
  passes a null path. `@` lists files only. A `[words](path)` link is the
  writer's own and stays a link; an older `[title](../<path>)` still opens
  through `libraryLinkTarget`.
- **A citation in inline code draws as the chat's chip**: in Live mode, an
  inline code span holding only a citation (`inlineCodeCitation`, the same
  `parseCitation` test chat applies) is replaced by `CitationWidget`
  (`live-preview/widgets/citation.ts`), which mounts `CitationCode` — and so `FileChip` — in a React
  root of its own, under a `TooltipProvider` for the picture lightbox. A
  click opens the file (⌘ in a new tab) without moving the caret; the source
  shows while the selection touches the span. Raw mode shows the text.

## AI suggestions are ghost text at the caret

They come from `app/src/components/documents/editor/chrome/aiSuggest.ts` and are off
by default
(`document_suggestions_enabled`; the topbar's `SuggestToggle`, shared by
every note through `documentPrefsStore`). 500 ms after an edit, with one
empty caret not mid-word, outside code and frontmatter and with no
completion open, up to 4000 characters before the caret and 1500 after go to
`document_suggest` on the `documentSuggestions` job's model. An answer
lands only if it is the newest request and the text and caret are
unchanged. Tab takes it (one undo step), Mod-→ the next word, Esc drops it;
typing its front eats it, anything else drops it. Its keymap is
`Prec.highest` but passes without a ghost, so the note's and snippets' Tab
are untouched. In Live mode, complete inline and display maths in the ghost
renders with the note's maths engine (`chrome/ghostMath.ts`); code, escaped
delimiters and invalid or incomplete maths stay as source. Raw mode shows
the original text. Acceptance always inserts the original markdown and
LaTeX. A preview redraws when the maths engine becomes ready.
Blur, toggle-off and note switch call
`document_suggest_cancel`. A failure is a red dot on the toggle until a
request succeeds.

## `NoteField` is the note editor as a form field

`app/src/components/documents/NoteField.tsx` serves markdown kept in a
database row — the task body ([projects.md](./projects.md)) — or a chat
composer's pasted text, kept with its draft. Live mode
only, with no session, title, Raw mode or AI suggestions; the caller
supplies the `@` subject, picture resolution and picture writes. At rest it
reads as prose; focus anywhere in its React tree — including the
`Toolbar`'s portaled Select and table popover, whose focus events bubble
through React — shows the brand border and the toolbar above the text. A
press that gives a field at rest its focus holds the toolbar back until the
pointer is released, since mounting it pushes the text down and would move the
line being clicked or dragged over. A blur is judged a tick later, and a window
losing focus (the picture picker) is not leaving. The field's text scroller
clips and masks its contents, so `NoteField` passes `noteExtensions` a
`tooltipParent`: the completion list and maths popover are mounted in
`document.body` (inside the dialog when the field is in one, which would
otherwise shut on a click outside it) instead of the editor. `ViewSlot`
(`editor/core/viewSlot.ts`) keeps the host to one `EditorView`: it destroys any
view still held before mounting the next and destroys even when the effect's
own cleanup threw first. Leaving, ⌘↵ and unmounting hand an edited doc to
`onCommit`; a new `text` replaces an unedited doc, outside history.
`noteExtensions` takes its placeholder and aria label. An optional `onChange`
hears the doc on every change, for a caller whose buttons read the text and
cannot wait on the blur (the chat's pasted text editor,
[harness.md](./harness.md#pasted-text-is-a-card-sent-inline));
`scrollClassName` replaces the text scroller's 420px bound.

`NoteField` mounts its CodeMirror view in a layout effect, before paint, so
swapping prose for the field cannot expose an empty editor mount point.
The shared `Toolbar` stays on one line at every pane width: its controls keep
their sizes inside a horizontal scroller with `useScrollFade` edge masks.
The scroller's scrollbar is hidden so overflow cannot change the bar's height;
menus and tooltips remain portaled outside its mask.

## Gotchas

- **Pass editor views to the toolbar through `getView`.** React's dev profiler
  recursively inspects object props; `EditorView.observer.win` reaches sandboxed
  frames and throws a cross-origin access error. Resolve the view only in handlers.

- **Tooltip rules cannot assume the editor is their ancestor.** With
  `tooltips({ parent })` CodeMirror mounts them in a container that carries
  the theme class but not `cm-editor`, so theme rules hang from `tooltipRoot`
  (`theme/tokens.ts`), never `&.cm-editor`, and state that the editor holds
  reaches the tooltip as a class on the tooltip itself: the maths popover
  hides in command mode through `followCommandMode`
  (`math/tools/mathTools/dom.ts`), not a `cm-math-command` ancestor rule.

- **The selection highlight is our own layer** (`core/selectionLayer.ts`):
  `drawSelection` draws the caret, and its highlight layer is hidden by the
  theme, since it has no option to skip spans; `noteSelection` draws the
  text rectangles and the shared text-and-inline-maths geometry
  (`mathSelection/lines.ts`, through `core/selectionMath.ts`). Display maths
  fills the `selectionGaps` with structural bands
  ([editor-maths.md](./editor-maths.md#rendered-maths-is-one-unit-to-the-selection-the-clipboard-and-the-caret)).
- **CodeMirror block decorations come from a `StateField`** — it throws for
  block widgets from a view plugin
  (`app/src/components/documents/editor/live-preview/livePreview/`). Reveal-on-caret
  decorations rebuild on selection and focus changes, not only edits.
- **An editable widget must patch itself in `updateDOM`** — every edit rebuilds
  the field, and fresh widget DOM drops the caret in a focused table cell
  (`app/src/components/documents/editor/live-preview/table/`).
- **`@codemirror/view` is patched** (`app/patches/`) — its scroller search took
  any box whose `scrollHeight` beat `clientHeight`, and under page zoom the
  editor's `overflow: visible` wrappers round a pixel over, so drag-select never
  scrolled `page-scroll`. The sticky toolbar's height is a `scrollMargins` top.
- **Tooltips are never clipped to the editor** (`core/unclippedTooltips.ts`):
  CodeMirror adds the toolbar's `scrollMargins` top to the editor's own top
  when it decides a tooltip's anchor is outside the editor, and parks the
  tooltip at -10000px, so the `@` and maths lists on the first lines never
  showed. The plugin sets `clip: false` on tooltips while a top margin exists.
