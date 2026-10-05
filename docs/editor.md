# Note editor

A student document is a markdown file edited in CodeMirror 6, in a Live
(rendered) or Raw mode. A document's row and file are in
[frontend.md](./frontend.md#uploads-and-documents-are-ordinary-library-files);
maths is [editor-maths.md](./editor-maths.md).

## Where

| Piece | Location |
| --- | --- |
| Load, save, title and mode; header controls | `app/src/components/documents/DocumentEditor.tsx`, `app/src/components/documents/DocumentControls.tsx` |
| Extensions: live preview, Raw mode, code, tables, widgets, commands | `app/src/components/documents/editor/` |

## The note editor is CodeMirror 6 over the file's exact text

It lives in `app/src/components/documents/editor/`; nothing is re-serialised.
`DocumentEditor.tsx` owns load, save, title and mode and loads only for a
student document; its lightweight header controls live in
`app/src/components/documents/DocumentControls.tsx`. `FilePage` keys the
whole editor by row id: pending saves and image attachments belong to one
note, while a rename keeps its editor. **Live** mode renders markdown in place —
headings, marks, links, lists and checkboxes, quotes, rules, code, pictures,
KaTeX maths and mermaid diagrams — and shows a construct's source while the selection touches
it (a heading, quote or list marker while the caret is on its line). **Raw**
is the same view with those decorations swapped out by a `Compartment`
for `rawMode.ts`: monospace source with line numbers, markdown and
frontmatter YAML coloured with the `--color-syntax-*` tokens.
Under the title, `DocumentMeta` shows subject, created (`first_seen_at`),
last updated (`modified_at` or this mount's last save) and a word count.
`write_document` writes a hidden
`.<name>.md.<pid>-<nanos>.tmp` sibling, fsyncs and renames it over the
note, so a reader never sees a truncated file; the `.tmp` suffix keeps a
leftover out of `list_documents`.

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

## Pictures and `@` links resolve to library files

- **A pasted, dropped or picked picture is written beside its note** in
  `documents/assets/`, on arrival, and linked relatively; `FileViewer` and the
  editor both resolve it with `libraryImageSrc` (`app/src/lib/libraryLinks.ts`).
  `is_document_rel` demands a `.md` one level down, so nothing can edit
  `assets/`.
- **`@` links a library file**
  (`app/src/components/documents/editor/mentions.ts`): at a word start,
  outside maths, code and frontmatter, the query after it (the chat's caps)
  searches every file of the note's subject, parsed or not, but the note
  (`searchNoteLinkFiles`); an empty query lists recently opened files. It is a
  source in the one `autocompletion()`. Accepting writes
  `[title](../<path>)`, each segment percent-encoded (`libraryLinkHref`),
  which `libraryLinkTarget` decodes, so ⌘-click and `FileViewer` resolve it
  alike. The subject and note path reach the editor through `NoteHost`.
  Lectures have no link form a note can open, so `@` lists files only.

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
