import { EditorView } from "@codemirror/view";

const tint = (token: string, pct: number) => `color-mix(in srgb, var(--color-${token}) ${pct}%, transparent)`;

/** The bar beside a changed line or block, in the margin left of the text. */
const bar = (color: string) => ({
  content: '""',
  position: "absolute",
  left: "-14px",
  top: "2px",
  bottom: "2px",
  width: "2px",
  borderRadius: "1px",
  backgroundColor: color,
  pointerEvents: "none",
});

/** Two names for one flash, alternated so each move restarts it. */
const flash = {
  "0%": { boxShadow: `0 0 0 3px ${tint("brand", 45)}` },
  "100%": { boxShadow: `0 0 0 3px ${tint("brand", 0)}` },
};

/** Change marks in the app's tokens; `cm-diff-current` is the hunk last
 *  navigated to, flashed once and held a shade stronger. */
export const diffTheme = EditorView.theme({
  ".cm-diff-ins": {
    backgroundColor: tint("success", 16),
    textDecoration: `underline ${tint("success", 55)}`,
    textDecorationThickness: "1px",
    textUnderlineOffset: "3px",
    borderRadius: "2px",
  },
  ".cm-diff-del": {
    color: tint("destructive", 85),
    textDecoration: "line-through",
    textDecorationColor: tint("destructive", 70),
    backgroundColor: tint("destructive", 8),
    borderRadius: "2px",
    opacity: "0.85",
  },
  ".cm-diff-del-block": {
    position: "relative",
    margin: "2px 0",
    paddingLeft: "10px",
    borderLeft: `2px solid ${tint("destructive", 70)}`,
    whiteSpace: "pre-wrap",
    fontFamily: "var(--font-sans)",
    color: tint("destructive", 85),
    opacity: "0.85",
  },
  ".cm-diff-del-block del": { textDecorationColor: tint("destructive", 70) },
  // An inline construct (maths, a picture, a chip) a change fell in.
  ".cm-diff-mod": {
    backgroundColor: tint("warning", 14),
    boxShadow: `0 0 0 1px ${tint("warning", 45)}`,
    borderRadius: "3px",
  },
  ".cm-diff-block": { position: "relative", borderRadius: "6px" },
  ".cm-diff-block.cm-diff-ins": { textDecoration: "none", backgroundColor: tint("success", 8) },
  ".cm-diff-block.cm-diff-mod": { boxShadow: "none", backgroundColor: tint("warning", 8) },
  ".cm-diff-block.cm-diff-ins::before": bar("var(--color-success)"),
  ".cm-diff-block.cm-diff-mod::before": bar("var(--color-warning)"),
  ".cm-line.cm-diff-line": { position: "relative" },
  ".cm-diff-line-ins::before": bar("var(--color-success)"),
  ".cm-diff-line-del::before": bar("var(--color-destructive)"),
  ".cm-diff-line-mix::before": bar("var(--color-warning)"),
  ".cm-diff-current-line::before": { width: "3px", left: "-14.5px" },
  ".cm-diff-ins.cm-diff-current": { backgroundColor: tint("success", 30) },
  ".cm-diff-mod.cm-diff-current": { backgroundColor: tint("warning", 26) },
  ".cm-diff-del.cm-diff-current": { backgroundColor: tint("destructive", 18) },
  ".cm-diff-del-block.cm-diff-current": { backgroundColor: tint("destructive", 8) },
  ".cm-diff-flash-0": { animation: "cm-diff-flash-0 900ms ease-out" },
  ".cm-diff-flash-1": { animation: "cm-diff-flash-1 900ms ease-out" },
  "@keyframes cm-diff-flash-0": flash,
  "@keyframes cm-diff-flash-1": flash,
});
