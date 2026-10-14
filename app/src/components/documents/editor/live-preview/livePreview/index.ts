import type { Extension } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

import { focusedField, trackFocus } from "../../core/liveFocus";
import { selectionGaps, type SelectionGap } from "../../core/selectionLayer";
import { mathField, visualMathField } from "../../math/field/mathField";
import { MathWidget, mathsWatcher } from "../widgets";
import { blockField } from "./blocks";
import { dollarTyping, edgePaste, edgeTyping, fieldLatexPaste } from "./edges";
import { inlinePlugin } from "./inline";
import { mathBlockField } from "./math-blocks";
import { tableField, tableKeys } from "./tables";

/**
 * Live mode: markdown renders in place and a construct shows its source while
 * the selection touches it — headings, quotes, list markers and code fences
 * while the caret is on their lines. Unfocused, nothing is revealed. A table
 * never is: it is atomic, and its cells take the caret instead. Rendered
 * maths is atomic too, and the visual field (`math/field/mathField`) edits it.
 *
 * An inline code span holding only a citation draws as the chat's file chip
 * (`mentionSyntax.ts`).
 *
 * Inline decorations come from a view plugin over the viewport; block widgets
 * (display maths, a picture alone on its line, a mermaid diagram, a rule, a
 * table, frontmatter properties) from a state field,
 * because CodeMirror rejects block decorations from a plugin. Both rebuild on
 * selection and focus changes, not only edits.
 */

export { syncLiveFocus } from "../../core/liveFocus";

/** Display maths draws structural bands of its own, so the text highlight
 *  skips the block and its line break. Inline maths shares the text layer. */
function renderedMaths(view: EditorView): SelectionGap[] {
  const out: SelectionGap[] = [];
  const { length } = view.state.doc;
  const drawn = (w: unknown) => w instanceof MathWidget && w.rendered();
  for (let it = view.state.field(mathBlockField).iter(); it.value; it.next()) {
    if (drawn(it.value.spec.widget)) out.push({ from: it.from, to: Math.min(it.to + 1, length) });
  }
  return out;
}

/** Live mode's drawing alone, with no way to edit: what a read-only view
 *  of a note (a saved version) carries. Without `trackFocus` it never
 *  counts as focused, so nothing but a find match reveals source.
 *  `visualMathField` stays because the fields here read it. */
export function livePreviewRender(): Extension {
  return [
    focusedField,
    visualMathField,
    blockField,
    tableField,
    mathBlockField,
    inlinePlugin,
    mathsWatcher,
    selectionGaps.of(renderedMaths),
  ];
}

/** Everything Live mode adds over Raw; swapped through a compartment. Each
 *  facet sees its providers in the same order as when these were one list. */
export function livePreview(): Extension {
  return [
    livePreviewRender(),
    trackFocus,
    mathField(),
    tableKeys,
    dollarTyping,
    edgeTyping,
    edgePaste,
    fieldLatexPaste,
  ];
}
