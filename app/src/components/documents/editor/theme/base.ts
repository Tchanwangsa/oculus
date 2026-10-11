import { brand, muted, type ThemeSpec } from "./tokens";

export const base: ThemeSpec = {
  "&": {
    color: "var(--color-foreground)",
    backgroundColor: "transparent",
    fontSize: "14px",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": {
    overflow: "visible",
    height: "auto",
    fontFamily: "var(--font-sans)",
    lineHeight: "1.7",
  },
  ".cm-content": {
    padding: "0",
    minHeight: "50vh",
    caretColor: brand,
  },
  ".cm-line": { padding: "0" },
  ".cm-cursor, .cm-dropCursor": { borderLeft: `1.5px solid ${brand}` },
  // Matches the base theme's specificity, which otherwise wins.
  "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection":
    { backgroundColor: `color-mix(in srgb, ${brand} 22%, transparent)` },
  ".cm-placeholder": { color: `color-mix(in srgb, ${muted} 40%, transparent)` },
};
