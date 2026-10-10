import { StateEffect, StateField, type EditorState, type Transaction } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

import { liveFocused } from "@/components/documents/editor/core/liveFocus";
import { ancestorAt } from "@/components/documents/editor/syntax/syntax";
import { MathField, mathsReady, parseError } from "@/lib/maths";
import { mathAt, mathContextOf, ownsLines, type MathContext } from "../../mathContext";
import { fieldTrapped, rustField } from "../fieldEngine";
import { fieldWrite } from "../mathFieldEdits";
import { lib, loadState, mathLiveSettled, type LoadState } from "./loader";
import { toField } from "./serialize";

/** The maths the field edits: its span, its LaTeX range, and whether the
 *  block layer draws it (a display block owning its lines). */
export interface VisualMath {
  start: number;
  end: number;
  from: number;
  to: number;
  display: boolean;
  block: boolean;
}

/** The maths the field is open on. `id` names it while edits map it along;
 *  a field writes only to the maths with its id (`writableSpan`). */
export interface ActiveMath extends VisualMath {
  id: number;
}

let nextId = 1;

interface VisualState {
  lib: LoadState;
  /** Start of the maths switched to TeX, until the selection leaves it. */
  tex: number | null;
  active: ActiveMath | null;
}

const setTex = StateEffect.define<number | null>();

export function visualOf(state: EditorState, ctx: MathContext): VisualMath | null {
  const block = ctx.display && ctx.node != null && ownsLines(state, ctx.node);
  // The inline layer can't replace a line break, so a multi-line block
  // inside a quote or list item stays source.
  if (!block && state.doc.lineAt(ctx.start).number !== state.doc.lineAt(ctx.end).number) return null;
  return { start: ctx.start, end: ctx.end, from: ctx.from, to: ctx.to, display: ctx.display, block };
}

/** The maths holding the one selection range: inline maths with the caret
 *  between its delimiters, a block anywhere on its lines but its edges. */
function targetAt(state: EditorState): VisualMath | null {
  const { ranges, main } = state.selection;
  if (ranges.length !== 1) return null;
  let ctx = mathAt(state, main.head);
  if (!ctx) {
    const node = ancestorAt(state, main.head, (n) => n.name === "BlockMath", [-1, 1]);
    if (node && ownsLines(state, node)) ctx = mathContextOf(node);
  }
  if (!ctx || main.from < ctx.start || main.to > ctx.end) return null;
  const v = visualOf(state, ctx);
  return v && atBlockEdge(state, v) ? null : v;
}

/** Typing passes through text the parser may briefly read differently; an
 *  edit inside the last field's LaTeX keeps it rather than unmounting it. */
function carried(prev: ActiveMath, tr: Transaction): ActiveMath | null {
  let inside = true;
  tr.changes.iterChangedRanges((fromA, toA) => {
    if (fromA < prev.from || toA > prev.to) inside = false;
  });
  if (!inside) return null;
  const next = {
    ...prev,
    from: tr.changes.mapPos(prev.from, -1),
    to: tr.changes.mapPos(prev.to, 1),
    end: tr.changes.mapPos(prev.end, 1),
  };
  const { ranges, main } = tr.state.selection;
  return ranges.length === 1 && main.from >= next.start && main.to <= next.end ? next : null;
}

/** A caret resting just before or after a block, or a selection taking the
 *  whole block: the rendering keeps it (`MathWidget` in `live-preview/widgets/math.ts`) rather
 *  than the field taking it, as inline maths keeps a caret on its edge. */
function atBlockEdge(state: EditorState, v: VisualMath): boolean {
  const { main } = state.selection;
  return v.block && (main.from === v.start || main.to === state.doc.lineAt(v.end).to);
}

const cleanCache = new Map<string, boolean>();

/** Whether the Rust field's edit model opens on it (it parses, and the
 *  engine doesn't trap). */
function opensInRust(source: string, display: boolean): boolean {
  try {
    MathField.open(source, display).free();
    return true;
  } catch {
    return false;
  }
}

/** The field can edit it: for the Rust field, its edit model opens on it;
 *  for MathLive's, MathLive reads it without errors and KaTeX renders it
 *  (the field's output KaTeX must draw afterwards). False until the maths
 *  engine (and for MathLive's field, MathLive) has loaded. */
export function readsCleanly(source: string, display: boolean): boolean {
  const rust = rustField();
  if (!mathsReady() || (!rust && !lib) || (rust && fieldTrapped(source, display))) return false;
  const key = `${rust ? "R" : "M"}${display ? "D" : "I"}${source}`;
  let ok = cleanCache.get(key);
  if (ok === undefined) {
    ok = rust
      ? opensInRust(source, display)
      : lib!.validateLatex(toField(source, display)).length === 0 &&
        parseError(source, { displayMode: display, strict: "ignore" }) === undefined;
    if (cleanCache.size > 500) cleanCache.clear();
    cleanCache.set(key, ok);
  }
  return ok;
}

