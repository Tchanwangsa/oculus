# Maths in the note editor

A note's maths is typed as LaTeX in either mode, with a popover beside it and
typed shorthands. The delimiters and the rest of the editor are [editor.md](./editor.md).

## Where

| Piece | Location |
| --- | --- |
| Delimiters, the maths under the caret | `app/src/components/documents/editor/mathSyntax.ts`, `app/src/components/documents/editor/mathContext.ts` |
| Popover and palette | `app/src/components/documents/editor/mathTools.ts`, `app/src/components/documents/editor/mathPalette.ts` |
| Shorthands | `app/src/components/documents/editor/mathShorthand.ts` |

## Maths is typed as LaTeX in the note, with a popover beside it

The popover is `app/src/components/documents/editor/mathTools.ts`: while the
caret is in maths (`mathAt` in `mathContext.ts`), in either mode, a CodeMirror
tooltip under the maths shows a live KaTeX preview (the parse error in red), recents
(`localStorage`), a one-row tab strip that scrolls sideways, and a palette
three rows tall that scrolls, with a matrix-size grid (`mathPalette.ts`). Esc hides it until the caret leaves that maths. Inside
maths, `\` plus a letter opens completion with KaTeX previews; it is the
editor's one `autocompletion()` (`extensions.ts`), so other sources join its
`override`. Palette buttons and completions insert `snippet()`s whose `{}`
slots are Tab fields; the snippet keymap is `Prec.highest`, above the note's
Tab. The caret between a lone `$$` pair (what Σ inserts mid-line) counts as
empty maths, since `$$` never parses inline.

## Typed shorthands expand inside maths

The rule tables are at the top of
`app/src/components/documents/editor/mathShorthand.ts`: `a/` → `\frac{a}{}`, `sr` → `^2`, `@a` → `\alpha`, `->` → `\to`,
`sin ` → `\sin `, `\left…\right` around a closed group holding a tall
construct. Never inside `\text{}`-like arguments. The typed character lands
first and the rewrite is its own history event, so ⌘Z gives back what was
typed.
