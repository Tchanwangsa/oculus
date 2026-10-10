# Maths in the note editor

In Live mode a note's maths renders, edits in a MathLive visual field, and has
a toolbox, quick picks and typed shorthands; in TeX and Raw mode it is typed as
LaTeX. The delimiters and the rest of the editor are [editor.md](./editor.md).

## Where

| Piece | Location |
| --- | --- |
| Delimiters, the maths under the caret | `app/src/components/documents/editor/math/mathSyntax.ts`, `app/src/components/documents/editor/math/mathContext.ts` |
| The visual field and its writes | `app/src/components/documents/editor/math/field/mathField/`, `app/src/components/documents/editor/math/field/mathFieldEdits.ts` |
| Rendered maths, atomic ranges, edge keys | `app/src/components/documents/editor/live-preview/widgets/`, `app/src/components/documents/editor/live-preview/livePreview/` |
| Toolbox, palette, quick picks, usage | `app/src/components/documents/editor/math/tools/mathTools/`, `app/src/components/documents/editor/math/tools/mathPalette.ts`, `app/src/components/documents/editor/math/tools/mathUsage.ts` |
| Shorthands | `app/src/components/documents/editor/math/tools/shorthand/` |
| Typing matrices | `app/src/components/documents/editor/math/field/mathMatrix.ts`, `app/src/components/documents/editor/math/field/mathMatrixField.ts` |

## Live mode edits maths visually

In `app/src/components/documents/editor/math/field/mathField/`, while the caret is in
maths, a MathLive `<math-field>` stands in for the rendering — inline for
`$…$`; for a `$$` block a shaded field the column's width with the maths
centred, its rows too — with slots for a fraction's parts or a
sum's limits, and `\` commands that become one symbol (MathLive's own
command list, restyled in `index.css`). MathLive is a lazy chunk imported as
a Live editor mounts; it reuses the KaTeX fonts in `app/src/styles/katex/`
and has sounds and the virtual keyboard off. Once it has loaded, MathLive also draws the maths the
field could open (`staticMath`: its static markup, with `mathlive/static.css`
injected minus its `@font-face` rules), in the box the field takes
(`.cm-math-ml` in `theme/math.ts`: the field's size, line height and padding,
and `\text{}` in KaTeX_Main without kerning or ligatures, as the field
sets it a span per letter), so entering maths moves neither it nor its line; while MathLive loads, if it
fails, and for maths the field can't take, the maths engine draws it in
KaTeX's markup ([viewers.md](./viewers.md#one-markdown-renderer-serves-every-surface)).
Until that engine has loaded, the rendering is the maths' source in muted
text (`cm-math-pending`), and `mathsWatcher` redraws it once it is ready.

A field mounts
over a static copy of its maths, unseen, and replaces it once focus has
rendered it (`FieldController.mount`): MathLive otherwise draws a field a
frame after it connects, and the note, briefly shorter, would clamp a page
scrolled to its end.

Each edit rewrites only the LaTeX between the
delimiters (MathLive's serialisation, placeholders dropped, a block's
environment one row per line); entering writes nothing. Undo is the note's:
the field's writes are typing transactions (`math/field/mathFieldEdits.ts`), so a run
of keystrokes is one undo step, and ⌘Z / ⌘⇧Z in the field (MathLive's own
undo is unbound) step the note's history and reload the field from the
note, the caret after the atoms that changed (`caretAfterChange`), or close
it when the step moves the caret out of that maths. A field
writes only to the maths it opened on, and only while that LaTeX is still
what it last wrote or loaded; a flush with no edit since writes nothing, so
⌘Z's own flush never lands a re-tidied copy on top of the step it undoes.

The note's selection stays inside the maths
while the field is open; a selection extended from there to past the maths
(Shift-click) starts at the maths' edge, so it holds exactly what is
highlighted. Click puts the
field's caret where the rendering was pressed, and clicks in the field
are hit-tested by `caretAt`, not MathLive (whose hit-test sends a click on
`\cos^2`, or between atoms, to the front): the innermost slot under the
click — a script, a numerator, a cell — takes the caret at its nearest
gap. A selection in the field whose ends sit at different depths — dragged
from beside a matrix into a cell, or across two cells — widens to take each
structure it reaches into whole (`wholeStructures`; MathLive's offsets put
a matrix's cells before the matrix, so it would highlight the cells without
the matrix), while a block's rows select as one run; ←/→, Backspace/Delete at
inline maths and ↑/↓ from the line beside a block enter it at that end.

