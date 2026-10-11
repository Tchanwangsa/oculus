import {
  StateEffect,
  StateField,
  type ChangeDesc,
  type EditorState,
  type Transaction,
} from "@codemirror/state";
import { showTooltip, type EditorView, type Tooltip, type TooltipView } from "@codemirror/view";

import { mathFieldFocused } from "@/components/documents/editor/core/liveFocus";
import { mathAt } from "../../mathContext";
import { MathToolsView } from "./popover";

/** The LaTeX range (`from`–`to`) and the whole node, delimiters included. */
export interface MathRange {
  from: number;
  to: number;
  display: boolean;
  nodeFrom: number;
  nodeTo: number;
}

/** The maths holding the one selection range, both ends in the same node. */
function caretMath(state: EditorState): MathRange | null {
  const { ranges, main } = state.selection;
  if (ranges.length !== 1) return null;
  const ctx = mathAt(state, main.head);
  if (!ctx) return null;
  if (!main.empty && mathAt(state, main.anchor)?.start !== ctx.start) return null;
  return { from: ctx.from, to: ctx.to, display: ctx.display, nodeFrom: ctx.start, nodeTo: ctx.end };
}

/** Typing inside inline maths passes through text the parser rejects (`$a $`,
 *  `$a \$`). An edit wholly inside the last maths keeps it for one step, so
 *  the popover doesn't flicker on every space. */
function carriedMath(prev: MathRange, tr: Transaction): MathRange | null {
  let inside = true;
  tr.changes.iterChangedRanges((fromA, toA) => {
    if (fromA < prev.from || toA > prev.to) inside = false;
  });
  if (!inside) return null;
  const from = tr.changes.mapPos(prev.from, -1);
  const to = tr.changes.mapPos(prev.to, 1);
  const { ranges, main } = tr.state.selection;
  if (ranges.length !== 1 || main.from < from || main.to > to) return null;
  return { ...prev, from, to, nodeTo: tr.changes.mapPos(prev.nodeTo, 1) };
}

const sameRange = (a: MathRange | null, b: MathRange | null) =>
  a === b ||
  (a != null &&
    b != null &&
    a.from === b.from &&
    a.to === b.to &&
    a.display === b.display &&
    a.nodeFrom === b.nodeFrom &&
    a.nodeTo === b.nodeTo);

/** What the toolbox has open: its popover. */
export type ToolsKind = "full";

/** What is open, on the maths starting at `nodeFrom`. */
export interface ToolsOpen {
  nodeFrom: number;
  kind: ToolsKind;
}

/**
 * The open state after a transaction: mapped through its `changes`, set by
 * an `openMathTools` effect (`set`, null closing) on the caret's maths, and
 * dropped once the caret's maths (`nodeFrom`) isn't the one it opened on.
 */
export function nextOpen(
  prev: ToolsOpen | null,
  changes: ChangeDesc | null,
  set: ToolsKind | null | undefined,
  nodeFrom: number | null,
): ToolsOpen | null {
  let open = prev && changes ? { ...prev, nodeFrom: changes.mapPos(prev.nodeFrom, 1) } : prev;
  if (set !== undefined) open = set && nodeFrom != null ? { nodeFrom, kind: set } : null;
  return open && open.nodeFrom === nodeFrom ? open : null;
}

interface ToolsState {
  math: MathRange | null;
  /** False while `math` is carried over an edit rather than parsed. */
  parsed: boolean;
  open: ToolsOpen | null;
  tooltip: Tooltip | null;
}

/** Open the popover on the caret's maths, or close it. */
export const openMathTools = StateEffect.define<ToolsKind | null>();

export const mathToolsField = StateField.define<ToolsState>({
  create(state) {
    return { math: caretMath(state), parsed: true, open: null, tooltip: null };
  },
  update(prev, tr) {
    let set: ToolsKind | null | undefined;
    for (const e of tr.effects) if (e.is(openMathTools)) set = e.value;
    let math = caretMath(tr.state);
    const parsed = math != null;
    if (!math && prev.math && prev.parsed && tr.docChanged) math = carriedMath(prev.math, tr);
    let open = nextOpen(prev.open, tr.docChanged ? tr.changes : null, set, math?.nodeFrom ?? null);
    if (open && prev.open && open.nodeFrom === prev.open.nodeFrom && open.kind === prev.open.kind) open = prev.open;
    const tooltip =
      !open ? null
      : prev.tooltip?.pos === open.nodeFrom && prev.open?.kind === open.kind ? prev.tooltip
      : toolsTooltip(open);
    if (sameRange(math, prev.math) && parsed === prev.parsed && open === prev.open && tooltip === prev.tooltip) {
      return prev;
    }
    return { math, parsed, open, tooltip };
  },
  provide: (f) => showTooltip.from(f, (v) => v.tooltip),
});

/** What is open on the caret's maths, or null. */
export function mathToolsOpen(state: EditorState): ToolsKind | null {
  return state.field(mathToolsField, false)?.open?.kind ?? null;
}

/** One `create` for every popover, so CodeMirror keeps the open one's DOM
 *  while it stays open. */
const createTools = (view: EditorView): TooltipView => new MathToolsView(view);

function toolsTooltip(open: ToolsOpen): Tooltip {
  return { pos: open.nodeFrom, above: false, create: createTools };
}

/** Σ and Mod-Shift-Space in maths: open the popover, or close it. False when
 *  the caret isn't in maths. */
export function toggleMathTools(view: EditorView): boolean {
  const v = view.state.field(mathToolsField, false);
  if (!v?.math) return false;
  view.dispatch({ effects: openMathTools.of(v.open ? null : "full") });
  if (!view.hasFocus && !mathFieldFocused()) view.focus();
  return true;
}
