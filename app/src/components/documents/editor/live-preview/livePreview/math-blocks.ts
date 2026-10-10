import { StateField, type Range } from "@codemirror/state";
import { Decoration, EditorView, type DecorationSet } from "@codemirror/view";

import { fieldReady } from "../../math/field/mathField";
import { MathWidget } from "../widgets";
import { blockField } from "./blocks";

/** The rendered maths among `decos`, inline or blocks. */
export function mathIn(decos: DecorationSet, block: boolean): DecorationSet {
  const out: Range<Decoration>[] = [];
  for (const iter = decos.iter(); iter.value; iter.next()) {
    const w = iter.value.spec.widget;
    if (w instanceof MathWidget && w.block === block) out.push(iter.value.range(iter.from, iter.to));
  }
  return Decoration.set(out);
}

/** Rendered maths is atomic once the field can take it, so a selection
 *  covers it whole and the caret rests only at its edges, where a block's
 *  field opens (`targetAt` in `math/field/mathField/visual-state.ts`).
 *  Before the field's engine loads, Backspace at its edge would take the
 *  whole node. */
export function mathAtoms(view: EditorView, set: DecorationSet | undefined): DecorationSet {
  return fieldReady(view.state) && set ? set : Decoration.none;
}

export const mathBlockField = StateField.define<DecorationSet>({
  create: (state) => mathIn(state.field(blockField), true),
  update(blocks, tr) {
    const next = tr.state.field(blockField);
    return next === tr.startState.field(blockField, false) ? blocks : mathIn(next, true);
  },
  provide: (f) => EditorView.atomicRanges.of((view) => mathAtoms(view, view.state.field(f))),
});
