import { historyField, redoDepth, undoDepth } from "@codemirror/commands";
import { language } from "@codemirror/language";
import type { EditorState, StateField } from "@codemirror/state";
import type { Tree } from "@lezer/common";

import { noteLanguage } from "../core/language";
import type { Context } from "./context";
import { firstDifference } from "./diff";
import { treeDifference } from "./tree";
import { isLive, type ShadowValue } from "./value";

export interface CheckOptions {
  /** Also compare the whole text and, given `tree`, the parse tree. */
  full?: boolean;
  /** A Lezer tree of the whole of `state.doc`, or null when there is none
   *  yet. */
  tree?: Tree | null;
}

/** The checks a transaction cannot make, since a field's update never sees
 *  the new state's other fields: undo and redo depth; with `full`, the whole
 *  text, the history as `historyField.toJSON` writes it, and the tree. A
 *  tree mismatch also parses the text afresh, to tell whether CodeMirror's
 *  incremental tree or the shadow is the odd one out. */
export function checkState(ctx: Context, field: StateField<ShadowValue>, state: EditorState, opts: CheckOptions = {}) {
  const live = state.field(field, false);
  if (!isLive(live) || live.chain.stopped || ctx.off) return;
  ctx.guard(live.chain, undefined, () => {
    const { shadow } = live;
    const history = state.field(historyField, false) !== undefined;
    if (history) {
      const want = [undoDepth(state), redoDepth(state)];
      const got = [shadow.undoDepth(), shadow.redoDepth()];
      if (want[0] !== got[0] || want[1] !== got[1]) {
        ctx.mismatch(live, "depth", { expected: { undo: want[0], redo: want[1] }, actual: { undo: got[0], redo: got[1] } });
        return;
      }
    }
    if (!opts.full) return;
    const json = state.toJSON(history ? { history: historyField } : undefined);
    const text: string = json.doc;
    const got = shadow.text();
    if (got !== text) {
      ctx.mismatch(live, "doc", firstDifference(text, got));
      return;
    }
    if (history) {
      const want = JSON.stringify(json.history);
      const got = shadow.historyJson();
      if (got !== want) {
        ctx.mismatch(live, "history", firstDifference(want, got));
        return;
      }
    }
    const tree = opts.tree;
    if (!tree || tree.length !== text.length || state.facet(language) !== noteLanguage) return;
    const triples = shadow.tree();
    const diff = treeDifference(tree, triples, live.names);
    if (!diff) return;
    const fresh = treeDifference(noteLanguage.parser.parse(text), triples, live.names);
    ctx.mismatch(live, "tree", {
      ...diff,
      shadowMatchesFreshLezerParse: fresh === null,
      ...(fresh && { freshLezerParse: fresh }),
    });
  });
}
