import { muted, type ThemeSpec } from "./tokens";

export const quotesRules: ThemeSpec = {
  ".cm-line.cm-quote": {
    borderLeft: "2px solid var(--color-border)",
    paddingLeft: "1em",
    color: muted,
  },
  ".cm-hr": { display: "flex", alignItems: "center", height: "1.7em", cursor: "text" },
  ".cm-hr::before": { content: '""', flex: "1", borderTop: "1px solid var(--color-border)" },
  ".cm-line.cm-hr-gap": { lineHeight: "0.6" },
};
