import type { Extension } from "@codemirror/state";

import { fieldKeys } from "../../field/mathField";
import { typedCommands } from "./completion";
import { fieldKey, toolsKeymap } from "./keys";
import { pastedField } from "./paste-shape";
import { mathToolsField } from "./state";

/**
 * The maths toolbox: a popover centred under the maths the caret is in (live
 * KaTeX preview, recents, a tabbed palette, an inline/block switch and a
 * close button), and `\` completion inside maths. Both insert `snippet()`s,
 * so a template's `{}` slots are Tab fields. The popover opens only when
 * asked — Σ or Mod-Shift-Space in maths — and stays until Esc, its close
 * button or the caret leaving that maths. Under Live mode's MathLive field
 * (`field/mathField`) it is lighter — no preview — and its cells insert into
 * the field; a control switches between the field and TeX. There Space first
 * opens quick picks at the field's caret: the five entries last used in the
 * note's subject, keyed 1–5, and a way on to the popover. The palette's
 * data is `mathPalette.ts`; the look is `theme/`.
 *
 * `state.ts` holds what is open, `popover.ts` and `quick-picks.ts` the two
 * tooltips, `shape.ts` the inline ⇄ block switch, `paste-shape.ts` the chip
 * after a paste, `keys.ts` the field's and the editor's keys.
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

/** The popover, in both modes, the quick picks in the visual field, and the
 *  chip on pasted maths. */
export function mathTools(): Extension {
  return [mathToolsField, pastedField, toolsKeymap, fieldKeys.of(fieldKey), typedCommands];
}
