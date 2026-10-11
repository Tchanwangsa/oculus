# Maths in the note editor

In Live mode a note's maths renders, edits in a visual field, and has a
toolbox, a list of picks and typed shorthands; in TeX and Raw mode it is typed as
LaTeX. The delimiters and the rest of the editor are [editor.md](./editor.md).
One Rust engine (`app/math-core`, the katex fork and the field's edit model,
compiled to WebAssembly) draws and edits it all; its layout is
[math-core's README](../app/math-core/README.md).

## Where

| Piece | Location |
| --- | --- |
| Delimiters, the maths under the caret | `app/src/components/documents/editor/math/mathSyntax.ts`, `app/src/components/documents/editor/math/mathContext.ts` |
| The field's state in the note, its writes, its widget, how it scrolls the note | `app/src/components/documents/editor/math/field/mathField/`, `app/src/components/documents/editor/math/field/mathFieldEdits.ts`, `app/src/components/documents/editor/math/field/fieldNote.ts`, `app/src/components/documents/editor/math/field/noteScroll.ts` |
| The field itself, its host in the note | `app/src/components/documents/editor/math/field/mathView/`, `app/src/components/documents/editor/math/field/rustField/` |
| The engine's facade, the geometry the field measures with | `app/src/lib/maths/` |
| Rendered maths, its atomic ranges | `app/src/components/documents/editor/live-preview/widgets/`, `app/src/components/documents/editor/live-preview/livePreview/` |
| Toolbox, palette, usage | `app/src/components/documents/editor/math/tools/mathTools/`, `app/src/components/documents/editor/math/tools/mathPalette.ts`, `app/src/components/documents/editor/math/tools/mathUsage.ts` |
| Shorthands | `app/src/components/documents/editor/math/tools/shorthand/`, `app/math-core/edit/src/shortcut/` |
| Typing matrices | `app/math-core/edit/src/command/grid/` |

## Live mode edits maths visually

While the caret is in maths, a visual field stands in for the rendering —
inline for `$…$`; for a `$$` block a shaded field the column's width with the
maths centred, its rows too — with slots for a fraction's parts or a sum's
limits, and `\` commands that become one symbol. `MathFieldWidget`
(`math/field/mathField/widget.ts`) opens it as a `RustFieldController`
(`math/field/rustField/`), which hosts a `MathView` (`math/field/mathView/`).
The widget, the toolbox (`fieldKeys`, `fieldTools`, palette inserts) and the
note's undo routing know it only as a `VisualField`
(`mathField/registry.ts`): mode (`math`, `text`, or `command` while a
`\command` is typed), a selection-change subscription, `isEmpty`,
`spaceFree`, `caretRect`, `insertTemplate`, `flush`, `leave`, `sync`, and the
field's DOM, view and maths. Leaving, removing the maths, pasting prose
beside it and a block's blank lines on opening are shared (`fieldNote.ts`).

The field opens only once the maths engine (`app/src/lib/maths`) has loaded,
and only on maths its edit model opens on (it parses, and the engine doesn't
trap: `readsCleanly`). Until the engine has loaded, the rendering is the
maths' source in muted text (`cm-math-pending`), and `mathsWatcher` redraws it
once it is ready. When it has, the rendering is the engine's KaTeX markup
([viewers.md](./viewers.md#one-markdown-renderer-serves-every-surface)), which
the field draws too, so entering maths moves neither it nor its line.

`MathView` is the field over the edit model (`MathField` in
`app/src/lib/maths/field.ts`). It draws the maths with the source map on
(`renderToString` with `sourceMap: true`, as the Live rendering draws it:
`math/hugArrays.ts`, `MATH_ARRAYSTRETCH`), and over it a caret, selection
bands over the selected atoms (a band per line, an array's rows each their
own, `bands` in `lib/maths/geometry`) and, while a `\command` is pending, the list of its options
(below) — all positioned from `lib/maths/geometry`. Keys,
typing and IME input arrive on a focused, invisible textarea at the caret
(`cm-math-view-input`, which `core/liveFocus.ts` counts as the editor's
focus), so the OS candidate window opens there; each runs one model command,
and the view redraws and hands the step to its host, never editing the source
itself. A caret stop sits beside the atom it touches, or at an empty slot's
`□`, and is drawn one em of that element's font tall, 0.75em above its
baseline and 0.25em below, so in a script or a root's index it is the
script's size. The baseline is the element's top plus its font face's
ascent, measured once per face and size (`lib/maths/geometry/font.ts`): an
inline box is as tall as its face's ascent and descent, which KaTeX's faces
differ in, so no box's height sizes the caret. A top-level row with nothing
in it gets a zero-size `oc-empty-row` marker on its baseline so the caret has
a place. Its styles are the `.cm-math-view` rules in `theme/math.ts`; the
field takes the rendering's box exactly (an inline field's 2px of tint each
side are taken back by a negative margin, a block's padding is the
rendering's at the note's font size), so opening or closing maths moves
neither it nor the text around it. The controller focuses it and places the caret once the
widget is in the document.

The model's source is the note's LaTeX, so a step's changes are written as
they are (`writeStep` in `rustField/write.ts`), with no tidy; only a block's
edges are kept to one line break once it spans lines. Each step is written as
it happens: `flush` does nothing. A shortcut's expansion comes as a second
transaction with `isolateHistory`, so ⌘Z after `sin` → `\sin` gives back
`sin`; a matrix edit is isolated the same way. Entering writes nothing.

Undo is the note's: the field's writes are typing transactions
(`math/field/mathFieldEdits.ts`), so a run of keystrokes is one undo step,
and ⌘Z / ⌘⇧Z in the field step the note's history and reload the field from
the note (`sync`), the caret at the end of what changed, compared from both
ends with the common tail stopping at the old caret (`caretAfterEdit`), or
close it when the step moves the caret out of that maths. A field writes
only to the maths it opened on, and only while that LaTeX is still what it
last wrote or loaded.

The note scrolls only to keep a caret on screen, and then only as far as
the nearer edge of the scroller's visible band, under the sticky toolbar
(`scrollMargins`): after each key in the field, its own caret (a new row,
typing at the window's bottom); after entering, the field's caret; after
leaving, the note's caret beside the maths (`noteScroll.ts`). CodeMirror's
`scrollIntoView` isn't used for these, since it measures a position inside
the field as the whole widget. Focus going back to the note holds every
scroller still (`focusNote`): WebKit 26 scrolls to the note's selection on
focus despite `preventScroll`, centring a line just off screen.

The note's selection stays inside the maths while the field is open; a
selection extended from there to past the maths (Shift-click) starts at the
maths' edge, so it holds exactly what is highlighted. A press on the
rendering puts the field's caret where it landed (`takePress`), and presses
in the field are hit-tested by `stopAt` (`lib/maths/geometry`): the innermost
slot under the press — a script, a numerator, a cell — takes the caret at its
nearest gap, and in a block the row whose vertical band holds the press, else
the nearest, bounds that search. A selection whose ends sit at different
depths — dragged from beside a matrix into a cell, or across two cells — is
widened by the model to take each structure it reaches into whole, while a
block's rows select as one run; ←/→ and Backspace/Delete at inline maths,
and those and ↑/↓ from the line beside a block (below), enter it at that
end.

The edit model owns every key but the toolbox's, ⌘⇧M, undo and Enter in a
block (`rustField/keys.ts`) and the pending `\command` list's (below): Esc
(once any toolbox is closed: leave, or revert a shortcut just expanded), an
arrow past the field's edge and Enter or Shift+Enter in inline maths leave
it; Shift+Enter in a block adds a row (a matrix's or an environment's when
the caret is in one), never a second empty one (on an empty row it does
nothing). Enter in a block makes a line outside it, as Enter does in text:
an empty line after the block with the note's caret on it, or before it
when the field's caret is at its very start, so a note starting with a
block can get text above it — its own undo step, the page scrolled only if
that line is off screen (`enterSide`, `newlineBeside` in `fieldNote.ts`);
with a `\command` being typed it commits it, and an open list's Enter
accepts its row first; Tab goes out of a text run, else to the next
empty slot, else types `\qquad`; ⌘Backspace deletes the
caret's line up to the caret (a block's row, or a cell of an environment's
rows, through the structure the caret is in); Backspace in an empty field
removes the maths, and in an empty script (`\cos^{}`) drops it. The steps'
effects call the shared `leaveMaths` and `removeMaths`. Committing a bare
`\text` (or `\textbf`, `\textit`, `\textrm`, `\textnormal`; KaTeX has no
`\mbox`) from the `\` list, or the toolbox's text cells, leaves the caret in
the text (mode `text`); → or Tab at the end of the text goes back to maths. A
`\command` being typed lives outside the source until committed. The view
draws it in the rendering, as the model would type it at the caret
(`mathView/pending.ts`: `\texttt{\textbackslash name}` inserted as a
template, display only), so the maths after it moves aside; its elements
carry no source range, every other keeps its own, and it shows monospace in
the brand colour at 0.8em (the maths' x-height), the caret after it.

The field has one list (`mathView/popover/`), which a pending `\command` and
Space open. While a `\command` is pending it hangs from the start of that
`\name`. Right after `\` it holds the field's picks (`fieldPicks`, below);
once letters are typed, every palette and completion template whose command
starts with the name, one row per template (`\sqrt` offers `\sqrt{}` and
`\sqrt[]{}`), the command typed exactly first, its bare form before its
templates, then common commands and shorter names. For a name nothing starts
with there is no list. Wherever it opens, it sits under the caret, flips
above when it would pass the bottom of what is visible (the window, the
note's scroller, any clipping ancestor) and there is more room above, and
slides sideways to stay inside them (`placeList`). The first row is
highlighted; ↑/↓ move the highlight and Space, Tab or Enter accept it, as a
click on a row does: the template goes in for the typed name (its first slot
taking the selection), the caret in its first empty slot — the index for
`\sqrt[]{}`, from which Tab or → reach the radicand. So `\` then Space
inserts the first pick. Esc right after a bare `\` closes the list and keeps
the `\` pending with none, so Space then types a control space `\ `; typing a
letter brings the list back. With no list, Space, Tab or Enter commit the
name as typed, and `\,` and the other non-letters make their control symbols
as before. Esc once letters are typed, and Backspace, stay the model's.

Copy and cut write the selection's LaTeX wrapped by its shape as
`text/plain` — within one line `$…$`; over more than one line (two rows, or
an array whose rows it holds, taken whole with its environment) or the
whole of a display formula `$$` lines, one row each, also as
`BLOCK_MATH_TYPE` — and bare as `application/x-latex` (`copied` in
`lib/markdown/mathSelection/clipboard.ts`, as chat and files copy); cut then
deletes as Backspace does. WebKit enables Copy and Cut only for a text
selection, so the field cancels `beforecopy`/`beforecut` while it has one.
Pasted maths goes through the model, refused unless it renders.

Focus in the field counts as the editor's (`core/liveFocus.ts`). Maths the
field can't read cleanly, a multi-line block inside a quote or list, and
maths switched with the toolbox's TeX control (⌘⇧M) are typed as LaTeX source
(TeX mode, as in Raw mode) until the caret leaves them. A formula the engine
traps on drops to TeX mode for the session (`markFieldTrap`), so it never
reopens to trap again; the same for maths the field fails to open on.

## Rendered maths is one unit to the selection, the clipboard and the caret

The rendering is `MathWidget` in
`app/src/components/documents/editor/live-preview/widgets/math.ts`, with the
atomic ranges in `live-preview/livePreview/`. Once the maths
engine has loaded, every rendering's range is atomic, so a drag or Shift+arrow
covers maths whole and Shift-click extends the selection over it. A
rendering a selection covers draws bands over its atoms, as chat does
(`WholeBands`, `mathSelection/whole.ts`; the rendering is `fieldHtml`, the
field's own, source map on), and the note's highlight leaves it out: the
selection layer is `core/selectionLayer.ts`, `drawSelection`'s with the
rendered maths cut out (`selectionGaps`, a block with its line break).
Native selection never paints the rendering, and a selection taking a
whole block keeps it rendered rather than opening the field. Copy and cut always write the note's source (`$$…$$` lines), since
a rendering passes clipboard events to CodeMirror. A rendered block takes
its column's width (`contain: inline-size` on `.cm-math-display`, as on the
field's block) and scrolls a too-wide formula, rather than widening the
note.

A block has no caret position of its own beside it: a caret at either
edge of its lines — however it gets there: an arrow, a click, Home/End, an
undo, a paste, leaving another field — is in the block, and its field opens
with its caret at that end (`targetAt` in
`math/field/mathField/visual-state.ts`, `placeCaret` in `rustField/mount.ts`);
only a selection taking the whole block, or running across it, keeps it
rendered. A press anywhere on the rendering opens the field in the row
under the pointer — the row whose vertical band holds it, else the nearest
— at that row's nearest gap, so a press far to the right of a row ends that
row, and a press in the padding above or below the formula its first or
last row; in the open field's padding, the field's start or end
(`pressBeside`). From the line beside a block, ↑/↓, ← at its start, → at its
end, and Backspace/Delete from a line with text (which would join it to a
`$$`) go straight into the field at the near end (`enterBlock` in
`math/field/mathField/keys.ts`). Leaving a field never edits the note
(`leaveMaths`): the caret goes to the line before or after the block, into
a touching block's field at its near end, and with nothing past the note's
start or end the field stays; Enter in the field adds a line (above).

Pasted into
the field, maths goes in at the caret and markdown with prose around maths
lands in the note just after that maths; LaTeX copied from a field pastes
outside maths in the shape it was copied
from (`fieldLatexPaste`, reading the bare `application/x-latex`): a block's
copy carries its `$$` lines (`BLOCK_MATH_TYPE`) and goes in on lines of its
own (inline in a table row), the rest as `$…$`. A
chip under the pasted maths (`PastedView`, `offerShapeSwitch`) offers
"Convert to block" or "Convert to inline" (`shapeChange`, the toolbox's own
switch) and a close button; a switch keeps it up for the way back, and it
goes 5 s after its last use with the pointer off it, on Esc, or with any
other edit.

The rendering and the field lay out alike: array and matrix rows
take `MATH_ARRAYSTRETCH` and a block's top-level `\\` lines
`MATH_LINE_GAP` (KaTeX's `.katex-newline` in `theme/math.ts`), and an `array`
that is all a `\left…\right` holds drops its outer column padding in both
(`math/hugArrays.ts`), so `\left[\begin{array}…\right]` hugs its brackets
like `bmatrix`. These are render-time only: the note's text never changes for
display.

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
Palette clicks, accepted completions, rows accepted from the field's list
(as themselves) and `\commands` typed out — in TeX (a non-letter typed after
the name) or committed in the visual field — all count, a typed command as
its palette entry (`math/tools/mathUsage.ts`, `localStorage`); the Recent row
and Popular are across all notes, while each subject keeps its own last five
to lead the field's picks.

In TeX and Raw mode maths
is typed as LaTeX (Space is a space), and the toolbox's cells insert
`snippet()`s whose `{}` slots are Tab fields; the snippet keymap is
`Prec.highest`, above the note's Tab. Inside maths, `\` plus a letter
opens completion with rendered previews, whatever the toolbox is doing; it is
the editor's one `autocompletion()` (`core/extensions.ts`), so other sources
join its `override`. It never opens over maths a visual field is open on
(`visualMath`): the field's writes are typing transactions, and the field
has its own list. The caret between a lone `$$` pair (what a typed `$`
and Σ insert) counts as empty inline maths (`emptyPair` in `math/mathContext.ts`):
mid-line `$$` never parses, and alone on a line it parses as an unclosed
block opener, which `mathAt` reads as no maths.

## Typing `$` opens maths

In Live mode a typed `$` writes `$$` with the caret between, which is an
empty inline field, ready to type in (`dollarTyping` in `livePreview/edges.ts`,
`Prec.high` so the shorthand handler doesn't type into the pair first).
At a line's start (container markup aside, or on the line it opens beside a
table) it writes `\(\)` instead: `$$` there opens a display block in every
Markdown reader, which would run to the next `$$` and take the text between.
The parser takes an empty `\(\)` as inline maths (`parenMath`), and the
field's first write turns it into `$…$` (`noteWrite`).
A second `$` there — in the empty field (`fieldKey`) or in the note's source
before the maths engine is ready — turns the pair into an empty block with the caret
on its line (`emptyPairToBlock`, which is `toggleShape`). A `$` stays a
plain character after `\` or `$`, in code, before a word (`$5`) and with a
selection; Raw mode never pairs. Inline maths left empty (`$$`, `$ $`,
`\(\)`) is deleted, outside the history, once the field closes with the
caret outside it (`dropEmptyInline` in `math/field/mathField/index.ts`); a caret still inside
(TeX mode, the window losing focus) keeps it.

## In the visual field, Space opens the picks at the caret

`fieldKey` in `mathTools/keys.ts`, reached through `fieldKeys` after the
field's open list and ahead of its own keys (`rustField/keys.ts`), opens the
field's list (above) on Space as its picks: the subject's five last used,
most recent first, then Popular, twenty in all (`fieldPicks`), the first
nine numbered, with an "All maths tools" row under them. No row is
highlighted at first: ↑/↓ start the highlight, then Space, Tab, Enter or a
click accept it (it counts as used); 1–9 accept that row. Space with none
highlighted, or that row, opens the full toolbox (`fieldTools`); Esc
closes the list and a second Esc leaves the field; Tab, Enter and any other
key close it and go on as usual, as does a caret move in the field. Space
inside `\text{}`, beside a text atom and in a `\command` being typed is the
field's (`spaceFree` is false there), and after a term in a matrix or bracket group it starts a cell (below). The full toolbox under
the field drops the preview, its cells insert into the field (`insertTemplate`:
slots become empty slots, the first taking the selection) and its TeX control
switches to TeX mode; there a Visual control switches back. Neither toolbox
ever takes focus. The field says so on any empty line (`syncHint` in
`rustField/hint.ts`): an empty inline field
shows "Space (␣) for math tools" in flow after it, inside its tint; an
empty block, or one empty row of its lines, centres "Start typing or Space
(␣) for math tools" on that line with the caret drawn just before it (the
hint's `::before`; the view's own is hidden). It goes once the
line has anything in it.

A block's field writes its LaTeX with no blank line at either end and never
two in a row (`squeezeBlankLines`, `math/field/mathFieldEdits.ts`), and the break after
the opening `$$` and before the closing one is a single newline: an empty
last row is kept as a trailing `\\`, not a blank line, while the field is
on the block; once it leaves, empty rows at the block's end go
(`dropEndRows`, `withoutEndRows`), so the rendering has no blank row under
the formula. Blank lines already in a
block go when the field opens on it (`dropBlankLines`). Both cleanups stay
outside the history. The empty-line hint (12px, one tight line) sits at the height of the empty
row's caret, clamped inside the field's box.

## Matrices are typed as in MATLAB

In the visual field `[a b; c d]` types a matrix; the edit model does it, in
`app/math-core/edit/src/command/grid/`. The caret's grid is the structure
it is directly in: a cell of a `matrix`, `pmatrix`, `bmatrix`, `Bmatrix`,
`vmatrix`, `Vmatrix` or `smallmatrix` (not `array`, `cases`, `aligned`),
or a bracket group whose delimiters match one (`(`, `[`, `\{`, `|`, `\|`),
which is one cell and becomes that matrix at its first new cell or row.
The group's right bracket is a `\left…\right` pair or an opening
bracket with whatever closes it later in the same slot. Maths mode only;
`spaceFree` (the model's own) keeps the picks off a Space the grid
takes.

- **Space** after a term ends the cell: into the next cell when the caret is
  at the cell's end and that cell is empty, else into a new column (what
  followed the caret moves into it). In an empty cell, at a cell's start and
  after an operator, relation, punctuation, opening or `\sin`-like operator
  it opens the picks as anywhere else. A cell holding only a binary
  operator or relation rejoins the cell before it on Space (MATLAB's
  `[a + b]`): its column goes when nothing else is in it, else the row's
  later cells shift left. So `(a + b)` typed with spaces stays plain
  brackets, while `[1 -1]` is two cells.
- **`;`** goes to the start of the next row: a new empty row, unless the
  next row is already empty. Inside a matrix it is never typed. In a
  bracket group the `;` is not written first, so ⌘Z after the matrix
  appears gives back `f(x`.
- **Backspace** at the start of an empty cell, in a grid of more than one,
  removes its column when all empty, else its row when all empty, else steps
  back to the end of the cell before. One cell left turns back into its
  bracket group, left open, the closer the user's to type, unless
  scripts follow it (`^T`), which keep their closer.
- **The closing bracket** typed in a matrix's cell drops trailing empty
  rows and columns and puts the caret after the matrix; one cell left
  becomes a closed bracket group. Only a matrix's own closer counts: `)` for
  `pmatrix`, `]` `bmatrix`, `}` `Bmatrix`, `|` `vmatrix` (none for `matrix`,
  `smallmatrix` or `Vmatrix`).

Rows are padded to the widest before any edit, so `[a b; c d e f g]` comes
out with empty cells after `b`; a row stops at ten columns. Each edit is
one undo step. New cells are empty cells in the source (Tab reaches them),
and the edit rewrites only the matrix's content, keeping the text of every
cell and separator it didn't touch, as a step marked `isolate`.

## Typed shorthands expand inside maths

The rule tables are in
`app/src/components/documents/editor/math/tools/shorthand/tables.ts`
(the rules that apply them in `rules.ts`): `a/` → `\frac{a}{}`, `sr` → `^2`, `@a` → `\alpha`, `->` → `\to`,
`sin ` → `\sin `, `\left…\right` around a closed group holding a tall
construct. Never inside `\text{}`-like arguments. The typed character lands
first and the rewrite is its own history event, so ⌘Z gives back what was
typed.

The visual field's shortcuts are its edit model's
(`app/math-core/edit/src/shortcut/`, the table in `table.rs`, which holds the
note's shorthands among its entries). Only one key
typed in maths takes part — never text mode, a pending `\command`, an IME
string, a template, a paste, or a name argument (`\mathbb{RR}`,
`\operatorname{sinc}`). A letter key expands only when the whole run of
letters before the caret is the key (`sin` and `x+sin` expand, `xsin` and
`card` stay; `2pi` is `2\pi`); a key starting with a symbol matches the
keys just typed. A power (`sr`, `cb`, `rd`, `invs`) takes one atom as its
base: one letter of the run (`xsr` is `x^2`), else the operand before it.
`!=` after an operand stays a factorial and `=` (`n!=`). A longer key
re-expands from before its first key (`sin` then `h` is `\sinh`). The key
lands as typed, then the expansion is a second write, its own undo step,
so ⌘Z gives back the typed keys; Esc right after an expansion does the
same without leaving the field, and the keys typed next that lead to a
longer shortcut stay as typed (`sin`, Esc, `h` is `sinh`).
