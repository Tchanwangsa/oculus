import { HighlightStyle, languageDataProp, syntaxHighlighting } from "@codemirror/language";
import type { Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { tags } from "@lezer/highlight";

import { noteLanguage } from "./language";
import { MATH_LINE_GAP } from "./mathField";

/**
 * The note editor's look, all from the app's tokens in `index.css` so dark
 * mode follows the `.dark` class. The editor grows with its text and the page
 * scrolls, so `.cm-scroller` is not a scroller. Classes named `cm-h1`,
 * `cm-quote`, … come from Live mode (`livePreview.ts`); the highlight styles
 * below colour the raw source in both modes — one for markdown, one for the
 * languages nested in fenced code (`codeLanguages.ts`).
 */

const brand = "var(--color-brand)";
const muted = "var(--color-muted-foreground)";
const mono = "var(--font-mono)";

export const noteTheme = EditorView.theme({
  "&": {
    color: "var(--color-foreground)",
    backgroundColor: "transparent",
    fontSize: "14px",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": {
    overflow: "visible",
    height: "auto",
    fontFamily: "var(--font-sans)",
    lineHeight: "1.7",
  },
  ".cm-content": {
    padding: "0",
    minHeight: "50vh",
    caretColor: brand,
  },
  ".cm-line": { padding: "0" },
  ".cm-cursor, .cm-dropCursor": { borderLeft: `1.5px solid ${brand}` },
  // Matches the base theme's specificity, which otherwise wins.
  "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection":
    { backgroundColor: `color-mix(in srgb, ${brand} 22%, transparent)` },
  ".cm-placeholder": { color: `color-mix(in srgb, ${muted} 40%, transparent)` },

  // Headings
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

  // Inline marks. Bold in a heading outweighs the heading, including the
  // highlight spans nested in it.
  ".cm-strong": { fontWeight: "600" },
  ".cm-h1 .cm-strong, .cm-h2 .cm-strong, .cm-h3 .cm-strong, .cm-h4 .cm-strong, .cm-h5 .cm-strong, .cm-h6 .cm-strong, .cm-h1 .cm-strong *, .cm-h2 .cm-strong *, .cm-h3 .cm-strong *, .cm-h4 .cm-strong *, .cm-h5 .cm-strong *, .cm-h6 .cm-strong *":
    { fontWeight: "800" },
  ".cm-em": { fontStyle: "italic" },
  ".cm-strike": { textDecoration: "line-through" },
  ".cm-inline-code": {
    fontFamily: mono,
    fontSize: "0.9em",
    backgroundColor: "var(--color-surface-raised)",
    borderRadius: "4px",
    padding: "0.1em 0.3em",
  },
  ".cm-link": {
    color: brand,
    textDecoration: "underline",
    textDecorationColor: `color-mix(in srgb, ${brand} 35%, transparent)`,
    textUnderlineOffset: "2px",
  },

  // Lists and tasks
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

  // Quotes and rules
  ".cm-line.cm-quote": {
    borderLeft: "2px solid var(--color-border)",
    paddingLeft: "1em",
    color: muted,
  },
  ".cm-hr": { display: "flex", alignItems: "center", height: "1.7em", cursor: "text" },
  ".cm-hr::before": { content: '""', flex: "1", borderTop: "1px solid var(--color-border)" },
  ".cm-line.cm-hr-gap": { lineHeight: "0.6" },

  // Fenced code
  ".cm-line.cm-codeblock": {
    fontFamily: mono,
    fontSize: "13px",
    backgroundColor: "var(--color-surface-raised)",
    padding: "0 12px",
  },
  // The gap around a block is a transparent border the fill stops short of:
  // lines can't take margins, and the vertical radius grows by its width.
  ".cm-line.cm-codeblock-first": {
    position: "relative",
    borderTop: "10px solid transparent",
    backgroundClip: "padding-box",
    borderTopLeftRadius: "8px 18px",
    borderTopRightRadius: "8px 18px",
    paddingTop: "8px",
  },
  ".cm-line.cm-codeblock-last": {
    borderBottom: "10px solid transparent",
    backgroundClip: "padding-box",
    borderBottomLeftRadius: "8px 18px",
    borderBottomRightRadius: "8px 18px",
    paddingBottom: "8px",
  },
  ".cm-code-info": { fontFamily: "var(--font-sans)", fontSize: "11px", color: muted },
  ".cm-code-text": { fontFamily: mono, fontSize: "0.92em" },
  // Live mode's header over a hidden opening fence. The copy button sits at
  // the first line's right edge whatever indent or quote precedes the fence.
  ".cm-code-label": {
    fontFamily: "var(--font-sans)",
    fontSize: "11px",
    color: muted,
    cursor: "pointer",
  },
  ".cm-code-label:hover": { color: "var(--color-foreground)" },
  ".cm-code-copy": {
    position: "absolute",
    top: "8px",
    right: "6px",
    border: "0",
    padding: "0 8px",
    borderRadius: "9999px",
    backgroundColor: "transparent",
    color: muted,
    fontFamily: "var(--font-sans)",
    fontSize: "11px",
    lineHeight: "20px",
    cursor: "pointer",
    transition: "background-color 120ms, color 120ms",
  },
  // `accent` is the code block's own fill, so the hover mixes from the text.
  ".cm-code-copy:hover": {
    backgroundColor: `color-mix(in srgb, ${muted} 16%, transparent)`,
    color: "var(--color-foreground)",
  },
  // A hidden closing fence keeps only the block's bottom padding.
  ".cm-line.cm-codeblock-close-hidden": { fontSize: "0", lineHeight: "0" },

  // Pictures
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

  // Maths
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
  ".cm-math-display .katex-html > .newline": { height: `${MATH_LINE_GAP}em` },
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
  // The MathLive field (`mathField.ts`) at the static rendering's size and
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
    "--selection-color": "inherit",
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

  // Mermaid diagrams; the picture's box is `.diagram` (`index.css`).
  ".cm-mermaid": { padding: "6px 0", cursor: "text" },
  ".cm-mermaid-editing:empty": { padding: "0" },
  ".cm-mermaid-source": { margin: "0", color: muted, fontFamily: mono, fontSize: "13px", whiteSpace: "pre-wrap" },
  ".cm-mermaid-error .cm-mermaid-source": { color: "var(--color-destructive)" },
  "&.cm-editor .cm-content .cm-snippetField": {
    backgroundColor: `color-mix(in srgb, ${brand} 14%, transparent)`,
    borderRadius: "3px",
  },

  // Tooltips (the maths popover, the completion list). `&.cm-editor` beats
  // the base theme's light-mode tooltip rules.
  "&.cm-editor .cm-tooltip.cm-math-tools, &.cm-editor .cm-tooltip.cm-tooltip-autocomplete": {
    backgroundColor: "var(--color-popover)",
    color: "var(--color-popover-foreground)",
    border: "1px solid var(--color-border)",
    borderRadius: "12px",
    boxShadow: "var(--shadow-lg)",
    fontFamily: "var(--font-sans)",
  },

  // Maths popover (`mathTools.ts`)
  ".cm-math-tools": {
    width: "352px",
    maxWidth: "calc(100vw - 16px)",
    padding: "8px",
    overflowX: "hidden",
    overflowY: "auto",
    fontSize: "12px",
    lineHeight: "1.4",
  },
  ".cm-math-tools.cm-math-tools-hidden, &.cm-math-command .cm-math-tools": { display: "none" },
  // Under the MathLive field: no preview, the field is the preview.
  ".cm-math-tools-visual .cm-math-preview, .cm-math-tools-visual .cm-math-error-text": { display: "none" },
  ".cm-math-tools-visual .cm-math-recents": { marginTop: "0" },
  ".cm-math-tools-visual .cm-math-recents[hidden] + .cm-math-tabbar": {
    marginTop: "0",
    paddingTop: "0",
    borderTop: "0",
  },
  ".cm-math-preview": {
    minHeight: "36px",
    maxHeight: "160px",
    overflow: "auto",
    padding: "6px 8px",
    borderRadius: "8px",
    backgroundColor: "var(--color-surface)",
    display: "flex",
    alignItems: "center",
    fontSize: "14px",
  },
  // Auto margins centre it and keep a wide formula's start reachable.
  ".cm-math-preview > *": { margin: "auto", flex: "none" },
  ".cm-math-preview-stale": { opacity: "0.45" },
  ".cm-math-preview-empty": { color: muted, fontSize: "12px" },
  ".cm-math-error-text": {
    color: "var(--color-destructive)",
    fontSize: "11px",
    marginTop: "4px",
    overflowWrap: "anywhere",
  },
  ".cm-math-error-text:empty": { display: "none" },
  ".cm-math-matrix-kinds": { display: "flex", flexWrap: "wrap", gap: "2px" },
  ".cm-math-tabbar": {
    display: "flex",
    alignItems: "center",
    gap: "4px",
    marginTop: "6px",
    paddingTop: "6px",
    borderTop: "1px solid var(--color-border-subtle)",
  },
  // One row that scrolls sideways; faded by `syncScrollFade` (`index.css`).
  ".cm-math-tabs": {
    flex: "1",
    minWidth: "0",
    display: "flex",
    gap: "2px",
    overflowX: "auto",
    scrollbarWidth: "none",
  },
  ".cm-math-tabs::-webkit-scrollbar": { display: "none" },
  ".cm-math-tabs .cm-math-pill": { flex: "none" },
  ".cm-math-pill": {
    border: "0",
    padding: "0 9px",
    borderRadius: "9999px",
    backgroundColor: "transparent",
    color: muted,
    fontFamily: "var(--font-sans)",
    fontSize: "12px",
    lineHeight: "22px",
    cursor: "pointer",
    whiteSpace: "nowrap",
    transition: "background-color 120ms, color 120ms",
  },
  ".cm-math-mode, .cm-math-shape": { flex: "none", color: "var(--color-foreground)", fontWeight: "500" },
  ".cm-math-mode[hidden], .cm-math-shape[hidden]": { display: "none" },
  ".cm-math-close": {
    flex: "none",
    width: "22px",
    height: "22px",
    display: "grid",
    placeItems: "center",
    padding: "0",
    border: "0",
    borderRadius: "9999px",
    backgroundColor: "transparent",
    color: muted,
    cursor: "pointer",
    transition: "background-color 120ms, color 120ms",
  },
  ".cm-math-close:hover": { backgroundColor: "var(--color-accent)", color: "var(--color-foreground)" },
  ".cm-math-pill:hover": { backgroundColor: "var(--color-accent)", color: "var(--color-foreground)" },
  ".cm-math-pill[aria-pressed=true]": { backgroundColor: "var(--color-accent)", color: "var(--color-foreground)" },
  ".cm-math-recents": {
    display: "flex",
    alignItems: "center",
    gap: "2px",
    marginTop: "6px",
  },
  ".cm-math-recents[hidden]": { display: "none" },
  ".cm-math-caption": { flex: "none", color: muted, fontSize: "11px", marginRight: "4px" },
  // One row that scrolls sideways, like the tab strip.
  ".cm-math-recent-cells": {
    flex: "1",
    minWidth: "0",
    display: "flex",
    alignItems: "center",
    gap: "2px",
    overflowX: "auto",
    scrollbarWidth: "none",
  },
  ".cm-math-recent-cells::-webkit-scrollbar": { display: "none" },
  ".cm-math-recents .cm-math-cell": { flex: "none", width: "36px" },
  ".cm-math-recents .cm-math-cell-wide": { width: "74px" },
  // Three rows of cells (32px + 2px gaps); the rest scrolls.
  // Faded top and bottom like the tab strip.
  ".cm-math-body": {
    marginTop: "6px",
    maxHeight: "100px",
    overflowY: "auto",
    overflowX: "hidden",
  },
  ".cm-math-grid": {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(36px, 1fr))",
    gridAutoFlow: "row dense",
    gap: "2px",
  },
  // Rows grow for a two-line preview (cases, matrices).
  ".cm-math-cell": {
    minHeight: "32px",
    minWidth: "0",
    border: "0",
    padding: "4px 2px",
    borderRadius: "6px",
    backgroundColor: "transparent",
    color: "var(--color-foreground)",
    fontSize: "11px",
    overflow: "hidden",
    cursor: "pointer",
    transition: "background-color 120ms",
  },
  ".cm-math-cell-wide": { gridColumn: "span 2" },
  ".cm-math-cell:hover": { backgroundColor: "var(--color-accent)" },
  ".cm-math-cell .katex": { cursor: "pointer" },
  // The visual field's quick picks: one row of palette cells, each with its
  // number key in the corner, and the expand button.
  // After `.cm-math-tools`, which it overrides; `-hidden` still beats it.
  ".cm-math-quick": {
    width: "auto",
    display: "flex",
    alignItems: "center",
    gap: "2px",
    padding: "4px",
  },
  ".cm-math-quick .cm-math-cell": { position: "relative", flex: "none", width: "36px" },
  ".cm-math-quick .cm-math-cell-wide": { width: "74px" },
  ".cm-math-quick-key": {
    position: "absolute",
    top: "1px",
    left: "3px",
    color: muted,
    fontFamily: "var(--font-sans)",
    fontSize: "9px",
    lineHeight: "1",
    fontVariantNumeric: "tabular-nums",
    pointerEvents: "none",
  },
  ".cm-math-quick .cm-math-close": { marginLeft: "2px" },
  ".cm-math-matrix": { display: "flex", alignItems: "flex-start", gap: "12px", marginBottom: "8px" },
  ".cm-math-matrix-grid": { display: "grid", gridTemplateColumns: "repeat(6, 14px)", gap: "2px" },
  ".cm-math-matrix-cell": {
    width: "14px",
    height: "14px",
    padding: "0",
    border: "1px solid var(--color-border)",
    borderRadius: "3px",
    backgroundColor: "var(--color-card)",
    cursor: "pointer",
  },
  ".cm-math-matrix-cell.cm-math-matrix-lit": {
    borderColor: brand,
    backgroundColor: "var(--color-brand-muted)",
  },
  ".cm-math-matrix-side": { display: "flex", flexDirection: "column", gap: "6px" },
  ".cm-math-matrix-size": { color: muted, fontSize: "11px", fontVariantNumeric: "tabular-nums" },

  // Completion list (`autocompletion()` in `extensions.ts`)
  "&.cm-editor .cm-tooltip.cm-tooltip-autocomplete > ul": {
    fontFamily: "var(--font-sans)",
    fontSize: "12px",
    minWidth: "220px",
    maxHeight: "16em",
    padding: "4px",
  },
  "&.cm-editor .cm-tooltip.cm-tooltip-autocomplete > ul > li": {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    padding: "2px 8px 2px 4px",
    borderRadius: "6px",
    lineHeight: "24px",
  },
  "&.cm-editor .cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]": {
    backgroundColor: "var(--color-accent)",
    color: "var(--color-foreground)",
  },
  ".cm-completionLabel": { fontFamily: mono, fontSize: "12px" },
  ".cm-completionMatchedText": { textDecoration: "none", color: brand },
  ".cm-completionDetail": {
    marginLeft: "auto",
    paddingLeft: "12px",
    fontStyle: "normal",
    fontFamily: mono,
    fontSize: "11px",
    color: muted,
  },
  ".cm-math-option-preview": {
    flex: "none",
    width: "44px",
    height: "24px",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    overflow: "hidden",
    fontSize: "10px",
  },
  // `@` file mentions (`mentions.ts`): a sans title, a muted folder and glyph.
  ".cm-mention-option": { maxWidth: "420px" },
  ".cm-mention-option .cm-completionLabel": {
    fontFamily: "var(--font-sans)",
    minWidth: "0",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  ".cm-mention-option .cm-completionDetail": { fontFamily: "var(--font-sans)", whiteSpace: "nowrap" },
  ".cm-mention-icon": { flex: "none", display: "flex", color: muted },

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

  // Frontmatter properties
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
});

/** Markdown source colouring: markers muted, inline code and LaTeX
 *  monospace. Scoped to the note grammar, so it never reaches nested code. */
const markdownHighlight = syntaxHighlighting(
  HighlightStyle.define(
    [
      // Strong before heading, so a heading's own weight wins over bold in it.
      { tag: tags.strong, fontWeight: "600" },
      { tag: tags.heading, fontWeight: "650" },
      { tag: tags.emphasis, fontStyle: "italic" },
      { tag: tags.strikethrough, textDecoration: "line-through" },
      { tag: tags.link, color: brand },
      { tag: tags.url, color: muted },
      { tag: tags.monospace, fontFamily: mono, fontSize: "0.92em" },
      { tag: tags.special(tags.content), fontFamily: mono, fontSize: "0.92em" },
      { tag: [tags.processingInstruction, tags.contentSeparator, tags.atom, tags.labelName], color: muted },
      { tag: tags.quote, color: muted },
    ],
    { scope: noteLanguage },
  ),
);

const codeStyle = HighlightStyle.define([
  { tag: [tags.keyword, tags.tagName, tags.deleted], color: "var(--color-syntax-keyword)" },
  {
    tag: [tags.string, tags.regexp, tags.character, tags.attributeValue, tags.inserted],
    color: "var(--color-syntax-string)",
  },
  { tag: tags.comment, color: "var(--color-syntax-comment)" },
  {
    tag: [tags.number, tags.bool, tags.null, tags.atom, tags.unit, tags.escape],
    color: "var(--color-syntax-number)",
  },
  {
    tag: [tags.function(tags.variableName), tags.function(tags.propertyName), tags.macroName],
    color: "var(--color-syntax-function)",
  },
  { tag: [tags.typeName, tags.className, tags.namespace], color: "var(--color-syntax-type)" },
  { tag: [tags.propertyName, tags.attributeName], color: "var(--color-syntax-property)" },
  { tag: tags.operator, color: "var(--color-syntax-operator)" },
  { tag: [tags.meta, tags.processingInstruction, tags.annotation], color: "var(--color-syntax-meta)" },
  { tag: tags.heading, fontWeight: "600" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strong, fontWeight: "600" },
]);

/** Fenced code's colouring, for every grammar except the note's own. */
const codeHighlight: Extension = [
  syntaxHighlighting({
    style: (t) => codeStyle.style(t),
    scope: (type) => type.prop(languageDataProp) !== noteLanguage.data,
  }),
  // A bare highlighter brings no CSS; this is the style's own module.
  EditorView.styleModule.of(codeStyle.module!),
];

export const noteHighlight: Extension = [markdownHighlight, codeHighlight];
