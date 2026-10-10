import { muted, mono, type ThemeSpec } from "./tokens";

export const code: ThemeSpec = {
  ".cm-line.cm-codeblock": {
    fontFamily: mono,
    fontSize: "13px",
    backgroundColor: "var(--color-surface-raised)",
    padding: "0 12px",
  },
  // The gap around a block is a transparent border the fill stops short of:
  // lines can't take margins, and the vertical radius grows by its width.
  ".cm-line.cm-codeblock-first": {
    position: "relative",
    borderTop: "10px solid transparent",
    backgroundClip: "padding-box",
    borderTopLeftRadius: "8px 18px",
    borderTopRightRadius: "8px 18px",
    paddingTop: "8px",
  },
  ".cm-line.cm-codeblock-last": {
    borderBottom: "10px solid transparent",
    backgroundClip: "padding-box",
    borderBottomLeftRadius: "8px 18px",
    borderBottomRightRadius: "8px 18px",
    paddingBottom: "8px",
  },
  ".cm-code-info": { fontFamily: "var(--font-sans)", fontSize: "11px", color: muted },
  ".cm-code-text": { fontFamily: mono, fontSize: "0.92em" },
  // Live mode's header over a hidden opening fence. The copy button sits at
  // the first line's right edge whatever indent or quote precedes the fence.
  ".cm-code-label": {
    fontFamily: "var(--font-sans)",
    fontSize: "11px",
    color: muted,
    cursor: "pointer",
  },
  ".cm-code-label:hover": { color: "var(--color-foreground)" },
  ".cm-code-copy": {
    position: "absolute",
    top: "8px",
    right: "6px",
    border: "0",
    padding: "0 8px",
    borderRadius: "9999px",
    backgroundColor: "transparent",
    color: muted,
    fontFamily: "var(--font-sans)",
    fontSize: "11px",
    lineHeight: "20px",
    cursor: "pointer",
    transition: "background-color 120ms, color 120ms",
  },
  // `accent` is the code block's own fill, so the hover mixes from the text.
  ".cm-code-copy:hover": {
    backgroundColor: `color-mix(in srgb, ${muted} 16%, transparent)`,
    color: "var(--color-foreground)",
  },
  // A hidden closing fence keeps only the block's bottom padding.
  ".cm-line.cm-codeblock-close-hidden": { fontSize: "0", lineHeight: "0" },
};
