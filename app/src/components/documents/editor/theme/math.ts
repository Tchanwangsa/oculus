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
  // The empty-line hint (`FieldController.syncHint`): in flow after an
  // inline field, inside its tint; in a block centred, at the line's `top`.
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
  ".cm-math-field-block .cm-math-hint": {
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
  // On a block's empty line the caret is drawn just before the centred
  // hint; MathLive's own would sit under it.
  ".cm-math-field.cm-math-field-empty math-field": { "--caret-color": "transparent" },
  ".cm-math-field-block .cm-math-hint::before": {
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
  // Until it renders, the field lies unseen over its static stand-in, still
  // focusable (`FieldController.mount`).
  ".cm-math-field-mounting": { position: "relative" },
  ".cm-math-field-mounting math-field": { position: "absolute", inset: "0", opacity: "0" },
};
