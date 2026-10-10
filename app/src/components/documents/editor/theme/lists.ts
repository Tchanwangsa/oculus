import { brand, muted, type ThemeSpec } from "./tokens";

export const lists: ThemeSpec = {
  ".cm-list-bullet": { color: muted, display: "inline-block", minWidth: "0.6em" },
  ".cm-list-number": { color: muted },
  ".cm-task-box": {
    display: "inline-block",
    position: "relative",
    width: "14px",
    height: "14px",
    marginRight: "2px",
    verticalAlign: "-2px",
    border: `1.5px solid color-mix(in srgb, ${muted} 70%, transparent)`,
    borderRadius: "4px",
    cursor: "pointer",
    boxSizing: "border-box",
  },
  ".cm-task-box[aria-checked=true]": { backgroundColor: brand, borderColor: brand },
  ".cm-task-box[aria-checked=true]::after": {
    content: '""',
    position: "absolute",
    left: "3.5px",
    top: "0.5px",
    width: "4px",
    height: "8px",
    border: "solid var(--color-brand-foreground)",
    borderWidth: "0 1.5px 1.5px 0",
    transform: "rotate(45deg)",
  },
  ".cm-task-done": { color: muted, textDecoration: "line-through" },
};
