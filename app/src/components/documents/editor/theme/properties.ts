import { muted, type ThemeSpec } from "./tokens";

export const properties: ThemeSpec = {
  ".cm-props": { padding: "4px 0 10px" },
  ".cm-props-card": {
    border: "1px solid var(--color-border)",
    borderRadius: "8px",
    padding: "8px 12px 10px",
    cursor: "text",
  },
  ".cm-props-label": { fontSize: "12px", fontWeight: "500", color: muted, marginBottom: "4px" },
  ".cm-props-grid": {
    display: "grid",
    gridTemplateColumns: "8.5rem minmax(0, 1fr)",
    columnGap: "12px",
    rowGap: "2px",
    alignItems: "baseline",
  },
  ".cm-prop-key": {
    fontSize: "12.5px",
    color: muted,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  ".cm-prop-value": { minWidth: "0", whiteSpace: "pre-wrap", overflowWrap: "anywhere" },
  ".cm-prop-number": { fontVariantNumeric: "tabular-nums" },
  ".cm-prop-raw": { gridColumn: "1 / -1", whiteSpace: "pre-wrap", overflowWrap: "anywhere", color: muted },
  ".cm-prop-chip": {
    display: "inline-block",
    margin: "1px 4px 1px 0",
    padding: "0 8px",
    borderRadius: "9999px",
    backgroundColor: "var(--color-surface-raised)",
    fontSize: "12px",
    lineHeight: "20px",
  },
};
