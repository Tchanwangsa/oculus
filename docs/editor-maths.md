# Maths in the note editor

In Live mode a note's maths renders, edits in a MathLive visual field, and has
a toolbox, quick picks and typed shorthands; in TeX and Raw mode it is typed as
LaTeX. The delimiters and the rest of the editor are [editor.md](./editor.md).

## Where

| Piece | Location |
| --- | --- |
| Delimiters, the maths under the caret | `app/src/components/documents/editor/mathSyntax.ts`, `app/src/components/documents/editor/mathContext.ts` |
| The visual field and its writes | `app/src/components/documents/editor/mathField.ts`, `app/src/components/documents/editor/mathFieldEdits.ts` |
| Rendered maths, atomic ranges, edge keys | `app/src/components/documents/editor/widgets.ts`, `app/src/components/documents/editor/livePreview.ts` |
| Toolbox, palette, quick picks, usage | `app/src/components/documents/editor/mathTools.ts`, `app/src/components/documents/editor/mathPalette.ts`, `app/src/components/documents/editor/mathUsage.ts` |
| Shorthands | `app/src/components/documents/editor/mathShorthand.ts` |

## Live mode edits maths visually

In `app/src/components/documents/editor/mathField.ts`, while the caret is in
maths, a MathLive `<math-field>` stands in for the rendering — inline for
`$…$`; for a `$$` block a shaded field the column's width with the maths
centred, its rows too — with slots for a fraction's parts or a
sum's limits, and `\` commands that become one symbol (MathLive's own
command list, restyled in `index.css`). MathLive is a lazy chunk imported as
a Live editor mounts; it reuses KaTeX's bundled fonts and has sounds and the
virtual keyboard off. Once it has loaded, MathLive also draws the maths the
field could open (`staticMath`: its static markup, with `mathlive/static.css`
injected minus its `@font-face` rules), in the box the field takes
(`.cm-math-ml` in `theme.ts`: the field's size, line height and padding,
and `\text{}` in KaTeX_Main without kerning or ligatures, as the field
sets it a span per letter), so entering maths moves neither it nor its line; while MathLive loads, if it
fails, and for maths the field can't take, KaTeX draws it.

A field mounts
over a static copy of its maths, unseen, and replaces it once focus has
rendered it (`FieldController.mount`): MathLive otherwise draws a field a
frame after it connects, and the note, briefly shorter, would clamp a page
scrolled to its end.

Each edit rewrites only the LaTeX between the
delimiters (MathLive's serialisation, placeholders dropped, a block's
environment one row per line); entering writes nothing. Undo is the note's:
the field's writes are typing transactions (`mathFieldEdits.ts`), so a run
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
its rows with a stylesheet adopted into the field's shadow root); Tab goes to the
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

Focus in the field counts as the editor's (`liveFocus.ts`). Maths
that MathLive or KaTeX can't read cleanly, a multi-line block inside a quote
or list, and maths switched with the toolbox's TeX control (⌘⇧M) are typed
as LaTeX source (TeX mode, as in Raw mode) until the caret leaves them.

## Rendered maths is one unit to the selection, the clipboard and the caret

The rendering is `MathWidget` in
`app/src/components/documents/editor/widgets.ts`, with the atomic ranges and
edge keys in `livePreview.ts`. Once MathLive has loaded, every rendering's range is atomic, so a drag or Shift+arrow covers
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
steps onto a line with text rather than joining it to a `$$`.

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
`MATH_LINE_GAP` (KaTeX's `.newline` in `theme.ts`, MathLive's root
`lines` table); MathLive's `array` is centred on the axis as KaTeX draws it
(it hangs from its first row there); and an `array` that is all a
`\left…\right` holds drops its outer column padding in both, so
`\left[\begin{array}…\right]` hugs its brackets like `bmatrix`. MathLive's
fixes patch its internal array atom (`patchArrays`), which its static
markup goes through too; KaTeX's are render-time only, and the note's text
never changes for display.

## Maths has a toolbox, opened on demand

The toolbox is `app/src/components/documents/editor/mathTools.ts`. Nothing
shows while
the caret is in maths (`mathAt` in `mathContext.ts`) until it is asked
for: Σ in the toolbar or ⌘⇧Space (Mod-Shift-Space; Ctrl-Space is
completion, and macOS reserves Ctrl- and ⌘-Space) opens or closes it, and
it stays open until Esc, its close button, or the caret leaving that maths
(`mathToolsField` holds what is open and on which maths, mapped through
edits). It is a CodeMirror tooltip centred under the maths (a block on the
text column): a live KaTeX preview (the parse error in red), a Recent row
and a tab strip, each one row that scrolls sideways, and a palette three
rows tall that scrolls, with a matrix-size grid (`mathPalette.ts`). Beside
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
palette entry (`mathUsage.ts`, `localStorage`); the Recent row and Popular
are across all notes, while each subject keeps its own recents for the
quick picks.

In TeX and Raw mode maths
is typed as LaTeX (Space is a space), and the toolbox's cells insert
`snippet()`s whose `{}` slots are Tab fields; the snippet keymap is
`Prec.highest`, above the note's Tab. Inside maths, `\` plus a letter
opens completion with KaTeX previews, whatever the toolbox is doing; it is
the editor's one `autocompletion()` (`extensions.ts`), so other sources
join its `override`. The caret between a lone `$$` pair (what Σ inserts
mid-line) counts as empty maths, since `$$` never parses inline.

## In the visual field, Space opens quick picks at the caret

The strip is `QuickPicksView` and `fieldKey` in `mathTools.ts`, reached
through `fieldKeys` ahead of the field's own keys in `mathField.ts`. MathLive
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
being typed keeps MathLive's meaning. The full toolbox under the field
drops the preview, its cells insert into the field (slots become MathLive
placeholders) and its TeX control switches to TeX mode; there a Visual
control switches back. Neither toolbox ever takes focus. An empty block's
field says so beside its caret ("Press Space for maths tools",
`syncHint` in `mathField.ts`).

## Typed shorthands expand inside maths

The rule tables are at the top of
`app/src/components/documents/editor/mathShorthand.ts`: `a/` → `\frac{a}{}`, `sr` → `^2`, `@a` → `\alpha`, `->` → `\to`,
`sin ` → `\sin `, `\left…\right` around a closed group holding a tall
construct. Never inside `\text{}`-like arguments. The typed character lands
first and the rewrite is its own history event, so ⌘Z gives back what was
typed. The visual field gets the Greek, power and operator rules as MathLive
inline shortcuts, over MathLive's defaults minus the ones that turn letter
runs into units or words (`PRUNED` in `mathField.ts`).
