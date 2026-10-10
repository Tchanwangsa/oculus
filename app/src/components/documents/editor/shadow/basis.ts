import { historyField } from "@codemirror/commands";
import {
  EditorState,
  type ChangeDesc,
  type ChangeSet,
  type EditorSelection,
  type Text,
} from "@codemirror/state";

import type { Pop, Replay } from "./types";

/** `@codemirror/commands`' `HistEvent`. */
interface HistEvent {
  changes?: ChangeSet;
  mapped?: ChangeDesc;
  startSelection?: EditorSelection;
  selectionsAfter: readonly EditorSelection[];
}

/** `@codemirror/commands`' `HistoryState`. */
export interface LiveHistory {
  done: readonly HistEvent[];
  undone: readonly HistEvent[];
  prevTime: number;
  prevUserEvent?: string;
}

/** The history field's runtime value. `historyField.toJSON` drops stored
 *  selections' goal columns and assoc and the previous time and user event,
 *  so a shadow seeded from it would group the next edit unlike CodeMirror.
 *  These are untyped internals, read here and nowhere else. */
function liveHistory(state: EditorState): LiveHistory | null {
  return (state.field(historyField, false) as unknown as LiveHistory | undefined) ?? null;
}

/** What a seed needs of a state, held by reference (all persistent values)
 *  and serialised only when a seed or a report wants it. */
export interface Basis {
  doc: Text;
  selection: EditorSelection;
  history: LiveHistory | null;
}

export function basisOf(state: EditorState): Basis {
  return { doc: state.doc, selection: state.selection, history: liveHistory(state) };
}

/** `selection.toJSON()` with each range's goal column, bidi level and assoc,
 *  and its `from` and `to`, which mapping can leave as `from > to`. */
export function richSelection(sel: EditorSelection): string {
  return JSON.stringify(richSelectionValue(sel));
}

function richSelectionValue(sel: EditorSelection) {
  return {
    ranges: sel.ranges.map((r) => ({
      anchor: r.anchor,
      head: r.head,
      goalColumn: r.goalColumn,
      bidiLevel: r.bidiLevel,
      assoc: r.assoc,
      from: r.from,
      to: r.to,
    })),
    main: sel.mainIndex,
  };
}

/** `selection.toJSON()`, as the shadow's `selectionJson()` writes it. */
export function plainSelection(sel: EditorSelection): string {
  return JSON.stringify(sel.toJSON());
}

function historyJson(h: LiveHistory): string {
  const event = (e: HistEvent) => ({
    changes: e.changes?.toJSON(),
    mapped: e.mapped?.toJSON(),
    startSelection: e.startSelection && richSelectionValue(e.startSelection),
    selectionsAfter: e.selectionsAfter.map(richSelectionValue),
  });
  return JSON.stringify({
    done: h.done.map(event),
    undone: h.undone.map(event),
    prevTime: h.prevTime,
    prevUserEvent: h.prevUserEvent ?? null,
  });
}

export function seedOf(b: Basis): Replay["seed"] {
  return {
    doc: b.doc.toString(),
    selection: richSelection(b.selection),
    history: b.history && historyJson(b.history),
  };
}

/** The selection CodeMirror's `undoSelection`/`redoSelection` restores from
 *  `state`, before any transaction filter: the top event's last selection. */
export function poppedSelection(state: EditorState, pop: Pop): EditorSelection | null {
  const h = liveHistory(state);
  const branch = pop === "undoSelection" ? h?.done : h?.undone;
  const after = branch?.[branch.length - 1]?.selectionsAfter;
  const sel = after?.[after.length - 1];
  if (!sel) return null;
  return state.facet(EditorState.allowMultipleSelections) ? sel : sel.asSingle();
}
