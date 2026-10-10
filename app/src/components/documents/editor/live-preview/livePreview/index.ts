import type { Extension } from "@codemirror/state";

import { focusedField, trackFocus } from "../../core/liveFocus";
import { mathField } from "../../math/field/mathField";
import { mathsWatcher } from "../widgets";
import { blockField } from "./blocks";
import { dollarTyping, edgePaste, edgeTyping, fieldLatexPaste } from "./edges";
import { inlinePlugin } from "./inline";
import { mathBlockField, mathBlockKeys } from "./math-blocks";
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

/** Everything Live mode adds over Raw; swapped through a compartment. */
export function livePreview(): Extension {
  return [
    focusedField,
    trackFocus,
    mathBlockKeys,
    mathField(),
    blockField,
    tableField,
    mathBlockField,
    tableKeys,
    dollarTyping,
    edgeTyping,
    edgePaste,
    fieldLatexPaste,
    inlinePlugin,
    mathsWatcher,
  ];
}
