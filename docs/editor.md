# Note editor

A student document is a markdown file edited in CodeMirror 6, in a Live
(rendered) or Raw mode. A document's row and file are in
[frontend.md](./frontend.md#uploads-and-documents-are-ordinary-library-files);
maths is [editor-maths.md](./editor-maths.md).

## Where

| Piece | Location |
| --- | --- |
| The view, title and mode; header controls | `app/src/components/documents/DocumentEditor.tsx`, `app/src/components/documents/DocumentControls.tsx` |
| Extensions: live preview, Raw mode, code, tables, widgets, commands | `app/src/components/documents/editor/` |
| One session per note | `app/src/lib/documentSessions.ts` |
| Find and replace | `app/src/components/documents/editor/find.ts`, `app/src/components/documents/editor/useEditorFind.ts` |
| A note's versions and its History panel | `app/src/lib/documentVersions.ts`, `app/src/components/documents/HistoryPanel.tsx` |

## The note editor is CodeMirror 6 over the file's exact text

It lives in `app/src/components/documents/editor/`; nothing is re-serialised.
`DocumentEditor.tsx` owns the view, title and mode and loads only for a
student document; its lightweight header controls live in
`app/src/components/documents/DocumentControls.tsx`. `FilePage` keys the
whole editor by row id: image attachments belong to one note, while a
rename keeps its editor. **Live** mode renders markdown in place —
headings, marks, links, lists and checkboxes, quotes, rules, code, pictures,
KaTeX maths and mermaid diagrams — and shows a construct's source while the selection touches
it (a heading, quote or list marker while the caret is on its line). **Raw**
is the same view with those decorations swapped out by a `Compartment`
for `rawMode.ts`: monospace source with line numbers, markdown and
frontmatter YAML coloured with the `--color-syntax-*` tokens.
Under the title, `DocumentMeta` shows subject, created (`first_seen_at`),
last updated (`modified_at` or the session's last save) and a word count.

- **Fenced code is parsed in its own language**
  (`app/src/components/documents/editor/codeLanguages.ts`): the info string's
  first word picks a `@codemirror/language-data` grammar, loaded lazily on
  first use; an untagged fence is guessed with highlight.js, which scores only
  the languages of a small subset whose pattern matches the sample. The guess
  is cached, and the fence left plain when no score is confident.
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

Editor commands, links, maths and completions share `ancestorAt` in
`app/src/components/documents/editor/syntax.ts`; each caller chooses its caret
bias and eligibility rules at a construct's boundary.

## Every editor of a note shares one session

Sessions (`app/src/lib/documentSessions.ts`) are keyed by row id, because
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

Find and replace in either mode is `find.ts`: a literal, case-insensitive
query, matches marked in the viewport, a step selects the next match, and
replacing goes through the note's history (Replace All is one undo step). A
match the find selected counts as touched in Live mode even while the find
bar holds focus, so its construct shows source (maths too, as TeX).

Its bar is `useEditorFind`, a find target like any other
([shell.md](./shell.md#f-reaches-one-registered-find)), used by
`DocumentEditor` (a row stuck under its `Toolbar`, rooted at the whole page,
title included). ⌘F seeds the
query from a one-line selection; ⌥⌘F, caught on the root since the menu
doesn't own it, also unfolds replace, which a read-only editor never shows.
Closing clears the highlights and focuses the editor with the last match
selected.

## A note's versions live in the database, never on disk

- **`document_versions` holds the whole text per row**
  (`app/src/lib/documentVersions.ts`), keyed by the file's row id so a rename
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
- **History docks inside the editor**
  (`app/src/components/documents/HistoryPanel.tsx`), not as a side panel
  item, because Replace current goes through that editor's view and the note
  stays beside the version. The header's Save version (⇧⌘S, focused pane
  only; ⌘S stays a flush) flushes first so the checkpoint matches the file.
  **Restore as copy** is the default: a new note "<title> (v3)" in a new tab.
  **Replace current** takes a `restore` snapshot first, then swaps the text
  as one `isolateHistory` transaction, so it saves like typing, reaches the
  other views and ⌘Z takes it back.

## Pictures, mentions and citations resolve to library files

- **A pasted, dropped or picked picture is written beside its note** in
  `documents/assets/`, on arrival, and linked relatively; `FileViewer` and the
  editor both resolve it with `libraryImageSrc` (`app/src/lib/libraryLinks.ts`).
  `is_document_rel` demands a `.md` one level down, so nothing can edit
  `assets/`.
- **`@` mentions a library file**
  (`app/src/components/documents/editor/mentions.ts`): at a word start,
  outside maths, code and frontmatter, the query after it (the chat's caps)
  searches every file, parsed or not, of the host's subject — or of the whole
  library when the host has none, each row then naming its subject — leaving
  out the note itself (`searchNoteLinkFiles`); an empty query lists recently
  opened files. It is a source in the one `autocompletion()`. Accepting writes
  the backticked library path the chat composer sends,
  `` `courses/<code>/files/week-3.pdf` `` (`mentionSyntax.ts`). The subject
  and note path reach the editor through `NoteHost`; an editor with no file
  passes a null path. `@` lists files only. A `[words](path)` link is the
  writer's own and stays a link; an older `[title](../<path>)` still opens
  through `libraryLinkTarget`.
- **A citation in inline code draws as the chat's chip**: in Live mode, an
  inline code span holding only a citation (`inlineCodeCitation`, the same
  `parseCitation` test chat applies) is replaced by `CitationWidget`
  (`widgets.ts`), which mounts `CitationCode` — and so `FileChip` — in a React
  root of its own, under a `TooltipProvider` for the picture lightbox. A
  click opens the file (⌘ in a new tab) without moving the caret; the source
  shows while the selection touches the span. Raw mode shows the text.

## AI suggestions are ghost text at the caret

They come from `app/src/components/documents/editor/aiSuggest.ts` and are off
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
are untouched. Blur, toggle-off and note switch call
`document_suggest_cancel`. A failure is a red dot on the toggle until a
request succeeds.

## Gotchas

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
