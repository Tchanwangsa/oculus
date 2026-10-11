import { snippet } from "@codemirror/autocomplete";
import { isolateHistory } from "@codemirror/commands";
import {
  StateEffect,
  StateField,
  type EditorState,
  type Extension,
  type Transaction,
  type TransactionSpec,
} from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import { mathAt } from "../../mathContext";
import { RULES } from "./rules";
import { inTextArgument } from "./scan";

/**
 * LaTeX shorthand in maths, Obsidian Latex Suite style: typing `@a`, `a/`,
 * `xsr`, `->`, `sin ` … rewrites the LaTeX before the caret. Fires only on a
 * typed character inside maths and outside `\text{}`-like arguments.
 *
 * The typed character goes in as its own transaction, then the rewrite as a
 * second one isolated in history, so ⌘Z restores exactly what was typed.
 * Rewrites with slots are `snippet()`s: Tab / Shift-Tab move between them and
 * the last Tab leaves the group. Their fields are numbered, `#{0}` being the
 * exit, because `snippet()` sorts a numbered field before unnumbered ones.
 * The rule tables are `tables.ts`, the scanning helpers `scan.ts`, the rules
 * themselves `rules.ts`.
 */

/** End of the last control word a rewrite inserted, while the caret has not
 *  moved off it: a letter typed there gets a space so it can't extend it. */
const setGlueEnd = StateEffect.define<number>();
const glueEnd = StateField.define<number | null>({
  create: () => null,
  update(value, tr) {
    for (const e of tr.effects) if (e.is(setGlueEnd)) return e.value;
    return tr.docChanged || tr.selection ? null : value;
  },
});

const expansion = { userEvent: "input.complete", annotations: isolateHistory.of("full") };

/** Longest maths prefix a rule scans; no rule reaches further back. */
const MAX_SCAN = 20000;

/**
 * The rewrite for a character just typed, `pos` being the caret after it and
 * `math` the LaTeX bounds it was typed in. Null when no rule fires.
 */
export function expand(
  state: EditorState,
  pos: number,
  math: { from: number; to: number },
): TransactionSpec | null {
  if (pos <= math.from || pos > math.to) return null;
  const base = Math.max(math.from, pos - MAX_SCAN);
  const s = state.sliceDoc(base, pos);
  if (inTextArgument(s)) return null;
  for (const rule of RULES) {
    const rewrite = rule(s);
    if (!rewrite) continue;
    if ("template" in rewrite) {
      // Take the snippet's transaction apart to dispatch it as an expansion.
      const built: Transaction[] = [];
      snippet(rewrite.template)({ state, dispatch: (tr) => built.push(tr) }, null, base + rewrite.from, pos);
      const t = built[0];
      if (!t) return null;
      return { changes: t.changes, selection: t.selection, effects: t.effects, scrollIntoView: true, ...expansion };
    }
    const changes = state.changes(
      rewrite.changes.map(({ from, to, insert }) => ({ from: base + from, to: base + (to ?? from), insert })),
    );
    const head = changes.mapPos(pos, 1);
    return {
      changes,
      selection: { anchor: head },
      effects: rewrite.glue ? setGlueEnd.of(head) : [],
      scrollIntoView: true,
      ...expansion,
    };
  }
  return null;
}

/** A letter typed straight after an inserted control word, with the space
 *  that keeps `\alpha` + `x` from reading as `\alphax`. */
export function spacedInput(state: EditorState, from: number, to: number, text: string): TransactionSpec | null {
  if (from !== to || !/^[A-Za-z]$/.test(text) || state.field(glueEnd, false) !== from) return null;
  return {
    changes: { from, insert: ` ${text}` },
    selection: { anchor: from + text.length + 1 },
    scrollIntoView: true,
    userEvent: "input.type",
  };
}

export function mathShorthand(): Extension {
  return [
    glueEnd,
    EditorView.inputHandler.of((view, from, to, text, insert) => {
      const { state } = view;
      if (view.composing || view.compositionStarted || state.readOnly) return false;
      if (text.length !== 1 || from !== to || state.selection.ranges.length > 1) return false;
      const spaced = spacedInput(state, from, to, text);
      if (spaced) {
        view.dispatch(spaced);
        return true;
      }
      // Judged before the insert: a typed space can unmake `$…$` maths.
      const math = mathAt(state, from);
      if (!math) return false;
      const typed = insert();
      view.dispatch(typed);
      const pos = view.state.selection.main.head;
      const bounds = { from: typed.changes.mapPos(math.from, -1), to: typed.changes.mapPos(math.to, 1) };
      if (view.state === typed.state && pos === from + 1) {
        const spec = expand(view.state, pos, bounds);
        if (spec) view.dispatch(spec);
      }
      return true;
    }),
  ];
}
