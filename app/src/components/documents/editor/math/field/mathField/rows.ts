import type { MathfieldElement } from "mathlive";

let rowSheet: CSSStyleSheet | null = null;

/** Centres `\displaylines` rows, as KaTeX centres a display block's bare
 *  `\\` lines: MathLive left-aligns its root `lines` table (the one root
 *  table with a lone left column), in the shadow root `::part` can't reach. */
export function centredRows(mf: MathfieldElement) {
  const root = mf.shadowRoot;
  if (!root || !("adoptedStyleSheets" in root)) return;
  if (!rowSheet) {
    rowSheet = new CSSStyleSheet();
    rowSheet.replaceSync(
      ".ML__latex > .ML__multiline_environment { justify-content: safe center; }\n" +
        ".ML__latex > .ML__mtable > .col-align-l:only-child > .ML__vlist-t { text-align: center; }",
    );
  }
  root.adoptedStyleSheets = [...root.adoptedStyleSheets, rowSheet];
}
