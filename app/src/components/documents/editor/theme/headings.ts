import { muted, type ThemeSpec } from "./tokens";

export const headings: ThemeSpec = {
  ".cm-h1, .cm-h2, .cm-h3, .cm-h4, .cm-h5, .cm-h6": {
    fontFamily: "var(--font-display)",
    fontWeight: "650",
    lineHeight: "1.4",
  },
  // Padding, not margin: CodeMirror measures line heights.
  ".cm-line.cm-h1": { fontSize: "1.6em", paddingTop: "0.5em", paddingBottom: "0.25em" },
  ".cm-line.cm-h2": { fontSize: "1.35em", paddingTop: "0.45em", paddingBottom: "0.2em" },
  ".cm-line.cm-h3": { fontSize: "1.15em", paddingTop: "0.35em", paddingBottom: "0.15em" },
  ".cm-line.cm-h4": { fontSize: "1em", paddingTop: "0.25em", paddingBottom: "0.1em" },
  ".cm-line.cm-h5": { fontSize: "0.95em" },
  ".cm-line.cm-h6": { fontSize: "0.9em", color: muted },
};
