import { brand, muted, mono, type ThemeSpec } from "./tokens";

export const completion: ThemeSpec = {
  "&.cm-editor .cm-content .cm-snippetField": {
    backgroundColor: `color-mix(in srgb, ${brand} 14%, transparent)`,
    borderRadius: "3px",
  },
  // Completion list (`autocompletion()` in `extensions.ts`)
  "&.cm-editor .cm-tooltip.cm-tooltip-autocomplete > ul": {
    fontFamily: "var(--font-sans)",
    fontSize: "12px",
    minWidth: "220px",
    maxHeight: "16em",
    padding: "4px",
  },
  "&.cm-editor .cm-tooltip.cm-tooltip-autocomplete > ul > li": {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    padding: "2px 8px 2px 4px",
    borderRadius: "6px",
    lineHeight: "24px",
  },
  "&.cm-editor .cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]": {
    backgroundColor: "var(--color-accent)",
    color: "var(--color-foreground)",
  },
  ".cm-completionLabel": { fontFamily: mono, fontSize: "12px" },
  ".cm-completionMatchedText": { textDecoration: "none", color: brand },
  ".cm-completionDetail": {
    marginLeft: "auto",
    paddingLeft: "12px",
    fontStyle: "normal",
    fontFamily: mono,
    fontSize: "11px",
    color: muted,
  },
  ".cm-math-option-preview": {
    flex: "none",
    width: "44px",
    height: "24px",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    overflow: "hidden",
    fontSize: "10px",
  },
  // `@` file mentions (`mentions.ts`): a sans title, a muted folder and glyph.
  ".cm-mention-option": { maxWidth: "420px" },
  ".cm-mention-option .cm-completionLabel": {
    fontFamily: "var(--font-sans)",
    minWidth: "0",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  ".cm-mention-option .cm-completionDetail": { fontFamily: "var(--font-sans)", whiteSpace: "nowrap" },
  ".cm-mention-icon": { flex: "none", display: "flex", color: muted },
};
