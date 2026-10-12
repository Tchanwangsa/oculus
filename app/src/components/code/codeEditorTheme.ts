import { EditorView } from "@codemirror/view";

const brand = "var(--color-brand)";

/** `CodeEditor`'s frame around Raw mode's look (`rawCodeChrome`): unlike a
 *  note, the editor fills its box and scrolls itself, so the gutter is fixed
 *  and needs the card's fill for code scrolled sideways under it. */
export const codeEditorTheme = EditorView.theme({
  "&": { height: "100%", color: "var(--color-foreground)", backgroundColor: "transparent" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { overflow: "auto" },
  ".cm-content": { padding: "10px 0 40px", caretColor: brand },
  ".cm-line": { padding: "0 16px 0 2px" },
  ".cm-cursor, .cm-dropCursor": { borderLeft: `1.5px solid ${brand}` },
  // Matches the base theme's specificity, which otherwise wins.
  "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection":
    { backgroundColor: `color-mix(in srgb, ${brand} 22%, transparent)` },
  // One step more specific than Raw mode's unfixed gutter.
  "&.cm-editor .cm-scroller .cm-gutters": {
    backgroundColor: "var(--color-card)",
    paddingLeft: "8px",
    zIndex: "1",
  },
  "&.cm-focused .cm-matchingBracket": {
    backgroundColor: `color-mix(in srgb, ${brand} 16%, transparent)`,
    outline: "none",
  },
});
