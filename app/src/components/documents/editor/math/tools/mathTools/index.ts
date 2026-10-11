import type { Extension } from "@codemirror/state";

import { fieldKeys, fieldTools } from "../../field/mathField";
import { typedCommands } from "./completion";
import { fieldKey, toolsKeymap } from "./keys";
import { pastedField } from "./paste-shape";
import { mathToolsField, openMathTools } from "./state";

/**
 * The maths toolbox: a popover centred under the maths the caret is in (live
 * KaTeX preview, recents, a tabbed palette, an inline/block switch and a
 * close button), and `\` completion inside maths. Both insert `snippet()`s,
 * so a template's `{}` slots are Tab fields. The popover opens only when
 * asked — Σ or Mod-Shift-Space in maths — and stays until Esc, its close
 * button or the caret leaving that maths. Under Live mode's visual field
 * (`field/rustField`) it is lighter — no preview — and its cells insert into
 * the field; a control switches between the field and TeX. There Space
 * opens the field's own list of picks (`field/mathView/popover/`), and
 * Space again the popover. The palette's data is `mathPalette.ts`; the look
 * is `theme/`.
 *
 * `state.ts` holds what is open, `popover.ts` the popover, `shape.ts` the
 * inline ⇄ block switch, `paste-shape.ts` the chip after a paste, `keys.ts`
 * the field's and the editor's keys.
 */

export { offerShapeSwitch } from "./paste-shape";
export { mathCompletionSource, mathOptionPreview } from "./completion";
export { emptyPairToBlock } from "./shape";
export {
  mathToolsOpen,
  nextOpen,
  openMathTools,
  toggleMathTools,
  type ToolsKind,
  type ToolsOpen,
} from "./state";

/** The popover, in both modes, its keys in the visual field, and the chip
 *  on pasted maths. */
export function mathTools(): Extension {
  return [
    mathToolsField,
    pastedField,
    toolsKeymap,
    fieldKeys.of(fieldKey),
    fieldTools.of((view) => view.dispatch({ effects: openMathTools.of("full") })),
    typedCommands,
  ];
}
