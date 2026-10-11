import { muted, mono, type ThemeSpec } from "./tokens";

export const pictures: ThemeSpec = {
  ".cm-image img": {
    display: "inline-block",
    maxWidth: "100%",
    maxHeight: "20rem",
    borderRadius: "8px",
    border: "1px solid var(--color-border)",
    verticalAlign: "bottom",
    cursor: "text",
  },
  ".cm-image-block": { padding: "6px 0" },
  ".cm-image-missing": { color: muted, fontStyle: "italic" },
  // Mermaid diagrams; the picture's box is `.diagram` (`index.css`).
  ".cm-mermaid": { padding: "6px 0", cursor: "text" },
  ".cm-mermaid-editing:empty": { padding: "0" },
  ".cm-mermaid-source": { margin: "0", color: muted, fontFamily: mono, fontSize: "13px", whiteSpace: "pre-wrap" },
  ".cm-mermaid-error .cm-mermaid-source": { color: "var(--color-destructive)" },
};
