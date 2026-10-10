import { brand, muted, type ThemeSpec } from "./tokens";

export const tables: ThemeSpec = {
  // Tables: the frame scrolls sideways so a wide table never widens the page.
  ".cm-table": { padding: "6px 0" },
  ".cm-table-frame": {
    display: "grid",
    gridTemplateColumns: "minmax(0, auto) auto",
    width: "fit-content",
    maxWidth: "100%",
    position: "relative",
  },
  ".cm-table-scroll": {
    overflowX: "auto",
    border: "1px solid var(--color-border)",
    borderRadius: "8px",
  },
  ".cm-table table": {
    borderCollapse: "collapse",
    // Hidden wins collapsed-border conflicts: the frame draws the outside.
    borderStyle: "hidden",
    fontSize: "14px",
    lineHeight: "1.5",
  },
  ".cm-table th, .cm-table td": {
    border: "1px solid var(--color-border)",
    padding: "0",
    textAlign: "left",
    verticalAlign: "top",
  },
  ".cm-table th": { fontWeight: "600", backgroundColor: "var(--color-surface)" },
  ".cm-table-scroll:focus": { outline: "none" },
  ".cm-table-handle": {
    position: "absolute",
    zIndex: "2",
    border: "0",
    padding: "0",
    borderRadius: "9999px",
    backgroundColor: `color-mix(in srgb, ${muted} 45%, transparent)`,
    transform: "translate(-50%, -50%)",
    cursor: "grab",
    touchAction: "none",
    userSelect: "none",
    WebkitUserSelect: "none",
    opacity: "0",
    transition: "opacity 120ms",
  },
  // A thin pill with a press target larger than it looks.
  ".cm-table-handle::before": { content: '""', position: "absolute", inset: "-6px" },
  ".cm-table-handle-col": { width: "20px", height: "6px" },
  ".cm-table-handle-row": { width: "6px", height: "20px" },
  ".cm-table-handle[hidden]": { display: "none" },
  ".cm-table:hover .cm-table-handle, .cm-table:focus-within .cm-table-handle": { opacity: "1" },
  ".cm-table-handle:hover, .cm-table-handle.cm-table-handle-on": { backgroundColor: brand },
  ".cm-table-moving, .cm-table-moving *": { cursor: "grabbing" },
  ".cm-table-drop": {
    position: "absolute",
    zIndex: "3",
    width: "2px",
    height: "2px",
    transform: "translate(-1px, -1px)",
    backgroundColor: brand,
    borderRadius: "1px",
    pointerEvents: "none",
  },
  ".cm-table-drop[hidden]": { display: "none" },
  ".cm-table .cm-table-selected": {
    backgroundImage: `linear-gradient(color-mix(in srgb, ${brand} 14%, transparent), color-mix(in srgb, ${brand} 14%, transparent))`,
  },
  // A drag across cells must not also select text inside the first one.
  ".cm-table-ranged .cm-table-cell": { userSelect: "none", WebkitUserSelect: "none" },
  ".cm-table th:focus-within, .cm-table td:focus-within": {
    boxShadow: `inset 0 0 0 1.5px color-mix(in srgb, ${brand} 60%, transparent)`,
  },
  ".cm-table-cell": {
    minWidth: "7em",
    maxWidth: "28em",
    // An empty div has no line box; without this an all-empty row shrinks
    // to its padding.
    minHeight: "calc(1.5em + 12px)",
    padding: "6px 10px",
    whiteSpace: "pre-wrap",
    // The editor's `word-break` would let a cell shrink below a word.
    wordBreak: "normal",
    overflowWrap: "break-word",
    outline: "none",
    cursor: "text",
  },
  ".cm-table-add": {
    border: "0",
    padding: "0",
    borderRadius: "9999px",
    backgroundColor: "transparent",
    color: muted,
    fontSize: "13px",
    lineHeight: "1",
    cursor: "pointer",
    opacity: "0",
    transition: "opacity 120ms, background-color 120ms",
  },
  ".cm-table:hover .cm-table-add, .cm-table:focus-within .cm-table-add": { opacity: "1" },
  ".cm-table-add:hover": { backgroundColor: "var(--color-accent)", color: "var(--color-foreground)" },
  ".cm-table-add-col": { width: "18px", marginLeft: "4px" },
  ".cm-table-add-row": { gridColumn: "1", height: "18px", marginTop: "4px" },
};
