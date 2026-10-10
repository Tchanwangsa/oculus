import { brand, mono, type ThemeSpec } from "./tokens";

export const inline: ThemeSpec = {
  // Inline marks. Bold in a heading outweighs the heading, including the
  // highlight spans nested in it.
  ".cm-strong": { fontWeight: "600" },
  ".cm-h1 .cm-strong, .cm-h2 .cm-strong, .cm-h3 .cm-strong, .cm-h4 .cm-strong, .cm-h5 .cm-strong, .cm-h6 .cm-strong, .cm-h1 .cm-strong *, .cm-h2 .cm-strong *, .cm-h3 .cm-strong *, .cm-h4 .cm-strong *, .cm-h5 .cm-strong *, .cm-h6 .cm-strong *":
    { fontWeight: "800" },
  ".cm-em": { fontStyle: "italic" },
  ".cm-strike": { textDecoration: "line-through" },
  ".cm-inline-code": {
    fontFamily: mono,
    fontSize: "0.9em",
    backgroundColor: "var(--color-surface-raised)",
    borderRadius: "4px",
    padding: "0.1em 0.3em",
  },
  ".cm-link": {
    color: brand,
    textDecoration: "underline",
    textDecorationColor: `color-mix(in srgb, ${brand} 35%, transparent)`,
    textUnderlineOffset: "2px",
  },
};
