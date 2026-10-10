import { MATH_LINE_GAP } from "../math/field/mathField";
import { brand, muted, mono, type ThemeSpec } from "./tokens";

export const math: ThemeSpec = {
  ".cm-math": { cursor: "text" },
  // `contain` keeps a too-wide formula scrolling in its block rather than
  // widening the whole note, as `.cm-math-field-block` does for the field.
  ".cm-math-display": { textAlign: "center", padding: "0.4em 0", overflowX: "auto", contain: "inline-size" },
  // MathLive's static rendering (`staticMath`) in the box the field takes:
  // its size, padding and, in a block, a `\displaylines` table spanning the
  // column with its rows centred, as `centredRows` draws them in the field.
  ".cm-math-ml": {
    display: "inline-block",
    verticalAlign: "baseline",
    fontSize: "1.21em",
    lineHeight: "1.2",
    padding: "0 2px",
  },
  ".cm-math-display > .cm-math-ml": { display: "block" },
  // `\text{}` in LaTeX's roman, as KaTeX sets it (MathLive defaults to the
  // system sans). The field draws text one span per letter, so no kerning or
  // ligatures here either, or entering the maths shifts it.
  ".cm-math-ml .ML__text": { fontFamily: "KaTeX_Main", fontKerning: "none", fontVariantLigatures: "none" },
  ".cm-math-ml .ML__latex > .ML__base:has(> .ML__multiline_environment)": { width: "100%" },
  ".cm-math-ml .ML__latex > .ML__base > .ML__multiline_environment": { justifyContent: "safe center" },
  ".cm-math-ml .ML__latex > .ML__base > .ML__mtable > .col-align-l:only-child > .ML__vlist-t": {
    textAlign: "center",
  },
  // KaTeX, while MathLive loads or for maths it can't read.
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
  // The MathLive field (`math/field/mathField`) at the static rendering's size and
  // line height, so opening it doesn't move the maths or its line. Rules
  // from out here beat its shadow `:host`.
  ".cm-math-field math-field": {
    display: "inline-block",
    verticalAlign: "baseline",
    fontSize: "1.21em",
    lineHeight: "1.2",
    color: "inherit",
    backgroundColor: `color-mix(in srgb, ${brand} 7%, transparent)`,
    border: "0",
    borderRadius: "4px",
    outline: "none",
    padding: "0 2px",
    "--caret-color": brand,
    "--selection-color": "var(--color-foreground)",
    "--selection-background-color": `color-mix(in srgb, ${brand} 22%, transparent)`,
    "--contains-highlight-background-color": `color-mix(in srgb, ${brand} 9%, transparent)`,
    "--placeholder-color": brand,
    "--smart-fence-color": muted,
    "--latex-color": brand,
    "--text-font-family": "KaTeX_Main",
  },
  ".cm-math-field math-field::part(container)": {
    minHeight: "0",
    padding: "0",
    "--_placeholder-color": brand,
    "--_placeholder-opacity": "0.5",
    "--_selection-color": "var(--color-foreground)",
    "--_selection-background-color": `color-mix(in srgb, ${brand} 22%, transparent)`,
    "--_contains-highlight-background-color": `color-mix(in srgb, ${brand} 9%, transparent)`,
    "--_latex-color": brand,
    "--_smart-fence-color": muted,
  },
  ".cm-math-field math-field::part(content)": { padding: "0" },
  // MathLive clips its content box, which cuts subscripts below the inline
  // field's tight line box; an inline field never needs to scroll.
  ".cm-math-field:not(.cm-math-field-block) math-field::part(content)": { overflow: "visible" },
  ".cm-math-field math-field::part(menu-toggle), .cm-math-field math-field::part(virtual-keyboard-toggle)": {
    display: "none",
  },
  // A block's field spans the column with the maths centred, as KaTeX draws
  // `.cm-math-display`; `safe` keeps a too-wide formula's start in view.
  // `contain` stops a too-wide formula widening the whole note.
  ".cm-math-field-block": { position: "relative", padding: "0.4em 0", contain: "inline-size" },
  ".cm-math-field-block math-field": { display: "block", width: "100%", boxSizing: "border-box" },
  ".cm-math-field-block math-field::part(content)": { justifyContent: "safe center" },
  // The empty-line hint (`syncHint`, in either field's folder): in flow
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
  ".cm-math-field-block .cm-math-hint, .cm-math-view-block .cm-math-hint": {
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
  // The Rust field holds the hint inside its tint, at the note's text size.
  ".cm-math-view:not(.cm-math-view-block) > .cm-math-hint": {
    fontSize: "calc(1em / 1.21)",
    backgroundColor: "transparent",
  },
  // On a block's empty line the caret is drawn just before the centred
  // hint; the field's own would sit under it.
  ".cm-math-field.cm-math-field-empty math-field": { "--caret-color": "transparent" },
  ".cm-math-view-empty .cm-math-view-caret": { display: "none" },
  ".cm-math-field-block .cm-math-hint::before, .cm-math-view-block .cm-math-hint::before": {
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
  // The Rust field (`math/field/mathView`): KaTeX's rendering at its own
  // size in the MathLive field's tint, one line box tall, so opening it
  // doesn't move the maths or its line.
  ".cm-math-view": {
    display: "inline-block",
    position: "relative",
    verticalAlign: "baseline",
    fontSize: "1.21em",
    lineHeight: "1.2",
    padding: "0 2px",
    borderRadius: "4px",
    backgroundColor: `color-mix(in srgb, ${brand} 7%, transparent)`,
    cursor: "text",
  },
  ".cm-math-view .katex": { fontSize: "1em" },
  // A block spans the column, its maths centred and scrolling sideways when
  // too wide, as `.cm-math-display` draws it; `contain` stops a too-wide
  // formula widening the whole note.
  ".cm-math-view-block": { display: "block", padding: "0.4em 0", contain: "inline-size" },
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
  ".cm-math-view .oc-empty-row": { display: "inline-block", width: "0", height: "1em", verticalAlign: "-0.25em" },
  // The pending `\command` and the palette's commands that start with it,
  // as MathLive's suggestion list was styled (`styles/math.css`).
  ".cm-math-view-popover": {
    position: "absolute",
    zIndex: "20",
    display: "flex",
    flexDirection: "column",
    minWidth: "160px",
    maxHeight: "240px",
    overflow: "hidden",
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
  ".cm-math-view-popover-typed": { padding: "2px 8px", fontFamily: mono, color: brand },
  ".cm-math-view-popover-row": {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    padding: "2px 8px",
    borderRadius: "8px",
  },
  ".cm-math-view-popover-preview": { minWidth: "28px", fontSize: "13px", textAlign: "center" },
  ".cm-math-view-popover-name": { fontFamily: mono, color: muted },
  // Until it renders, the field lies unseen over its static stand-in, still
  // focusable (`FieldController.mount`).
  ".cm-math-field-mounting": { position: "relative" },
  ".cm-math-field-mounting math-field": { position: "absolute", inset: "0", opacity: "0" },
};
