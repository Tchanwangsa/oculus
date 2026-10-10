import { EditorSelection, Prec, StateField, type ChangeSpec, type EditorState, type Range } from "@codemirror/state";
import { Decoration, EditorView, keymap, type Command, type DecorationSet } from "@codemirror/view";

import { enterTable, TableWidget } from "../table";
import { blockField } from "./blocks";
import { caretOf, type Span } from "./shared";

function tablesIn(blocks: DecorationSet): DecorationSet {
  const out: Range<Decoration>[] = [];
  for (const iter = blocks.iter(); iter.value; iter.next()) {
    if (iter.value.spec.widget instanceof TableWidget) out.push(iter.value.range(iter.from, iter.to));
  }
  return Decoration.set(out);
}

/** The drawn tables alone. Atomic, so the caret and selection skip their
 *  source; it rests only at a table's start or end. */
export const tableField = StateField.define<DecorationSet>({
  create: (state) => tablesIn(state.field(blockField)),
  update(tables, tr) {
    const blocks = tr.state.field(blockField);
    return blocks === tr.startState.field(blockField, false) ? tables : tablesIn(blocks);
  },
  provide: (f) => EditorView.atomicRanges.of((view) => view.state.field(f)),
});

/** The drawn table starting (`from`) or ending (`to`) exactly at `pos`. */
export function tableAt(state: EditorState, pos: number, edge: "from" | "to"): Span | null {
  let found = null as Span | null;
  state.field(tableField, false)?.between(pos, pos, (from, to) => {
    if ((edge === "from" ? from : to) !== pos) return;
    found = { from, to };
    return false;
  });
  return found;
}

/** The drawn table on the line just above the one holding `pos` (`up`), or
 *  just below it. */
export function tableBeside(state: EditorState, pos: number, up: boolean): Span | null {
  const ln = state.doc.lineAt(pos);
  if (up) return ln.from > 0 ? tableAt(state, ln.from - 1, "to") : null;
  return ln.to < state.doc.length ? tableAt(state, ln.to + 1, "from") : null;
}

/** An arrow into a table: from the line under it (its first visual line for
 *  ↑, its start for ←) or the table's end into the last row; from the line
 *  over it or the table's start into the header. */
function intoTable(dir: "up" | "down" | "left" | "right"): Command {
  const back = dir === "up" || dir === "left";
  return (view) => {
    const { state } = view;
    const head = caretOf(state);
    if (head == null) return false;
    let table = tableAt(state, head, back ? "to" : "from");
    let x: number | undefined;
    if (!table) {
      const ln = state.doc.lineAt(head);
      table = tableBeside(state, head, back);
      if (!table) return false;
      if (dir === "left" || dir === "right") {
        if (head !== (back ? ln.from : ln.to)) return false;
      } else {
        const next = view.moveVertically(EditorSelection.cursor(head), !back).head;
        if (back ? next >= ln.from : next <= ln.to) return false;
        x = view.coordsAtPos(head)?.left;
      }
    }
    const col = x != null ? { x } : dir === "left" ? "last" : "first";
    return enterTable(view, table.from, back ? "last" : "first", col, dir === "right" ? "start" : "end");
  };
}

/** Backspace from the line under a table (or its end), Delete from the line
 *  over it (or its start): into the table, never joining a row or deleting the
 *  atomic whole. An empty line between goes too, unless text lies beyond it. */
function deleteIntoTable(forward: boolean): Command {
  return (view) => {
    const { state } = view;
    const { doc } = state;
    const head = caretOf(state);
    if (head == null) return false;
    let table = tableAt(state, head, forward ? "from" : "to");
    let change: ChangeSpec | null = null;
    if (!table) {
      const ln = doc.lineAt(head);
      if (head !== (forward ? ln.to : ln.from)) return false;
      table = tableBeside(state, head, !forward);
      if (!table) return false;
      const beyond = forward ? (ln.number > 1 ? doc.line(ln.number - 1) : null) : ln.number < doc.lines ? doc.line(ln.number + 1) : null;
      if (!ln.length && !beyond?.text.trim()) {
        change = forward ? { from: ln.from, to: ln.to + 1 } : { from: ln.from - 1, to: ln.to };
      }
    }
    if (change) view.dispatch({ changes: change, scrollIntoView: true, userEvent: "delete" });
    // Deleting the line over a table moves it up by that newline.
    const base = change && forward ? table.from - 1 : table.from;
    const entered = enterTable(view, base, forward ? "first" : "last", forward ? "first" : "last", forward ? "start" : "end");
    return entered || change != null;
  };
}

export const tableKeys = Prec.highest(
  keymap.of([
    { key: "ArrowUp", run: intoTable("up") },
    { key: "ArrowDown", run: intoTable("down") },
    { key: "ArrowLeft", run: intoTable("left") },
    { key: "ArrowRight", run: intoTable("right") },
    { key: "Backspace", run: deleteIntoTable(false) },
    { key: "Delete", run: deleteIntoTable(true) },
  ]),
);
