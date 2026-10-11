import { EditorSelection, type ChangeSpec, type EditorState, type StateCommand } from "@codemirror/state";

import type { Dispatch } from "./shared";

/** Line numbers the selection covers; a range ending at a line's start
 *  does not take that line. */
function selectedLines(state: EditorState): number[] {
  const lines = new Set<number>();
  for (const r of state.selection.ranges) {
    const first = state.doc.lineAt(r.from).number;
    let last = state.doc.lineAt(r.to).number;
    if (!r.empty && last > first && state.doc.line(last).from === r.to) last--;
    for (let n = first; n <= last; n++) lines.add(n);
  }
  return [...lines].sort((a, b) => a - b);
}

/** Apply line edits, carets pushed past text inserted at them. */
function editLines(
  state: EditorState,
  dispatch: Dispatch,
  edit: (text: string, index: number) => { strip: number; insert: string } | null,
  at: (text: string) => number = (text) => text.length - text.trimStart().length,
): boolean {
  const specs: ChangeSpec[] = [];
  selectedLines(state).forEach((n, i) => {
    const ln = state.doc.line(n);
    const e = edit(ln.text, i);
    if (!e) return;
    const from = ln.from + at(ln.text);
    specs.push({ from, to: from + e.strip, insert: e.insert });
  });
  if (!specs.length) return false;
  const changes = state.changes(specs);
  const selection = EditorSelection.create(
    state.selection.ranges.map((r) =>
      EditorSelection.range(changes.mapPos(r.anchor, 1), changes.mapPos(r.head, 1)),
    ),
    state.selection.mainIndex,
  );
  dispatch(state.update({ changes, selection, scrollIntoView: true, userEvent: "input" }));
  return true;
}

const HEADING_PREFIX = /^#{1,6}[ \t]+|^#{1,6}$/;
/** A list or task prefix after indentation. */
const LIST_PREFIX = /^(?:[-*+]|\d+[.)])[ \t]+(?:\[[ xX]\][ \t]+)?/;

export type BlockType = "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6";

/** Make each selected line plain text or a heading. */
export function setBlockType(type: BlockType): StateCommand {
  const level = type === "p" ? 0 : Number(type.slice(1));
  return ({ state, dispatch }) =>
    editLines(state, dispatch, (text) => {
      const body = text.trimStart();
      const strip = HEADING_PREFIX.exec(body)?.[0].length ?? 0;
      const insert = level ? `${"#".repeat(level)} ` : "";
      if (!strip && !insert) return null;
      return { strip, insert };
    });
}

export type ListKind = "bullet" | "ordered" | "task" | "quote";

const HAS: Record<ListKind, RegExp> = {
  bullet: /^[-*+][ \t]+(?!\[[ xX]\][ \t])/,
  ordered: /^\d+[.)][ \t]+/,
  task: /^[-*+][ \t]+\[[ xX]\][ \t]+/,
  quote: /^>[ \t]?/,
};

/** Toggle a list kind or a quote on the selected lines: off if every
 *  non-blank line has it, else on, replacing any other list marker. */
export function toggleList(kind: ListKind): StateCommand {
  return ({ state, dispatch }) => {
    const lines = selectedLines(state).map((n) => state.doc.line(n).text);
    const filled = lines.filter((t) => t.trim() !== "");
    const on = filled.length > 0 && filled.every((t) => HAS[kind].test(t.trimStart()));
    const skipBlank = lines.length > 1;
    let count = 0;
    // A new quote marker goes at the line's start, a list marker after its indent.
    const at = kind === "quote" && !on ? () => 0 : undefined;
    return editLines(state, dispatch, (text) => {
      const body = text.trimStart();
      if (skipBlank && body === "") return null;
      if (kind === "quote") {
        if (on) return { strip: HAS.quote.exec(body)?.[0].length ?? 0, insert: "" };
        return { strip: 0, insert: "> " };
      }
      const existing = HAS[kind].exec(body)?.[0].length ?? 0;
      if (on) return { strip: existing, insert: "" };
      const strip = LIST_PREFIX.exec(body)?.[0].length ?? HEADING_PREFIX.exec(body)?.[0].length ?? 0;
      count += 1;
      const insert = kind === "bullet" ? "- " : kind === "task" ? "- [ ] " : `${count}. `;
      return { strip, insert };
    }, at);
  };
}