/** Whether the field's engine has loaded: the Rust field needs only the
 *  maths engine, MathLive's field MathLive's own import (`lib`, which the
 *  state holds). */
function engineLoad(lib: LoadState): LoadState {
  if (!rustField()) return lib;
  return mathsReady() ? "ready" : "loading";
}

/** Where the open-able field's engine stands, or null outside Live mode. */
export function fieldLoad(state: EditorState): LoadState | null {
  const v = state.field(visualMathField, false);
  return v ? engineLoad(v.lib) : null;
}

const sameVisual = (a: ActiveMath | null, b: ActiveMath | null) =>
  a === b ||
  (a != null &&
    b != null &&
    a.id === b.id &&
    a.start === b.start &&
    a.end === b.end &&
    a.from === b.from &&
    a.to === b.to &&
    a.display === b.display &&
    a.block === b.block);

export const visualMathField = StateField.define<VisualState>({
  create: () => ({ lib: loadState, tex: null, active: null }),
  update(prev, tr) {
    let lib = prev.lib;
    let tex = prev.tex != null && tr.docChanged ? tr.changes.mapPos(prev.tex, 1) : prev.tex;
    for (const e of tr.effects) {
      if (e.is(mathLiveSettled)) lib = e.value;
      else if (e.is(setTex)) tex = e.value;
    }
    const here = targetAt(tr.state);
    if (tex != null && here?.start !== tex) tex = null;
    let active: ActiveMath | null = null;
    if (engineLoad(lib) === "ready" && liveFocused(tr.state)) {
      const target = here ?? (prev.active && tr.docChanged ? carried(prev.active, tr) : null);
      if (target && target.start !== tex) {
        const same = prev.active != null && tr.changes.mapPos(prev.active.start, -1) === target.start;
        // Checked on the way in and after outside edits (an undo); the
        // field's own output isn't re-judged.
        const own = same && (!tr.docChanged || tr.annotation(fieldWrite) === prev.active!.id);
        if (own || readsCleanly(tr.state.sliceDoc(target.from, target.to).trim(), target.display)) {
          active = { ...target, id: same ? prev.active!.id : nextId++ };
        }
      }
    }
    if (lib === prev.lib && tex === prev.tex && sameVisual(active, prev.active)) return prev;
    return { lib, tex, active: sameVisual(active, prev.active) ? prev.active : active };
  },
});

/** The maths the field is editing, or null. */
export function visualMath(state: EditorState): ActiveMath | null {
  return state.field(visualMathField, false)?.active ?? null;
}

/**
 * How Live mode draws maths the selection touches but the field isn't on:
 * rendered when the field could take it (the caret on its edge, a selection
 * running past it) or while the field's engine loads, else as source. A
 * caret inside it before the engine arrives gets the source, since typing
 * would otherwise land beside the rendering.
 */
export function touchedMath(state: EditorState, start: number, from: number, to: number, display: boolean): "render" | "source" {
  const v = state.field(visualMathField, false);
  const load = v && engineLoad(v.lib);
  if (!v || load === "failed" || v.tex === start) return "source";
  if (load === "loading") return targetAt(state)?.start === start ? "source" : "render";
  return readsCleanly(state.sliceDoc(from, to).trim(), display) ? "render" : "source";
}

/** What the toolbox's mode control offers: "tex" while the field is open,
 *  "visual" for maths switched to TeX that the field can take, else null. */
export function visualToggle(state: EditorState): "tex" | "visual" | null {
  const v = state.field(visualMathField, false);
  if (!v || engineLoad(v.lib) !== "ready") return null;
  if (v.active) return "tex";
  const here = targetAt(state);
  if (!here || v.tex !== here.start) return null;
  return readsCleanly(state.sliceDoc(here.from, here.to).trim(), here.display) ? "visual" : null;
}

/** Switch the caret's maths to TeX (typed as source) or back. */
export function setMathMode(view: EditorView, mode: "tex" | "visual") {
  const here = targetAt(view.state);
  if (!here) return;
  if (mode === "tex") {
    // The caret at the end of the LaTeX, not on a block's closing line.
    const end = here.from + view.state.sliceDoc(here.from, here.to).trimEnd().length;
    view.dispatch({ effects: setTex.of(here.start), selection: { anchor: Math.max(end, here.from) } });
    view.focus();
  } else {
    view.dispatch({ effects: setTex.of(null) });
  }
}
