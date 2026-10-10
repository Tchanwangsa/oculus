import { MATH_LINE_GAP } from "../math/field/mathField";
import { brand, muted, mono, type ThemeSpec } from "./tokens";

export const math: ThemeSpec = {
  ".cm-math": { cursor: "text" },
  // `contain` keeps a too-wide formula scrolling in its block rather than
  // widening the whole note, as `.cm-math-view-block` does for the field.
  ".cm-math-display": { textAlign: "center", padding: "0.4em 0", overflowX: "auto", contain: "inline-size" },
  ".cm-math-display .katex-display": { margin: "0" },
  // A block's top-level `\\` lines, spaced as the field spaces its rows.
  ".cm-math-display .katex-html > .katex-newline": { height: `${MATH_LINE_GAP}em` },
  // Rendered maths selects as one unit (`MathWidget`): no native highlight on
  // KaTeX's glyphs, which CodeMirror hides only inside a line. A selected
  // block fills opaquely over the card, so the selection layer under it
  // can't double its tint.
  ".cm-content .cm-math::selection, .cm-content .cm-math ::selection": { backgroundColor: "transparent" },
  ".cm-math-display.cm-math-selected": {
    backgroundColor: `color-mix(in srgb, ${brand} 22%, var(--color-card))`,
    borderRadius: "6px",
  },
  ".cm-math-error": { color: "var(--color-destructive)", fontFamily: mono, fontSize: "13px" },
  // Its source while the maths engine loads (`MathWidget`).
  ".cm-math-pending": { color: muted, fontFamily: mono, fontSize: "13px", whiteSpace: "pre-wrap" },
  // The empty-line hint (`syncHint`, `math/field/rustField/hint.ts`): in flow
  // after an inline field, inside its tint; in a block centred, at the
  // line's `top`.
  ".cm-math-hint": {
    padding: "0 2px",
    borderRadius: "4px",
    backgroundColor: `color-mix(in srgb, ${brand} 7%, transparent)`,
    whiteSpace: "nowrap",
    color: muted,
    pointerEvents: "none",
    userSelect: "none",
    WebkitUserSelect: "none",
  },
  ".cm-math-view-block .cm-math-hint": {
    position: "absolute",
    left: "50%",
    transform: "translate(-50%, -50%)",
    maxWidth: "90%",
    // One tight line, so the box is the text and centring it on the empty
    // row keeps it inside the field.
    fontSize: "12px",
    lineHeight: "1.2",
    padding: "0",
    backgroundColor: "transparent",
    overflow: "hidden",
    textOverflow: "ellipsis",
  },
  // An inline field holds the hint inside its tint, at the note's text size.
  ".cm-math-view:not(.cm-math-view-block) > .cm-math-hint": {
    fontSize: "calc(1em / 1.21)",
    backgroundColor: "transparent",
  },
  // On a block's empty line the caret is drawn just before the centred
  // hint; the field's own would sit under it.
  ".cm-math-view-empty .cm-math-view-caret": { display: "none" },
  ".cm-math-view-block .cm-math-hint::before": {
    content: '""',
    display: "inline-block",
    width: "0",
    height: "1.15em",
    marginRight: "2px",
    verticalAlign: "text-bottom",
    borderLeft: `1.5px solid ${brand}`,
    animation: "cm-math-blink 1.2s steps(1) infinite",
  },
  "@keyframes cm-math-blink": { "50%": { visibility: "hidden" } },
  ".cm-math-hint[hidden]": { display: "none" },
  // The field (`math/field/mathView`): KaTeX's rendering at its own size in
  // a tint, one line box tall, so opening it doesn't move the maths or its
  // line. The margin takes back the padding's advance: the tint reaches 2px
  // past the maths without moving the text beside it.
  ".cm-math-view": {
    display: "inline-block",
    position: "relative",
    verticalAlign: "baseline",
    fontSize: "1.21em",
    lineHeight: "1.2",
    padding: "0 2px",
    margin: "0 -2px",
    borderRadius: "4px",
    backgroundColor: `color-mix(in srgb, ${brand} 7%, transparent)`,
    cursor: "text",
  },
  ".cm-math-view .katex": { fontSize: "1em" },
  // A block spans the column, its maths centred and scrolling sideways when
  // too wide, as `.cm-math-display` draws it, with its padding at the note's
  // font size (the view's is 1.21×); `contain` stops a too-wide formula
  // widening the whole note.
  ".cm-math-view-block": { display: "block", padding: "calc(0.4em / 1.21) 0", margin: "0", contain: "inline-size" },
  ".cm-math-view-block .katex-display": { margin: "0" },
  ".cm-math-view-block .katex-html > .katex-newline": { height: `${MATH_LINE_GAP}em` },
  // The overlays' frame, stacking them over the bands but under nothing else.
  ".cm-math-view-frame": { position: "relative", display: "inline-block", isolation: "isolate" },
  ".cm-math-view-block .cm-math-view-frame": { display: "block", overflowX: "auto", overflowY: "hidden" },
  ".cm-math-view-bands": { position: "absolute", inset: "0", zIndex: "-1", pointerEvents: "none" },
  ".cm-math-view-band": {
    position: "absolute",
    borderRadius: "2px",
    backgroundColor: `color-mix(in srgb, ${brand} 22%, transparent)`,
  },
  ".cm-math-view-caret": {
    position: "absolute",
    width: "0",
    marginLeft: "-0.75px",
    borderLeft: `1.5px solid ${brand}`,
    pointerEvents: "none",
    visibility: "hidden",
  },
  ".cm-math-view-focused .cm-math-view-caret": { visibility: "visible" },
  ".cm-math-view-focused .cm-math-view-caret.cm-math-view-blink": { animation: "cm-math-blink 1.2s steps(1) infinite" },
  ".cm-math-view-caret[hidden]": { display: "none" },
  // Focused, empty and invisible at the caret, where the IME's window opens.
  ".cm-math-view-input": {
    position: "absolute",
    width: "1px",
    padding: "0",
    border: "0",
    margin: "0",
    opacity: "0",
    resize: "none",
    overflow: "hidden",
    whiteSpace: "pre",
    fontSize: "inherit",
    caretColor: "transparent",
    outline: "none",
    pointerEvents: "none",
  },
  // An IME's text while it composes, over the maths at the caret.
  ".cm-math-view-preedit": {
    position: "absolute",
    whiteSpace: "pre",
    fontFamily: "KaTeX_Main",
    backgroundColor: "var(--color-card)",
    textDecoration: "underline",
    textDecorationColor: brand,
    pointerEvents: "none",
  },
  ".cm-math-view-preedit[hidden]": { display: "none" },
  ".cm-math-view .oc-placeholder": { color: brand, opacity: "0.5" },
  // Zero-size on the baseline, where the caret's geometry reads it
  // (`lib/maths/geometry/layout.ts`).
  ".cm-math-view .oc-empty-row": { display: "inline-block", width: "0", height: "0" },
  // The `\name` being typed in command mode, typed into the rendering as
  // the TeX source it is (`mathView/pending.ts`).
  ".cm-math-view .katex [data-pending]": { fontFamily: mono, color: brand },
  // The palette's options for the pending `\command`, in the popover look:
  // a box around a scroller of rows (`mathView/popover/`).
  ".cm-math-view-popover": {
    position: "absolute",
    zIndex: "20",
    display: "flex",
    flexDirection: "column",
    minWidth: "160px",
    padding: "4px",
    backgroundColor: "var(--color-popover)",
    color: "var(--color-popover-foreground)",
    border: "1px solid var(--color-border)",
    borderRadius: "12px",
    boxShadow: "var(--shadow-lg)",
    fontFamily: "var(--font-sans)",
    fontSize: "12px",
    lineHeight: "1.5",
    textAlign: "left",
    whiteSpace: "nowrap",
    cursor: "default",
    userSelect: "none",
    WebkitUserSelect: "none",
  },
  ".cm-math-view-popover[hidden]": { display: "none" },
  ".cm-math-view-popover-list": {
    display: "flex",
    flexDirection: "column",
    maxHeight: "232px",
    overflowY: "auto",
  },
  ".cm-math-view-popover-row": {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    padding: "2px 8px",
    borderRadius: "8px",
  },
  ".cm-math-view-popover-row[aria-selected=true]": {
    backgroundColor: "var(--color-accent)",
    color: "var(--color-foreground)",
  },
  ".cm-math-view-popover-preview": { minWidth: "28px", fontSize: "13px", textAlign: "center" },
  ".cm-math-view-popover-name": { fontFamily: mono, color: muted },
};