Esc (once any toolbox is closed), an arrow past the field's edge and Enter
in inline maths leave it;
Enter or Shift+Enter in display maths adds a row (a block's own `\\` lines
stay bare in the note; the field holds them in MathLive's `\displaylines`,
which KaTeX lacks, since MathLive rejects a bare top-level `\\`, and centres
its rows with a stylesheet adopted into the field's shadow root), never a
second empty one: on an empty row Enter does nothing (`onEmptyLine`); Tab
goes to the
next empty slot, else types `\qquad`; ⌘Backspace deletes the caret's line
up to the caret (a block's row, or a cell of an environment's rows, through
the structure the caret is in); Backspace in an empty field removes the
maths, and in an empty script (`\cos^{}`) drops it with the caret just
after its atom (`dropEmptyScript` — MathLive's own caret there lands at -2,
which it counts from the field's end). Committing a bare `\text` (or `\textbf`, `\textit`, `\textrm`,
`\mbox`) from the `\` list, or the toolbox's text cells, switches the
field to MathLive's text mode at the caret (`startText`) — MathLive alone
drops the empty argument — and → or Tab at the end of the text goes back
to maths.

Focus in the field counts as the editor's (`core/liveFocus.ts`). Maths
that MathLive or the maths engine can't read cleanly (`readsCleanly`, false
until both have loaded), a multi-line block inside a quote
or list, and maths switched with the toolbox's TeX control (⌘⇧M) are typed
as LaTeX source (TeX mode, as in Raw mode) until the caret leaves them.

## Rendered maths is one unit to the selection, the clipboard and the caret

The rendering is `MathWidget` in
`app/src/components/documents/editor/live-preview/widgets/math.ts`, with the atomic ranges and
edge keys in `live-preview/livePreview/`. Once MathLive has loaded, every rendering's range is atomic, so a drag or Shift+arrow covers
maths whole and Shift-click extends the selection over it; inline maths
highlights like a word, and a block a selection covers fills as one
(`cm-math-selected`) — native selection never paints the rendering, and a
selection taking a whole block keeps it rendered rather than opening the
field. Copy and cut always write the note's source (`$$…$$` lines), since
a rendering passes clipboard events to CodeMirror. A rendered block takes
its column's width (`contain: inline-size` on `.cm-math-display`, as on the
field's block) and scrolls a too-wide formula, rather than widening the
note.

A block has a caret
spot before and after it, drawn one line tall beside its first or last
row: a press in its padding above or below the formula (or above or below
the shaded field) rests the caret there; a press anywhere across the
formula's rows opens the field in the row under the pointer — the row
whose vertical band holds it, else the nearest — at that row's nearest
gap, so a press far to the right of a row ends that row (`caretAt`). From
the spot before a block →/↓/Delete enter the field at its start, from the
spot after it ←/↑/Backspace at its end; Shift+arrow selects the block;
Enter adds a line there, typed or pasted text goes on its own line, and
Backspace before a block (Delete after it) removes an empty line beyond or
steps onto a line with text rather than joining it to a `$$`. From the line
beside a block, ← at its start or → at its end (and Backspace/Delete from a
line with text) stop at those spots first, so two blocks that touch still
take a line between them; ↑/↓ go straight into the field (`enterBlock` in
`math/field/mathField/keys.ts`). The browser has no text position at either spot and types
at the nearest one it has, the next line or past a touching block, so input
there goes to the editor's caret instead (`typedAt` in `livePreview/edges.ts`).

Pasted into
the field, maths goes in at the caret and markdown with prose around maths
lands in the note just after that maths; LaTeX copied from a field (no
`\displaylines` wrapper) pastes outside maths in the shape it was copied
from: a block's copy carries its `$$` lines (`BLOCK_MATH_TYPE`) and goes in
on lines of its own (inline in a table row), inline maths as `$…$`. A
chip under the pasted maths (`PastedView`, `offerShapeSwitch`) offers
"Convert to block" or "Convert to inline" (`shapeChange`, the toolbox's own
switch) and a close button; a switch keeps it up for the way back, and it
goes 5 s after its last use with the pointer off it, on Esc, or with any
other edit.

Both of MathLive's
drawings and the KaTeX fallback lay out alike: array and matrix rows
take `MATH_ARRAYSTRETCH` and a block's top-level `\\` lines
`MATH_LINE_GAP` (KaTeX's `.katex-newline` in `theme/math.ts`, MathLive's root
`lines` table); MathLive's `array` is centred on the axis as KaTeX draws it
(it hangs from its first row there); and an `array` that is all a
`\left…\right` holds drops its outer column padding in both, so
`\left[\begin{array}…\right]` hugs its brackets like `bmatrix`. MathLive's
fixes patch its internal array atom (`patchArrays`), which its static
markup goes through too; KaTeX's are render-time only, and the note's text
never changes for display.

## Maths has a toolbox, opened on demand

The toolbox is `app/src/components/documents/editor/math/tools/mathTools/`. Nothing
shows while
the caret is in maths (`mathAt` in `math/mathContext.ts`) until it is asked
for: Σ in the toolbar or ⌘⇧Space (Mod-Shift-Space; Ctrl-Space is
completion, and macOS reserves Ctrl- and ⌘-Space) opens or closes it, and
it stays open until Esc, its close button, or the caret leaving that maths
(`mathToolsField` holds what is open and on which maths, mapped through
edits). It is a CodeMirror tooltip centred under the maths (a block on the
text column): a live preview (the parse error in red; empty until the maths
engine has loaded), a Recent row
and a tab strip, each one row that scrolls sideways, and a palette three
rows tall that scrolls, with a matrix-size grid (`math/tools/mathPalette.ts`). Beside
the tabs, a switch rewrites the maths as inline `$…$` or a block on lines
of its own (`toggleShape`: the text around inline maths splits onto lines
in the same container, and a block turned inline rejoins the paragraph
lines right above and below it), and a close button does what Esc does.

The first tab, Popular and selected by default, holds the most used
entries and pads with `POPULAR_DEFAULTS` while history is thin; it is
rebuilt only when the toolbox opens or a tab is picked, so cells never
move under the pointer; a palette click inserts and leaves it open.
Palette clicks, quick picks, accepted completions and `\commands` typed
out — in TeX (a non-letter typed after the name) or in the visual field
(MathLive committing its command mode) — all count, a typed command as its
palette entry (`math/tools/mathUsage.ts`, `localStorage`); the Recent row and Popular
are across all notes, while each subject keeps its own recents for the
quick picks.

In TeX and Raw mode maths
is typed as LaTeX (Space is a space), and the toolbox's cells insert
`snippet()`s whose `{}` slots are Tab fields; the snippet keymap is
`Prec.highest`, above the note's Tab. Inside maths, `\` plus a letter
opens completion with rendered previews, whatever the toolbox is doing; it is
the editor's one `autocompletion()` (`core/extensions.ts`), so other sources
join its `override`. The caret between a lone `$$` pair (what a typed `$`
and Σ insert) counts as empty inline maths (`emptyPair` in `math/mathContext.ts`):
mid-line `$$` never parses, and alone on a line it parses as an unclosed
block opener, which `mathAt` reads as no maths.

## Typing `$` opens maths

In Live mode a typed `$` writes `$$` with the caret between, which is an
empty inline field, ready to type in (`dollarTyping` in `livePreview/edges.ts`,
`Prec.high` so the shorthand handler doesn't type into the pair first).
At a line's start (container markup aside, or on the line it opens beside a
block) it writes `\(\)` instead: `$$` there opens a display block in every
Markdown reader, which would run to the next `$$` and take the text between.
The parser takes an empty `\(\)` as inline maths (`parenMath`), and the
field's first write turns it into `$…$` (`FieldController.flush`).
A second `$` there — in the empty field (`fieldKey`) or in the note's source
before MathLive is ready — turns the pair into an empty block with the caret
on its line (`emptyPairToBlock`, which is `toggleShape`). A `$` stays a
plain character after `\` or `$`, in code, before a word (`$5`) and with a
selection; Raw mode never pairs. Inline maths left empty (`$$`, `$ $`,
`\(\)`) is deleted, outside the history, once the field closes with the
caret outside it (`dropEmptyInline` in `math/field/mathField/index.ts`); a caret still inside
(TeX mode, the window losing focus) keeps it.

## In the visual field, Space opens quick picks at the caret

The strip is `QuickPicksView` and `fieldKey` in `mathTools/` (`quick-picks.ts`,
`keys.ts`), reached through `fieldKeys` ahead of the field's own keys in
`mathField/controller/keyboard.ts`. MathLive
ignores Space in maths, so there it opens a strip hanging from the field's
caret like a completion list, flipped above near the window's bottom and
hung leftward from the caret when it would pass the editor's clipped
right edge (`clipRight`): the five entries last used in the note's
subject, most recent first, padded from Popular while the subject has
little history (`quickPicks`), numbered, and a ⌄ button.
1–5 or a click inserts one and closes the strip; Space again or ⌄ opens
the full toolbox; Esc closes the strip and a second Esc leaves the field;
any other key, or a caret move in the field, closes it and goes on as
usual. Space inside `\text{}`, beside a text atom and in a `\command`
being typed keeps MathLive's meaning, and after a term in a matrix or
bracket group it starts a cell (below). The full toolbox under the field
drops the preview, its cells insert into the field (slots become MathLive
placeholders) and its TeX control switches to TeX mode; there a Visual
control switches back. Neither toolbox ever takes focus. The field says so
on any empty line (`syncHint` in `mathField/controller/hint.ts`): an empty inline field
shows "Space (␣) for math tools" in flow after it, inside its tint; an
empty block, or one empty row of its lines, centres "Start typing or Space
(␣) for math tools" on that line with the caret drawn just before it (the
hint's `::before`; MathLive's own, centred under the hint, is hidden). It goes once the
line has anything in it.

A block's field writes its LaTeX with no blank line at either end and never
two in a row (`squeezeBlankLines`, `math/field/mathFieldEdits.ts`), and the break after
the opening `$$` and before the closing one is a single newline: an empty
last row is kept as a trailing `\\`, not a blank line, while the field is
on the block; once it leaves, empty rows at the block's end go
(`dropEndRows`, `withoutEndRows`), so the rendering has no blank row under
the formula for the caret beside it to rest on. Blank lines already in a
block go when the field opens on it (`dropBlankLines`). Both cleanups stay
outside the history. The empty-line hint (12px, one tight line) sits at the height of the empty
row's leading atom, clamped inside the field's box, two frames after the edit,
since MathLive draws in a frame of its own and its hidden caret can lag.

## Matrices are typed as in MATLAB

In the visual field `[a b; c d]` types a matrix
(`math/field/mathMatrix.ts`, pure; `mathMatrixField.ts` reads MathLive's atoms and
writes back). The caret's grid is the structure it is directly in: a cell of
a `matrix`, `pmatrix`, `bmatrix`, `Bmatrix`, `vmatrix`, `Vmatrix` or
`smallmatrix` (not `array`, `cases`, `aligned`), or a bracket group whose
delimiters match one (`(`, `[`, `\{`, `|`, `\|`; the right one typed or
still MathLive's ghost), which is one cell and becomes that matrix at its
first new cell or row. Maths mode only; `FieldController.key` runs these
keys after the toolbox's, and `spaceFree` keeps the quick picks off a Space
they take.

- **Space** after a term ends the cell: into the next cell when the caret is
  at the cell's end and that cell is empty, else into a new column (what
  followed the caret moves into it). In an empty cell, at a cell's start and
  after an operator, relation, punctuation, opening or `\sin`-like operator
  it opens the quick picks as anywhere else. A cell holding only a binary
  operator or relation rejoins the cell before it on Space (MATLAB's
  `[a + b]`): its column goes when nothing else is in it, else the row's
  later cells shift left. So `(a + b)` typed with spaces stays plain
  brackets, while `[1 -1]` is two cells.
- **`;`** goes to the start of the next row: a new empty row, unless the
  next row is already empty. In a bracket group the `;` is typed first and
  written as its own undo step, so ⌘Z after the matrix appears gives back
  `f(x;`; inside a matrix it is never typed.
- **Backspace** at the start of an empty cell, in a grid of more than one,
  removes its column when all empty, else its row when all empty, else steps
  back to the end of the cell before. One cell left turns back into its
  bracket group, ghost and all, so `)` still closes it.
- **The closing bracket** (`)`, `]`, `}`, `|`) typed in a matrix's cell drops
  trailing empty rows and columns and puts the caret after the matrix; one
  cell left becomes a closed bracket group.

Rows are padded to the widest before any edit, so `[a b; c d e f g]` comes
out with empty cells after `b`; a row stops at MathLive's ten columns. New
cells are placeholders, which Tab reaches and the note never stores. Each
edit replaces the whole structure (its scripts kept, `^T`) and is one undo
step: pending keystrokes are written first, then the edit with
`isolateHistory` (`flush(true)`).

## Typed shorthands expand inside maths

The rule tables are at the top of
`app/src/components/documents/editor/math/tools/shorthand/rules.ts`: `a/` → `\frac{a}{}`, `sr` → `^2`, `@a` → `\alpha`, `->` → `\to`,
`sin ` → `\sin `, `\left…\right` around a closed group holding a tall
construct. Never inside `\text{}`-like arguments. The typed character lands
first and the rewrite is its own history event, so ⌘Z gives back what was
typed. The visual field gets the Greek, power and operator rules as MathLive
inline shortcuts, over MathLive's defaults minus the ones that turn letter
runs into units or words (`PRUNED` in `mathField/shortcuts.ts`).
