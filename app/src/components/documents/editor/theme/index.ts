import { EditorView } from "@codemirror/view";

import { base } from "./base";
import { code } from "./code";
import { completion } from "./completion";
import { headings } from "./headings";
import { inline } from "./inline";
import { lists } from "./lists";
import { math } from "./math";
import { pictures } from "./pictures";
import { popoverTooltips } from "./popover-tooltips";
import { properties } from "./properties";
import { quotesRules } from "./quotes-rules";
import { tables } from "./tables";

export { noteHighlight } from "./highlight";

/**
 * The note editor's look, all from the app's tokens in `index.css` so dark
 * mode follows the `.dark` class. The editor grows with its text and the page
 * scrolls, so `.cm-scroller` is not a scroller. Classes named `cm-h1`,
 * `cm-quote`, … come from Live mode (`live-preview/`); the highlight styles
 * below colour the raw source in both modes — one for markdown, one for the
 * languages nested in fenced code (`codeLanguages.ts`).
 */
export const noteTheme = EditorView.theme({
  ...base,
  ...headings,
  ...inline,
  ...lists,
  ...quotesRules,
  ...code,
  ...pictures,
  ...math,
  ...popoverTooltips,
  ...completion,
  ...tables,
  ...properties,
});
