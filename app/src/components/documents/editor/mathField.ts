import { isolateHistory, redo, undo } from "@codemirror/commands";
import {
  EditorSelection,
  EditorState,
  Facet,
  Prec,
  StateEffect,
  StateField,
  Transaction,
  type ChangeSpec,
  type Extension,
} from "@codemirror/state";
import { EditorView, ViewPlugin, WidgetType, keymap, type Command } from "@codemirror/view";
import katex from "katex";
import type { InlineShortcutDefinitions, MathfieldElement } from "mathlive";

import { noteHost } from "./host";
import { liveFocused, mathFieldFocused, setFocused } from "./liveFocus";
import { mathAt, mathContextOf, ownsLines, type MathContext } from "./mathContext";
import {
  FIELD_INPUT,
  caretAfterChange,
  fieldWrite,
  minimalChange,
  selectionPastField,
  squeezeBlankLines,
  withoutEndRows,
  writableSpan,
} from "./mathFieldEdits";
import { applyGridEdit, gridKey, type GridStep } from "./mathMatrixField";
import { GREEK, OPERATORS, POWERS } from "./mathShorthand";
import { recordCommand } from "./mathUsage";
import { ancestorAt } from "./syntax";

/**
 * Visual maths in Live mode: while the caret is in a maths node, a MathLive
 * `<math-field>` stands in for it and is where it is edited — slots for a
 * fraction's parts, `\` commands that become one symbol. Each edit in the
 * field rewrites only the LaTeX between the delimiters; opening it writes
 * nothing. Maths MathLive or KaTeX can't read cleanly, and maths switched to
 * TeX from the toolbox, are typed as LaTeX source (`mathTools.ts`).
 * MathLive is imported on first use. It also draws the maths the field isn't
 * on (`staticMath`), so opening the field doesn't move it; until it arrives,
 * if it fails, or for LaTeX it can't read, maths renders as KaTeX.
 * Rendered markdown's read-only field (`lib/mathSelect.ts`) reuses the
 * loader, hit-test, widening and copy helpers exported here.
 */

type MathLive = typeof import("mathlive");

// ── Loading ───────────────────────────────────────────────────────────────

type LoadState = "loading" | "ready" | "failed";

let lib: MathLive | null = null;
let loadState: LoadState = "loading";
let loading: Promise<void> | null = null;
const waiting = new Set<EditorView>();
const mathLiveSettled = StateEffect.define<LoadState>();

export function loadMathLive(): Promise<void> {
  loading ??= Promise.all([import("mathlive"), import("mathlive/static.css?raw")]).then(
    ([m, css]) => {
      configure(m);
      staticStyles(css.default);
      lib = m;
      settle("ready");
    },
    () => settle("failed"),
  );
  return loading;
}

/** MathLive's stylesheet for static markup (`staticMath`), once, without
 *  its `@font-face` rules: KaTeX's CSS declares the same families. */
function staticStyles(css: string) {
  const style = document.createElement("style");
  style.dataset.mathliveStatic = "";
  style.textContent = css.replace(/@font-face\s*\{[^}]*\}/g, "");
  document.head.append(style);
}

function settle(state: LoadState) {
  loadState = state;
  for (const view of waiting) view.dispatch({ effects: mathLiveSettled.of(state) });
  waiting.clear();
}

/** MathLive has loaded: rendered maths is drawn with it (`staticMath`). */
export function mathLiveReady(): boolean {
  return lib != null;
}

/** Statics, once: our bundle already carries KaTeX's fonts (MathLive reuses
 *  them when every family is in `document.fonts`), and nothing may reach
 *  the network or make a sound. */
function configure(m: MathLive) {
  const MF = m.MathfieldElement;
  MF.fontsDirectory = null;
  MF.soundsDirectory = null;
  MF.keypressSound = null;
  MF.plonkSound = null;
  MF.computeEngine = null;
  patchArrays(MF);
}

/** Row stretch for arrays and matrices, in KaTeX (`widgets.ts`) and here. */
export const MATH_ARRAYSTRETCH = 1.2;
/** Space between a display block's top-level `\\` lines, in em: KaTeX's
 *  `.newline` (`theme.ts`) and the field's root `lines` table. */
export const MATH_LINE_GAP = 0.5;

/**
 * Layout fixes on MathLive's internal array atom (recheck on upgrade), so
 * entering a block doesn't move it: `array` sits centred on the maths axis
 * as in LaTeX and KaTeX (MathLive hangs it from its first row, which pads a
 * `\left[ \begin{array}…\right]` above), drops its outer column padding
 * when it is all a `\left…\right` holds, as KaTeX's rendering does
 * (`hugArrays` in `widgets.ts`), and the root `lines` table
 * (`\displaylines`) takes `MATH_LINE_GAP` between rows while other arrays
 * take `MATH_ARRAYSTRETCH`. The class is reached through a throwaway field.
 */
function patchArrays(MF: MathLive["MathfieldElement"]) {
  const probe = new MF();
  probe.value = "\\begin{array}{c}x\\end{array}";
  probe.style.cssText = "position: fixed; left: -9999px; visibility: hidden";
  document.body.append(probe);
  const array = modelOf(probe)?.at(1)?.parent;
  probe.remove();
  if (array?.type !== "array") return;
  type ArrayAtom = {
    environmentName: string;
    arraystretch?: number;
    leftDelim?: string;
    rightDelim?: string;
    parent?: { type: string; body?: { type: string }[] };
  };
  type Context = { getRegisterAsEm(name: string, precision?: number): number };
  const proto = Object.getPrototypeOf(array) as { render(this: ArrayAtom, context: object): unknown };
  const render = proto.render;
  const spaced = new WeakSet<object>();
  let contextPatched = false;
  proto.render = function (context) {
    if (this.environmentName === "array") {
      const { leftDelim, rightDelim, parent } = this;
      // Delimiters of "." draw none and add no outer padding.
      const hug = parent?.type === "leftright" && parent.body?.filter((a) => a.type !== "first").length === 1;
      this.environmentName = "matrix";
      if (hug) this.leftDelim = this.rightDelim = ".";
      try {
        return stretched(this, MATH_ARRAYSTRETCH, () => render.call(this, context));
      } finally {
        this.environmentName = "array";
        if (hug) Object.assign(this, { leftDelim, rightDelim });
      }
    }
    if (this.environmentName !== "lines") return stretched(this, MATH_ARRAYSTRETCH, () => render.call(this, context));
    // The table reads `jot` from a context whose parent is this one.
    if (!contextPatched) {
      contextPatched = true;
      const cx = Object.getPrototypeOf(context) as Context & { parent?: object };
      const em = cx.getRegisterAsEm;
      cx.getRegisterAsEm = function (this: Context & { parent?: object }, name, precision) {
        return name === "jot" && this.parent && spaced.has(this.parent) ? MATH_LINE_GAP : em.call(this, name, precision);
      };
    }
    spaced.add(context);
    try {
      return render.call(this, context);
    } finally {
      spaced.delete(context);
    }
  };
}

/** Render with `stretch` unless the environment sets its own (`cases`,
 *  `smallmatrix`), as KaTeX's `\arraystretch` macro applies. */
function stretched<T>(atom: { arraystretch?: number }, stretch: number, render: () => T): T {
  if (atom.arraystretch !== undefined) return render();
  atom.arraystretch = stretch;
  try {
    return render();
  } finally {
    delete atom.arraystretch;
  }
}

/** Starts the import as a Live editor mounts, so the first click into maths
 *  finds MathLive ready. */
const loader = ViewPlugin.fromClass(
  class {
    constructor(readonly view: EditorView) {
      if (loadState !== "loading") return;
      waiting.add(view);
      void loadMathLive();
    }
    destroy() {
      waiting.delete(this.view);
    }
  },
);

// ── Inline shortcuts ──────────────────────────────────────────────────────

/** MathLive defaults that turn ordinary letter runs (variable names, prose)
 *  into units, words or rare function aliases. */
const PRUNED = [
  "in", "!in", "of", "and", "or", "not", "sub", "sup", "sube", "supe", "mod", "(mod",
  "mm", "cm", "km", "kg", "ft", "inch", "mi", "ii", "jj", "ee", "dx", "dy", "dt", "xin", "sint",
  "ch", "sh", "th", "tg", "ctg", "cth", "cotg", "arctg", "lg", "lb", "cosec",
  "mean", "median", "fft", "lcm", "erf", "erfc", "bessel", "randomReal", "randomInteger",
  "approaches", "union", "asterisk", "divide", "infinity", "defint", "times", "prop",
  "diamond", "square", "lt", "lt=", "gt", "gt=", "ceil", "floor", "frac", "cbrt", "grad",
  "del", "TT", "AA", "EE", "!EE", "Re", "Im",
];

let shortcutTable: InlineShortcutDefinitions | null = null;

/** MathLive's table, pruned, with the note's own shorthands (`@a` → α,
 *  `sr` → ², `->` → →) on top. */
function shortcuts(defaults: Readonly<InlineShortcutDefinitions>): InlineShortcutDefinitions {
  if (shortcutTable) return shortcutTable;
  const out: InlineShortcutDefinitions = { ...defaults };
  for (const key of PRUNED) delete out[key];
  for (const [key, name] of Object.entries(GREEK)) out[`@${key}`] = `\\${name}`;
  for (const [key, latex] of Object.entries(POWERS)) out[key] = latex.replace(/#\{1\}/, "#?").replace(/#\{0\}/, "");
  for (const [key, latex] of OPERATORS) if (!key.startsWith("\\")) out[key] = latex;
  shortcutTable = out;
  return out;
}

// ── Which maths is visual ─────────────────────────────────────────────────

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

function visualOf(state: EditorState, ctx: MathContext): VisualMath | null {
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
 *  whole block: the rendering keeps it (`MathWidget` in `widgets.ts`) rather
 *  than the field taking it, as inline maths keeps a caret on its edge. */
function atBlockEdge(state: EditorState, v: VisualMath): boolean {
  const { main } = state.selection;
  return v.block && (main.from === v.start || main.to === state.doc.lineAt(v.end).to);
}

const cleanCache = new Map<string, boolean>();

/** MathLive reads it without errors and KaTeX renders it: only then does the
 *  field, whose output KaTeX must draw afterwards, get to edit it. */
export function readsCleanly(source: string, display: boolean): boolean {
  if (!lib) return false;
  const key = `${display ? "D" : "I"}${source}`;
  let ok = cleanCache.get(key);
  if (ok === undefined) {
    ok = lib.validateLatex(toField(source, display)).length === 0;
    if (ok) {
      try {
        katex.renderToString(source, { displayMode: display, throwOnError: true, strict: "ignore" });
      } catch {
        ok = false;
      }
    }
    if (cleanCache.size > 500) cleanCache.clear();
    cleanCache.set(key, ok);
  }
  return ok;
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
    if (lib === "ready" && liveFocused(tr.state)) {
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
 * running past it) or while MathLive loads, else as source. A caret inside
 * it before MathLive arrives gets the source, since typing would otherwise
 * land beside the rendering.
 */
export function touchedMath(state: EditorState, start: number, from: number, to: number, display: boolean): "render" | "source" {
  const v = state.field(visualMathField, false);
  if (!v || v.lib === "failed" || v.tex === start) return "source";
  if (v.lib === "loading") return targetAt(state)?.start === start ? "source" : "render";
  return readsCleanly(state.sliceDoc(from, to).trim(), display) ? "render" : "source";
}

/** What the toolbox's mode control offers: "tex" while the field is open,
 *  "visual" for maths switched to TeX that the field can take, else null. */
export function visualToggle(state: EditorState): "tex" | "visual" | null {
  const v = state.field(visualMathField, false);
  if (!v || v.lib !== "ready") return null;
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

// ── Into the field from the note ──────────────────────────────────────────

/** ←/Backspace from just after inline maths, →/Delete from just before it,
 *  open the field at that end instead of stepping over the rendered maths. */
function enterInline(back: boolean): Command {
  return (view) => {
    const { state } = view;
    const v = state.field(visualMathField, false);
    const { ranges, main } = state.selection;
    if (!v || v.lib !== "ready" || ranges.length !== 1 || !main.empty) return false;
    const node = ancestorAt(
      state,
      main.head,
      (n) => n.name === "InlineMath" || (n.name === "BlockMath" && !ownsLines(state, n)),
      [back ? -1 : 1],
    );
    if (!node || (back ? node.to : node.from) !== main.head) return false;
    const ctx = mathContextOf(node);
    const target = ctx && visualOf(state, ctx);
    if (!target || !readsCleanly(state.sliceDoc(target.from, target.to).trim(), target.display)) return false;
    view.dispatch({ selection: { anchor: back ? target.to : target.from }, scrollIntoView: true });
    return true;
  };
}

/** The display block the field can take on the line just above (`up`) or
 *  below the one holding `pos`. */
function blockBeside(state: EditorState, pos: number, up: boolean): VisualMath | null {
  const ln = state.doc.lineAt(pos);
  if (up ? ln.number === 1 : ln.number === state.doc.lines) return null;
  const next = state.doc.line(ln.number + (up ? -1 : 1));
  const node = ancestorAt(state, up ? next.to : next.from, (n) => n.name === "BlockMath", [up ? -1 : 1]);
  if (!node || !ownsLines(state, node)) return null;
  if (up ? state.doc.lineAt(node.to).number !== next.number : node.from !== next.from) return null;
  const ctx = mathContextOf(node);
  const target = ctx && visualOf(state, ctx);
  return target && readsCleanly(state.sliceDoc(target.from, target.to).trim(), true) ? target : null;
}

/** Up to a display block from the line beside it. ↑/↓ off the edge line go
 *  into the field, since block widgets don't hold the caret and vertical
 *  motion would step over them. ← at a line's start or → at its end, and
 *  Backspace/Delete (`deleting`) from a line with text, which would join it
 *  onto the `$$`, stop at the block's edge, beside its rendering, where
 *  Enter or typing adds a line; the same key again enters the field
 *  (`intoMathBlock` in `livePreview.ts`). */
function enterBlock(dir: "up" | "down" | "left" | "right", deleting = false): Command {
  const back = dir === "up" || dir === "left";
  return (view) => {
    const { state } = view;
    const v = state.field(visualMathField, false);
    const { ranges, main } = state.selection;
    if (!v || v.lib !== "ready" || ranges.length !== 1 || !main.empty) return false;
    const ln = state.doc.lineAt(main.head);
    if (dir === "left" || dir === "right") {
      if (main.head !== (back ? ln.from : ln.to) || (deleting && !ln.length)) return false;
    } else {
      const next = view.moveVertically(EditorSelection.cursor(main.head), !back).head;
      if (back ? next >= ln.from : next <= ln.to) return false;
    }
    const target = blockBeside(state, main.head, back);
    if (!target) return false;
    const anchor =
      dir === "left" ? state.doc.lineAt(target.end).to
      : dir === "right" ? target.start
      : back ? target.to : target.from;
    view.dispatch({ selection: { anchor }, scrollIntoView: true });
    return true;
  };
}

const entryKeys = Prec.highest(
  keymap.of([
    { key: "ArrowLeft", run: (view) => enterInline(true)(view) || enterBlock("left")(view) },
    { key: "Backspace", run: (view) => enterInline(true)(view) || enterBlock("left", true)(view) },
    { key: "ArrowRight", run: (view) => enterInline(false)(view) || enterBlock("right")(view) },
    { key: "Delete", run: (view) => enterInline(false)(view) || enterBlock("right", true)(view) },
    { key: "ArrowUp", run: enterBlock("up") },
    { key: "ArrowDown", run: enterBlock("down") },
  ]),
);

/** Where the last press on rendered maths landed, as a fraction of its
 *  `.ML__latex` box, so the field can put its caret there once it replaces
 *  the rendering, which it lays out the same (`staticMath`). A KaTeX
 *  rendering's ink stands in, spaced narrower than MathLive's. */
let pressed: { fx: number; fy: number; at: number } | null = null;

export function noteMathPress(x: number, y: number, rendered: HTMLElement) {
  const ml = rendered.querySelector(".ML__latex");
  const ink = ml
    ? [ml.getBoundingClientRect()]
    : [...rendered.querySelectorAll(".katex-html > .base")].map((b) => b.getBoundingClientRect());
  const r = ink.length ? ink : [rendered.getBoundingClientRect()];
  const left = Math.min(...r.map((b) => b.left));
  const top = Math.min(...r.map((b) => b.top));
  const width = Math.max(...r.map((b) => b.right)) - left;
  const height = Math.max(...r.map((b) => b.bottom)) - top;
  pressed = { fx: width ? (x - left) / width : 0, fy: height ? (y - top) / height : 0.5, at: Date.now() };
}

// ── The field ─────────────────────────────────────────────────────────────

/** LaTeX as the note keeps it: placeholders already gone, `\operatorname`
 *  without MathLive's inner `\mathrm`, and nothing that would end inline
 *  maths early (`x\ $` reads as an escaped space before the `$`). */
export function tidy(latex: string, inline: boolean): string {
  const out = latex
    .replace(/\\operatorname(\*?)\{\\mathrm\{([^{}]*)\}\}/g, "\\operatorname$1{$2}")
    .trimStart()
    .replace(/(?<!\\)\s+$/, "");
  return inline && /\\\s$/.test(out) ? `${out}{}` : out;
}

/** `body` split at its own top-level `\\` (row gaps kept), or null when its
 *  braces or environments don't balance. */
function splitRows(body: string): { rows: string[]; seps: string[] } | null {
  const rows: string[] = [];
  const seps: string[] = [];
  let depth = 0;
  let env = 0;
  let cur = 0;
  for (let i = 0; i < body.length; i++) {
    const c = body[i];
    if (c === "{") depth++;
    else if (c === "}" && --depth < 0) return null;
    else if (c === "\\") {
      if (body.startsWith("\\begin{", i)) env++;
      else if (body.startsWith("\\end{", i) && --env < 0) return null;
      else if (body[i + 1] === "\\" && depth === 0 && env === 0) {
        const gap = /^\\\\\s*(?:\[[^\]]*\])?/.exec(body.slice(i))![0];
        rows.push(body.slice(cur, i).trim());
        seps.push(gap.replace(/\s+/g, ""));
        i += gap.length - 1;
        cur = i + 1;
        continue;
      }
      i++;
    }
  }
  if (depth !== 0 || env !== 0) return null;
  rows.push(body.slice(cur).trim());
  return { rows, seps };
}

/** Rows of a whole-value environment, or null when it isn't one. */
function rowsOf(latex: string): { open: string; rows: string[]; seps: string[]; close: string } | null {
  const m = /^(\\begin\{([a-zA-Z]+\*?)\})([\s\S]*)(\\end\{\2\})$/.exec(latex);
  if (!m) return null;
  let open = m[1];
  let body = m[3];
  // Column spec of the environments that take one.
  const spec = /^(?:array|alignedat|subarray)$/.test(m[2]) ? /^\{[^{}]*\}/.exec(body) : null;
  if (spec) {
    open += spec[0];
    body = body.slice(spec[0].length);
  }
  const split = splitRows(body);
  return split && { open, ...split, close: m[4] };
}

/** A display block one row per line, so Raw mode and diffs read it: an
 *  environment's rows, or the block's own top-level `\\` lines. */
export function layoutBlock(latex: string): string {
  const join = (rows: string[], seps: string[]) =>
    rows.map((row, i) => (i < seps.length ? `${row} ${seps[i]}` : row)).join("\n");
  const env = rowsOf(latex);
  if (env) return env.rows.length < 2 ? latex : `${env.open}\n${join(env.rows, env.seps)}\n${env.close}`;
  const top = splitRows(latex);
  return top && top.rows.length > 1 ? join(top.rows, top.seps) : latex;
}

const WHOLE_ENV = /^\\begin\{([a-zA-Z]+\*?)\}[\s\S]*\\end\{\1\}$/;

/** Display LaTeX as the field holds it: top-level `\\` lines, which KaTeX
 *  draws but MathLive rejects bare, go inside MathLive's `\displaylines`.
 *  `fromField` takes the wrapper off again, so the note keeps bare lines. */
export function toField(source: string, display: boolean): string {
  if (!display || WHOLE_ENV.test(source)) return source;
  const top = splitRows(source);
  return top && top.rows.length > 1 ? `\\displaylines{${source}}` : source;
}

export function fromField(latex: string): string {
  return latex.startsWith("\\displaylines{") && latex.endsWith("}") ? latex.slice(14, -1).trim() : latex;
}

/** MathLive's static markup by display flag and source; a note re-renders often. */
const staticCache = new Map<string, string | null>();

/** MathLive's markup for maths the field could open (`readsCleanly`), else
 *  null. It goes through the patched array atom, as the field does. */
function renderStatic(source: string, display: boolean): string | null {
  if (!lib || !readsCleanly(source, display)) return null;
  const key = `${display ? "D" : "I"}${source}`;
  let html = staticCache.get(key);
  if (html === undefined) {
    try {
      html = lib.convertLatexToMarkup(toField(source, display), { defaultMode: display ? "math" : "inline-math" });
    } catch {
      html = null;
    }
    if (staticCache.size >= 500) staticCache.clear();
    staticCache.set(key, html);
  }
  return html;
}

/** Maths drawn by MathLive without a field, in the box a field opened on it
 *  takes (`.cm-math-ml` in `theme.ts`), or null to draw it with KaTeX. */
export function staticMath(source: string, display: boolean): HTMLElement | null {
  const html = renderStatic(source, display);
  if (!html) return null;
  const dom = document.createElement("span");
  dom.className = "cm-math-ml";
  dom.innerHTML = html;
  return dom;
}

/** Pasted text that is all maths: one delimited `$…$`, `$$…$$`, `\(…\)` or
 *  `\[…\]`, or LaTeX with no delimiters in it. */
function mathOnly(text: string): boolean {
  const t = text.trim();
  if (!/\$|(?<!\\)\\[([]/.test(t)) return true;
  return (
    /^\$\$(?:(?!\$\$)[\s\S])*\$\$$/.test(t) ||
    /^\$[^$]+\$$/.test(t) ||
    /^\\\[(?:(?!\\\])[\s\S])*\\\]$/.test(t) ||
    /^\\\((?:(?!\\\))[\s\S])*\\\)$/.test(t)
  );
}

let rowSheet: CSSStyleSheet | null = null;

/** Centres `\displaylines` rows, as KaTeX centres a display block's bare
 *  `\\` lines: MathLive left-aligns its root `lines` table (the one root
 *  table with a lone left column), in the shadow root `::part` can't reach. */
export function centredRows(mf: MathfieldElement) {
  const root = mf.shadowRoot;
  if (!root || !("adoptedStyleSheets" in root)) return;
  if (!rowSheet) {
    rowSheet = new CSSStyleSheet();
    rowSheet.replaceSync(
      ".ML__latex > .ML__multiline_environment { justify-content: safe center; }\n" +
        ".ML__latex > .ML__mtable > .col-align-l:only-child > .ML__vlist-t { text-align: center; }",
    );
  }
  root.adoptedStyleSheets = [...root.adoptedStyleSheets, rowSheet];
}

/** The slice of MathLive's internal model `caretAt`, `wholeStructures`,
 *  `dropEmptyScript`, `atomsOf` and the matrix keys (`mathMatrixField.ts`)
 *  read. */
export interface MlAtom {
  type: string;
  command: string;
  value: string | undefined;
  mode: string;
  parent: MlAtom | undefined;
  parentBranch: unknown;
  captureSelection: boolean;
  leftSibling: MlAtom | undefined;
  rightSibling: MlAtom | undefined;
  environmentName?: string;
  /** A `leftright` atom's delimiters; `?` is a right one not yet typed. */
  leftDelim?: string;
  rightDelim?: string;
  /** An array's cells, row by row, each its atoms from a `first`. */
  rows?: (MlAtom[] | undefined)[][];
  body?: MlAtom[];
  /** Set, drops the cached LaTeX of the atom and its ancestors. */
  isDirty: boolean;
  hasChildren: boolean;
  /** Alone in its branch: for a `first` atom, the branch is empty. */
  hasNoSiblings: boolean;
  hasEmptyBranch(branch: string): boolean;
  branch(name: string): MlAtom[] | undefined;
}

export interface MlModel {
  at(offset: number): MlAtom | undefined;
  offsetOf(atom: MlAtom): number;
}

export function modelOf(mf: MathfieldElement): MlModel | null {
  const model = (mf as unknown as { _mathfield?: { model?: MlModel } })._mathfield?.model;
  return model && typeof model.at === "function" ? model : null;
}

/** The field's atoms in offset order, each as a comparable key. */
function atomsOf(mf: MathfieldElement): string[] {
  const model = modelOf(mf);
  const keys: string[] = [];
  for (let i = 0; model && i <= mf.lastOffset; i++) {
    const a = model.at(i);
    keys.push(a ? `${a.type}\0${a.command}\0${a.value ?? ""}` : "");
  }
  return keys;
}

type TextStyle = { fontSeries?: "b"; fontShape?: "it" };

/** Commands whose argument is text: committing one bare, or its palette
 *  entry, starts typing text in that style (`startText`). */
const TEXT_COMMANDS: Record<string, TextStyle> = {
  "\\text": {},
  "\\textrm": {},
  "\\textnormal": {},
  "\\mbox": {},
  "\\textbf": { fontSeries: "b" },
  "\\textit": { fontShape: "it" },
};

/** How far past its ink a slot (a script, a numerator) still takes a click. */
const SLOT_REACH = 4;

export interface Box {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

const union = (a: Box, b: Box): Box => ({
  left: Math.min(a.left, b.left),
  top: Math.min(a.top, b.top),
  right: Math.max(a.right, b.right),
  bottom: Math.max(a.bottom, b.bottom),
});

interface Slot extends Box {
  root: boolean;
  /** A row of a root `lines` table (a block's top-level `\\` lines). */
  line: boolean;
  /** Each caret offset in the slot and the x it sits at. */
  carets: { offset: number; x: number }[];
}

/**
 * The caret offset for a click, or null to keep MathLive's. MathLive's
 * hit-test sends a click on an operator with scripts (`\cos^2`), or between
 * two atoms, to the front of the field. Here the click picks the innermost
 * slot (a script, a numerator, a cell, else the top level) whose box, a
 * little widened, holds it, and the caret gap in that slot nearest its x.
 * In a block of several lines, the row whose band holds the click (else the
 * nearest band) bounds that search, and a click in no slot of it, however
 * far beside it, takes the row's nearest gap — its start or end. A click
 * beside one-line maths, or inside a matrix but in no cell, keeps MathLive's
 * (its row logic, the field's two ends).
 */
export function caretAt(mf: MathfieldElement, x: number, y: number): number | null {
  const model = modelOf(mf);
  if (!model) return null;
  const slots = new Map<MlAtom, Map<string, Slot>>();
  const arrays: Box[] = [];
  for (let i = 0; i <= mf.lastOffset; i++) {
    const atom = model.at(i);
    const parent = atom?.parent;
    if (!atom || !parent) continue;
    // A bare `x^2` keeps its scripts in a box-less atom after the `x`, whose
    // caret sits past them; its slots came first, as children do.
    const own = [...(slots.get(atom)?.values() ?? [])];
    const r: Box | undefined = mf.getElementInfo(i)?.bounds ?? (own.length ? own.reduce<Box>(union, own[0]) : undefined);
    if (!r) continue;
    if (atom.type === "array") arrays.push(r);
    let captured = false;
    for (let a: MlAtom | undefined = parent; a; a = a.parent) if (a.captureSelection) captured = true;
    if (captured) continue;
    let byBranch = slots.get(parent);
    if (!byBranch) slots.set(parent, (byBranch = new Map()));
    const key = JSON.stringify(atom.parentBranch);
    // `union` copies: a DOMRect spreads to nothing.
    const prev = byBranch.get(key);
    const root = !parent.parent;
    const slot: Slot = { ...union(prev ?? r, r), root, line: root && parent.type === "array", carets: prev?.carets ?? [] };
    byBranch.set(key, slot);
    // A slot's leading `first` atom is the caret before its content.
    slot.carets.push({ offset: i, x: atom.type === "first" ? r.left : r.right });
  }
  let row: Slot | null = null;
  let gap = Infinity;
  for (const byBranch of slots.values()) {
    for (const s of byBranch.values()) {
      const d = y < s.top ? s.top - y : y > s.bottom ? y - s.bottom : 0;
      if (s.line && d < gap) [row, gap] = [s, d];
    }
  }
  let best: Slot | null = null;
  let area = Infinity;
  for (const byBranch of slots.values()) {
    for (const s of byBranch.values()) {
      // The top level takes any height, so the field's padding reaches it.
      const reach = s.root ? 0 : SLOT_REACH;
      if (x < s.left - reach || x > s.right + reach || (!s.root && (y < s.top || y > s.bottom))) continue;
      if (row && (s.line ? s !== row : s.bottom < row.top || s.top > row.bottom)) continue;
      const a = (s.right - s.left) * (s.bottom - s.top);
      if (a < area) [best, area] = [s, a];
    }
  }
  best ??= row;
  if (!best) return null;
  if (best.root && arrays.some((r) => x >= r.left && x <= r.right && y >= r.top && y <= r.bottom)) return null;
  let pick = best.carets[0];
  for (const c of best.carets) if (Math.abs(c.x - x) < Math.abs(pick.x - x)) pick = c;
  return pick.offset;
}

/**
 * A range whose ends sit at different depths, widened so both ends share a
 * branch and every structure it reaches into is taken whole. MathLive's
 * offsets put a matrix's cells before the matrix itself, so a drag from
 * beside one into a cell selects the cells but not the matrix. Ends in two
 * cells take the whole matrix. Null when the ends already share a branch.
 */
export function wholeStructures(model: MlModel, start: number, end: number): [number, number] | null {
  const chain = (atom: MlAtom | undefined) => {
    const out: MlAtom[] = [];
    for (let a = atom; a?.parent; a = a.parent) out.push(a);
    return out;
  };
  // The root `lines` table's rows read as one run, as the note's lines do.
  const branch = (a: MlAtom) => (a.parent?.environmentName === "lines" ? "lines" : JSON.stringify(a.parentBranch));
  const from = chain(model.at(start));
  const to = chain(model.at(end));
  for (const [i, a] of from.entries()) {
    const j = to.findIndex((b) => b.parent === a.parent && branch(b) === branch(a));
    if (j < 0) continue;
    if (i === 0 && j === 0) return null;
    // An offset is the caret after its atom: start before the lifted atom,
    // end after it.
    const left = i === 0 ? start : a.leftSibling && model.offsetOf(a.leftSibling);
    const right = j === 0 ? end : model.offsetOf(to[j]);
    return left == null || left < 0 || right < 0 ? null : [left, right];
  }
  return null;
}

/** Clipboard type of a block field's copy: the block's source, `$$` lines
 *  included, for a paste outside maths (`livePreview.ts`). */
export const BLOCK_MATH_TYPE = "application/x-oculus-math-block";

/** The one field per editor, for the toolbox. */
const fields = new WeakMap<EditorView, FieldController>();

/** Keys the toolbox (`mathTools.ts`) takes in the field ahead of the field's
 *  own: Space for its quick picks, its shortcut, keys while it is open. A
 *  handler returns true when it took the key. */
export const fieldKeys = Facet.define<(view: EditorView, e: KeyboardEvent, field: FieldController) => boolean>();

export function activeMathField(view: EditorView): FieldController | null {
  return fields.get(view) ?? null;
}

export class FieldController {
  readonly dom: HTMLElement;
  readonly mf: MathfieldElement;
  /** The LaTeX (trimmed, as the widget sees it) last written or loaded, so
   *  the field's own writes don't echo back into it. */
  private shown: string;
  /** The value last handed to MathLive or last flushed, which `getValue`
   *  returns verbatim until an edit: unchanged, there is nothing to write. */
  private loaded: string;
  private tabbing = false;
  private tabFailed = false;
  /** In MathLive's command mode (`\lam…`), and just out of it. */
  private typingCommand = false;
  private committing = false;
  /** Unmounted: MathLive's late `input` must not write. */
  private dead = false;
  /** The maths drawn statically, holding the field's box until it renders. */
  private standIn: HTMLElement | null;
  /** The prompt to Space for the toolbox on an empty line: centred on the
   *  line in a block, whose caret is hidden then, after the empty field in
   *  flow inline. The frame its placement waits for. */
  private hint: HTMLElement | null = null;
  private hintFrame = 0;

  constructor(
    readonly view: EditorView,
    source: string,
    readonly display: boolean,
    readonly block: boolean,
    /** The maths this field edits (`ActiveMath.id`), for its whole life. */
    readonly id: number,
  ) {
    const MF = lib!.MathfieldElement;
    this.dom = document.createElement(block ? "div" : "span");
    this.dom.className = block ? "cm-math-field cm-math-field-block" : "cm-math-field";
    const mf = new MF();
    this.mf = mf;
    if (block) centredRows(mf);
    this.loaded = toField(source, display);
    mf.value = this.loaded;
    this.shown = source;
    // MathLive draws a field a frame after it connects; until `mount` has it
    // render, the static rendering keeps its box, or the note would shrink
    // and a layout read then would clamp a page scrolled to its end.
    this.standIn = staticMath(source, display);
    if (this.standIn) {
      this.dom.classList.add("cm-math-field-mounting");
      this.dom.append(this.standIn);
    }
    this.dom.append(mf);
    this.hint = document.createElement("span");
    this.hint.className = "cm-math-hint";
    this.hint.textContent = block ? "Start typing or Space (␣) for math tools" : "Space (␣) for math tools";
    this.hint.hidden = true;
    this.dom.append(this.hint);

    mf.addEventListener("input", (e) => {
      this.committed(e as InputEvent);
      this.flush();
      this.syncHint();
    });
    mf.addEventListener("move-out", (e) => this.moveOut(e as CustomEvent<{ direction: Direction }>));
    mf.addEventListener("focusout", () => this.focusLeft());
    // MathLive's command list sits where the toolbox does; one at a time.
    mf.addEventListener("mode-change", () => this.modeChanged());
    mf.addEventListener("selection-change", () => {
      this.selectionChanged();
      this.syncHint();
    });
    // Capture, so these keys never reach MathLive's own handling.
    this.dom.addEventListener("keydown", (e) => this.key(e), true);
    // Undo from the Edit menu arrives as `beforeinput`, not a key.
    this.dom.addEventListener("beforeinput", (e) => this.historyInput(e), true);
    mf.addEventListener("beforeinput", (e) => this.committingText(e as InputEvent));
    // Bubbling, so MathLive (in its shadow root) has placed the caret first.
    this.dom.addEventListener("pointerdown", (e) => this.press(e));
    this.dom.addEventListener("mousedown", (e) => this.pressBeside(e));
    this.dom.addEventListener("paste", (e) => this.paste(e), true);
    // Bubbling, after MathLive has put its LaTeX on the clipboard.
    this.dom.addEventListener("copy", (e) => this.copied(e));
    this.dom.addEventListener("cut", (e) => this.copied(e));

    fields.set(view, this);
    queueMicrotask(() => this.mount());
  }

  /** Focus and place the caret: where the rendered maths was pressed, else at
   *  the end the note's caret came from. */
  private mount() {
    if (!this.dom.isConnected) {
      this.dropStandIn();
      return;
    }
    const mf = this.mf;
    // Options MathLive accepts only once the element is connected.
    mf.defaultMode = this.display ? "math" : "inline-math";
    mf.mathVirtualKeyboardPolicy = "manual";
    mf.menuItems = [];
    mf.environmentPopoverPolicy = "off";
    mf.inlineShortcuts = shortcuts(mf.inlineShortcuts);
    // The note's history is the one undo (`history`); MathLive's stays unused.
    mf.keybindings = mf.keybindings.filter((k) => k.command !== "undo" && k.command !== "redo");
    mf.onExport = (_mf, latex) => latex;
    // Focus renders the field at once, so it takes over the same box.
    mf.focus();
    this.dropStandIn();
    this.syncHint();
    const target = this.target();
    const sel = this.view.state.selection.main;
    const ink = mf.shadowRoot?.querySelector(".ML__latex")?.getBoundingClientRect();
    if (pressed && ink && Date.now() - pressed.at < 600) {
      const x = ink.left + pressed.fx * ink.width;
      const y = ink.top + pressed.fy * ink.height;
      mf.position = caretAt(mf, x, y) ?? mf.getOffsetFromPoint(x, y);
    } else if (target && !sel.empty && sel.from <= target.from && sel.to >= target.to) {
      mf.select();
    } else {
      mf.position = target && sel.head <= target.from ? 0 : mf.lastOffset;
    }
    pressed = null;
    if (this.block) this.dropBlankLines();
  }

  /** Blank lines in a block's LaTeX (an older note's, another editor's) go
   *  as the field opens on it, the LaTeX itself untouched and outside the
   *  history; the field's own writes never add them (`squeezeBlankLines`). */
  private dropBlankLines() {
    const target = this.target();
    const current = target ? this.view.state.sliceDoc(target.from, target.to) : "";
    if (!target || !current.includes("\n") || !current.trim()) return;
    const tidied = `\n${squeezeBlankLines(current.trim())}\n`;
    const change = minimalChange(current, tidied, target.from);
    if (!change) return;
    this.shown = tidied.trim();
    this.view.dispatch({ changes: change, annotations: [fieldWrite.of(this.id), Transaction.addToHistory.of(false)] });
  }

  /** The field in flow, in place of its static stand-in. */
  private dropStandIn() {
    if (!this.standIn) return;
    // Class first: a layout between the two lines sees both, never neither.
    this.dom.classList.remove("cm-math-field-mounting");
    this.standIn.remove();
    this.standIn = null;
  }

  /** A press in a block's padding, above or below the shaded field: the
   *  caret rests before or after the block, as on its rendering. */
  private pressBeside(e: MouseEvent) {
    if (e.button !== 0 || e.target !== this.dom || !this.block) return;
    e.preventDefault();
    this.flush();
    const target = this.target();
    if (!target) return;
    const above = e.clientY < this.mf.getBoundingClientRect().top;
    const anchor = above ? target.start : this.view.state.doc.lineAt(target.end).to;
    this.view.dispatch({ selection: { anchor }, userEvent: "select.pointer" });
    this.view.focus();
  }

  /** Maths pasted here goes in at the caret (MathLive drops `$` delimiters);
   *  markdown with prose around maths can't live inside maths, so it goes
   *  into the note just after this maths. */
  private paste(e: ClipboardEvent) {
    const data = e.clipboardData;
    const text = data?.getData("text/plain") ?? "";
    if (!data || !text || data.types.includes("application/json+mathlive") || mathOnly(text)) return;
    e.preventDefault();
    e.stopPropagation();
    this.flush();
    const { view } = this;
    const target = this.target();
    if (!target) return;
    const at = target.block ? view.state.doc.lineAt(target.end).to : target.end;
    const insert = target.block ? `\n${text.replace(/^\n+/, "")}` : text;
    view.dispatch({
      changes: { from: at, insert },
      selection: { anchor: at + insert.length },
      scrollIntoView: true,
      userEvent: "input.paste",
    });
    view.focus();
  }

  /** Copied LaTeX as the note would hold it, without the `\displaylines`
   *  wrapper KaTeX can't draw. A block's copy also carries its `$$` lines
   *  (`BLOCK_MATH_TYPE`), so it pastes into the note as a block. */
  private copied(e: ClipboardEvent) {
    const data = e.clipboardData;
    const text = data?.getData("text/plain");
    if (!data || !text) return;
    const latex = fromField(tidy(text, !this.display));
    data.setData("text/plain", latex);
    if (this.display) data.setData(BLOCK_MATH_TYPE, `$$\n${layoutBlock(latex)}\n$$`);
  }

  /** A plain click: our caret (`caretAt`) over the one MathLive placed. */
  private press(e: PointerEvent) {
    if (e.button !== 0 || e.detail > 1 || e.shiftKey || !this.mf.selectionIsCollapsed) return;
    const fixed = caretAt(this.mf, e.clientX, e.clientY);
    if (fixed != null && fixed !== this.mf.position) this.mf.position = fixed;
  }

  /** A selection dragged or extended into or out of a structure takes it
   *  whole (`wholeStructures`), keeping the end being moved as the caret. */
  private selectionChanged() {
    const model = modelOf(this.mf);
    const { ranges } = this.mf.selection;
    if (!model || ranges.length !== 1 || ranges[0][0] === ranges[0][1]) return;
    const [start, end] = ranges[0][0] < ranges[0][1] ? ranges[0] : [ranges[0][1], ranges[0][0]];
    const whole = wholeStructures(model, start, end);
    if (!whole) return;
    const backward = this.mf.position === start;
    this.mf.selection = { ranges: [whole], direction: backward ? "backward" : "forward" };
  }

  /** The doc changed under the field (an undo, say): it shows the note's
   *  LaTeX, the caret where the change was (`caretAfterChange`). */
  sync(source: string) {
    if (source === this.shown) return;
    this.shown = source;
    this.loaded = toField(source, this.display);
    const at = this.mf.position;
    const before = atomsOf(this.mf);
    // Without a mode MathLive takes the caret's: in `\text` it would insert
    // the LaTeX as literal text instead of replacing the value.
    this.mf.setValue(this.loaded, { silenceNotifications: true, mode: "math" });
    if (this.dom.isConnected) this.mf.position = Math.min(caretAfterChange(before, atomsOf(this.mf), at), this.mf.lastOffset);
    this.syncHint();
  }

  /** Nothing typed in the field (a bare `$$` inline pair, a fresh block). */
  isEmpty(): boolean {
    return this.mf.getValue("latex-without-placeholders") === "";
  }

  /** The caret is on a line holding nothing: the whole block when empty, or
   *  one row of its lines (a root `lines` table, whose rows MathLive keeps
   *  as branches of one array atom). */
  private onEmptyLine(): boolean {
    const model = modelOf(this.mf);
    const at = this.mf.position;
    const here = model?.at(at);
    if (!model || !here || here.type !== "first") return false;
    if (here.parent?.type !== "root" && here.parent?.environmentName !== "lines") return false;
    // The row's own atoms, not the next offset's: that steps inside a
    // leading `\left(` and would read a full row as empty.
    return here.hasNoSiblings;
  }

  /** Show the hint on an empty line, hide it elsewhere. Two frames on, once
   *  MathLive has drawn the edit (it draws in a frame of its own), a block's
   *  goes at the height of the caret's line: its empty row's leading atom,
   *  not the hidden caret, which can still be where it was. */
  private syncHint() {
    if (!this.hint || this.hintFrame) return;
    this.hintFrame = requestAnimationFrame(() => {
      this.hintFrame = requestAnimationFrame(() => {
        this.hintFrame = 0;
        this.placeHint();
      });
    });
  }

  private placeHint() {
    const hint = this.hint;
    if (!hint) return;
    const live = !this.dead && this.dom.isConnected && this.mf.mode === "math" && this.onEmptyLine();
    const line = live && this.block ? this.mf.getElementInfo(this.mf.position)?.bounds : null;
    this.dom.classList.toggle("cm-math-field-empty", line != null);
    if (!live || (this.block && !line)) {
      hint.hidden = true;
      return;
    }
    hint.hidden = false;
    if (line) {
      // The row's centre, kept so the hint stays inside the field's box.
      const box = this.dom.getBoundingClientRect();
      const half = hint.offsetHeight / 2;
      const centre = line.top + line.height / 2 - box.top;
      hint.style.top = `${Math.min(Math.max(centre, half), box.height - half)}px`;
    }
  }

  /** The maths this field may write to, or null when it is gone or was
   *  changed from outside since the field last saw it. */
  private target(): ActiveMath | null {
    if (this.dead) return null;
    return writableSpan(this.view.state, visualMath(this.view.state), this.id, this.shown);
  }

  /** MathLive's command list sits where the toolbox does; one at a time.
   *  Leaving command mode inserts what was typed, reported by the next
   *  `input` (a timeout away); a cancelled command reports nothing, so the
   *  window closes after that timeout. */
  private modeChanged() {
    const latex = this.mf.mode === "latex";
    this.view.dom.classList.toggle("cm-math-command", latex);
    if (this.typingCommand && !latex) {
      this.committing = true;
      queueMicrotask(() => window.setTimeout(() => (this.committing = false), 0));
    }
    this.typingCommand = latex;
    this.syncHint();
  }

  /** A command committed from command mode counts toward the toolbox's
   *  Recent row, Popular tab and quick picks (`mathUsage.ts`). */
  private committed(e: InputEvent) {
    // `data` is the inserted LaTeX (WebKit strips `inputType`); keystrokes
    // still queued from command mode carry none.
    const names = this.committing ? e.data?.match(/\\[a-zA-Z]+/g) : null;
    if (!names) return;
    this.committing = false;
    const subject = this.view.state.facet(noteHost).subjectId;
    for (const name of new Set(names)) recordCommand(name, subject);
  }

  /** A bare `\text` committed from command mode inserts nothing (MathLive
   *  drops the empty argument), so text is typed there instead. The commit's
   *  `beforeinput` carries the command, inside the commit itself. */
  private committingText(e: InputEvent) {
    const text = this.committing ? TEXT_COMMANDS[e.data ?? ""] : undefined;
    if (text) queueMicrotask(() => this.startText(text));
  }

  /** Type text at the caret, as inside `\text{}` (or `\textbf{}`…): MathLive's
   *  text mode, which → or Tab at the run's end leaves (`key`). */
  private startText(style: TextStyle) {
    this.mf.executeCommand(["switchMode", "text"]);
    // MathLive's insert style sticks until the caret moves: set it outright.
    this.mf.applyStyle({ fontSeries: "auto", fontShape: "auto", ...style });
  }

  /** In text mode: whether the caret is at the end of its run of text. */
  private atTextEnd(): boolean {
    return modelOf(this.mf)?.at(this.mf.position + 1)?.mode !== "text";
  }

  /** Write the field's LaTeX into the note. MathLive reports edits a tick
   *  late (`input` from a timeout), so leaving the field flushes first.
   *  `isolate` makes the write an undo step of its own (a matrix edit). */
  flush(isolate = false) {
    const { view, mf } = this;
    const target = this.target();
    const value = mf.getValue("latex");
    if (!target || value === this.loaded) return;
    // What the field holds is now what the note holds, so a flush with no edit
    // since (⌘Z's, before it steps the history) writes nothing: a rewrite of
    // the maths in the layout it is tidied to would sit on top of the step
    // being undone, and undo would only revert that.
    this.loaded = value;
    let latex = fromField(tidy(mf.getValue("latex-without-placeholders"), !target.display));
    if (target.block) latex = squeezeBlankLines(layoutBlock(latex));
    const current = view.state.sliceDoc(target.from, target.to);
    // A block's edges are one line break each, however many it had.
    const edge = (ws: string) => (target.block && ws.includes("\n") ? "\n" : ws);
    const lead = edge(/^\s*/.exec(current)![0]);
    const trail = edge(/\s*$/.exec(current.slice(/^\s*/.exec(current)![0].length))![0]);
    let insert = lead + latex + trail;
    const annotations = isolate ? [fieldWrite.of(this.id), isolateHistory.of("full")] : fieldWrite.of(this.id);
    if (target.block && (latex.includes("\n") || !current.trim())) {
      insert = `${lead.includes("\n") ? lead : "\n"}${latex}${trail.includes("\n") ? trail : "\n"}`;
    }
    // An empty `\(\)` (a `$` typed at a line's start) becomes `$…$` once it
    // holds something; the caret stays inside, so the field stays open.
    if (!target.display && !current.trim() && latex && view.state.sliceDoc(target.start, target.from) === "\\(") {
      this.shown = latex;
      view.dispatch({
        changes: { from: target.start, to: target.end, insert: `$${latex}$` },
        selection: { anchor: target.start + 1 },
        userEvent: FIELD_INPUT,
        annotations,
      });
      return;
    }
    const change = minimalChange(current, insert, target.from);
    this.shown = insert.trim();
    if (!change) return;
    view.dispatch({
      changes: change,
      selection: { anchor: target.from },
      userEvent: FIELD_INPUT,
      annotations,
    });
  }

  /** Undo and redo are the note's: pending keystrokes go in first, then the
   *  note's history steps and the field reloads from it (`sync`), or closes
   *  when the step moved the caret out of this maths. */
  private history(redoing: boolean) {
    // A command being typed (`\lam…`) isn't in the note yet.
    if (this.mf.mode === "latex") return;
    this.flush();
    (redoing ? redo : undo)(this.view);
  }

  private historyInput(e: InputEvent) {
    if (e.inputType !== "historyUndo" && e.inputType !== "historyRedo") return;
    e.preventDefault();
    e.stopPropagation();
    this.history(e.inputType === "historyRedo");
  }

  /** Our keys: the toolbox's first (`fieldKeys`), then the matrix keys, leave,
   *  new line, Tab between slots, toggle to TeX, delete the maths when empty,
   *  the note's undo and redo. Commands being typed (`\lam…`) keep MathLive's. */
  private key(e: KeyboardEvent) {
    if (e.isComposing) return;
    const mf = this.mf;
    const typingCommand = mf.mode === "latex";
    const mod = e.metaKey || e.ctrlKey;
    const stop = () => {
      e.preventDefault();
      e.stopPropagation();
    };
    if (this.view.state.facet(fieldKeys).some((take) => take(this.view, e, this))) {
      stop();
      return;
    }
    // Space, `;` and Backspace unshifted; the closing brackets are shifted keys.
    const plain = !mod && !e.altKey && (!e.shiftKey || !/^(?: |;|Backspace)$/.test(e.key));
    const grid = plain ? this.gridStep(e.key) : null;
    if (grid) {
      stop();
      this.editGrid(grid, e.key === ";");
    } else if (e.key === "Escape" && !typingCommand) {
      stop();
      this.leave("forward");
    } else if (e.key === "Enter" && !typingCommand && !mod && !e.altKey) {
      stop();
      // Never a second empty line: Enter on an empty one does nothing.
      if (!this.display) this.leave("forward");
      else if (this.mf.mode !== "math" || !this.onEmptyLine()) this.newLine();
    } else if (
      mf.mode === "text" &&
      !mod &&
      !e.altKey &&
      !e.shiftKey &&
      (e.key === "Tab" || (e.key === "ArrowRight" && mf.selectionIsCollapsed && this.atTextEnd()))
    ) {
      // Out of the text, as → leaves a fraction's slot: maths again.
      stop();
      while (!this.atTextEnd()) mf.executeCommand("moveToNextChar");
      mf.executeCommand(["switchMode", "math"]);
      mf.applyStyle({ fontSeries: "auto", fontShape: "auto" });
    } else if (e.key === "Tab" && !typingCommand && !mod && !e.altKey) {
      stop();
      this.tab(e.shiftKey);
    } else if (e.key === "Backspace" && mod && !e.altKey && !e.shiftKey) {
      stop();
      this.deleteLineBackward();
    } else if (e.key === "Backspace" && !mod && mf.selectionIsCollapsed && !mf.getValue("latex-without-placeholders")) {
      stop();
      this.remove();
    } else if (e.key === "Backspace" && !mod && mf.selectionIsCollapsed && this.emptyScript()) {
      stop();
      this.dropEmptyScript();
    } else if (mod && e.shiftKey && !e.altKey && e.code === "KeyM") {
      stop();
      this.flush();
      if (this.target()) setMathMode(this.view, "tex");
    } else if (mod && !e.altKey && (e.key.toLowerCase() === "z" || (e.key.toLowerCase() === "y" && !e.shiftKey))) {
      stop();
      this.history(e.shiftKey || e.key.toLowerCase() === "y");
    }
  }

  /** A row after the caret's (MathLive splits it in a multi-line
   *  environment); a caret just outside a whole-value environment moves in
   *  first, so the row joins it rather than wrapping it. */
  private newLine() {
    const mf = this.mf;
    if (WHOLE_ENV.test(mf.getValue())) {
      if (mf.position === mf.lastOffset) mf.position = mf.lastOffset - 1;
      else if (mf.position === 0) mf.position = 1;
    }
    mf.executeCommand("addRowAfter");
  }

  /** Tab: the next empty slot, else a `\qquad`. Shift-Tab: the slot before,
   *  else nothing. */
  private tab(back: boolean) {
    this.tabbing = true;
    this.tabFailed = false;
    this.mf.executeCommand(back ? "moveToPreviousPlaceholder" : "moveToNextPlaceholder");
    this.tabbing = false;
    if (this.tabFailed && !back) this.mf.insert("\\qquad", { format: "latex" });
  }

  private moveOut(e: CustomEvent<{ direction: Direction }>) {
    e.preventDefault();
    if (this.tabbing) {
      this.tabFailed = true;
      return;
    }
    // MathLive announces the move after this returns; leaving now would
    // unmount the field and dispose its model under that code.
    const dir = e.detail.direction;
    queueMicrotask(() => {
      if (this.dom.isConnected) this.leave(dir);
    });
  }

  /** Back to the note, the caret just outside the maths on that side. A block
   *  at the very start or end of the note gets a line to land on. */
  leave(dir: Direction) {
    this.flush();
    const { view } = this;
    const target = this.target();
    if (!target) {
      view.focus();
      return;
    }
    const { doc } = view.state;
    const ahead = dir === "forward" || dir === "downward";
    let changes: ChangeSpec | undefined;
    let anchor: number;
    if (target.block) {
      const first = doc.lineAt(target.start);
      const last = doc.lineAt(target.end);
      if (ahead) {
        if (last.to < doc.length) anchor = last.to + 1;
        else {
          changes = { from: doc.length, insert: "\n" };
          anchor = doc.length + 1;
        }
      } else if (first.from > 0) anchor = first.from - 1;
      else {
        changes = { from: 0, insert: "\n" };
        anchor = 0;
      }
    } else {
      anchor = ahead ? target.end : target.start;
      if (dir === "upward" || dir === "downward") {
        const moved = view.moveVertically(EditorSelection.cursor(anchor), ahead).head;
        if (moved < target.start || moved > target.end) anchor = moved;
      }
    }
    view.dispatch({ changes, selection: { anchor }, scrollIntoView: true, userEvent: changes ? "input" : "select" });
    view.focus();
  }

  /** The atom whose script the caret sits in when that script is empty
   *  (`\cos^{}`), else null. */
  private emptyScript(): MlAtom | null {
    const at = modelOf(this.mf)?.at(this.mf.position);
    const branch = at?.parentBranch;
    if (at?.type !== "first" || !at.parent || (branch !== "superscript" && branch !== "subscript")) return null;
    return at.parent.hasEmptyBranch(branch) ? at.parent : null;
  }

  /** Backspace in an empty script drops it, the caret just after its atom.
   *  MathLive drops it too, but an atom left with no branches (`\cos`) gets
   *  the caret at -2, which MathLive counts from the field's end. */
  private dropEmptyScript() {
    const owner = this.emptyScript();
    if (!owner) return;
    this.mf.executeCommand("deleteBackward");
    const at = modelOf(this.mf)?.offsetOf(owner) ?? -1;
    if (!owner.hasChildren && at >= 0) this.mf.position = at;
  }

  /** Mod-Backspace, as in the note: the caret's line goes up to the caret —
   *  from the start of its row (a cell, in an environment's rows) through
   *  the structure the caret is in. At a line's start it is Backspace. */
  private deleteLineBackward() {
    const mf = this.mf;
    if (mf.selectionIsCollapsed && !mf.getValue("latex-without-placeholders")) {
      this.remove();
      return;
    }
    const model = modelOf(mf);
    let atom = mf.selectionIsCollapsed ? model?.at(mf.position) : undefined;
    // An atom of the root, or of a cell of an array that is (`\displaylines`).
    const inLine = (a: MlAtom) => !a.parent?.parent || (a.parent.type === "array" && !a.parent.parent.parent);
    while (atom?.parent && !inLine(atom)) atom = atom.parent;
    let first = atom;
    while (first?.leftSibling) first = first.leftSibling;
    const start = first ? model!.offsetOf(first) : -1;
    const end = atom ? model!.offsetOf(atom) : -1;
    if (start >= 0 && end > start) mf.selection = { ranges: [[start, end]], direction: "backward" };
    mf.executeCommand("deleteBackward");
  }

  /** Backspace in an empty field takes the maths (a block's lines) away. */
  private remove() {
    const { view } = this;
    const target = this.target();
    if (!target) return;
    const { doc } = view.state;
    let from = target.start;
    let to = target.end;
    if (target.block) {
      from = doc.lineAt(target.start).from;
      to = doc.lineAt(target.end).to;
      if (to < doc.length) to++;
      else if (from > 0) from--;
    }
    view.dispatch({ changes: { from, to }, selection: { anchor: from }, userEvent: "delete" });
    view.focus();
  }

  /** Space is free for the toolbox: MathLive ignores it in maths, but types
   *  it in `\text{}` and beside a text atom, and ends a `\command`; in a
   *  matrix or bracket group it may end a cell (`gridStep`). */
  spaceFree(): boolean {
    const model = modelOf(this.mf);
    const at = this.mf.position;
    return (
      this.mf.mode === "math" &&
      model != null &&
      model.at(at - 1)?.mode !== "text" &&
      model.at(at + 1)?.mode !== "text" &&
      !this.gridStep(" ")
    );
  }

  /** What a key does in the matrix or bracket group at the caret
   *  (`mathMatrixField.ts`), or null when it does nothing special there. */
  private gridStep(key: string): GridStep | null {
    const model = modelOf(this.mf);
    return model ? gridKey(this.mf, model, key) : null;
  }

  /** A matrix key's edit, an undo step of its own: pending keystrokes go in
   *  first. A `;` turning a bracket group into a matrix is first typed as a
   *  step of its own, so ⌘Z gives back `f(x;` for maths that meant it. */
  private editGrid(step: GridStep, semicolon: boolean) {
    const model = modelOf(this.mf);
    if (!model) return;
    this.flush();
    if (semicolon && step.at.group) {
      this.mf.insert(";", { format: "latex", mode: "math" });
      this.flush(true);
    }
    applyGridEdit(this.mf, model, step.at, step.edit);
    this.flush(true);
    this.syncHint();
  }

  /** Where the field's caret is drawn, or the selection's end when there is
   *  none, for the quick picks to hang from. */
  caretRect(): Box | null {
    const caret = this.mf.shadowRoot?.querySelector(".ML__caret, .ML__text-caret, .ML__latex-caret");
    const r = caret?.getBoundingClientRect();
    if (r?.height) return { left: r.right, right: r.right, top: r.top, bottom: r.bottom };
    return this.mf.getElementInfo(this.mf.position)?.bounds ?? null;
  }

  /** A palette entry: its `#{}` slots become MathLive placeholders, the first
   *  taking the selection, and the caret lands in the first empty one. */
  insertTemplate(template: string) {
    // MathLive's placeholder in `\text{}` takes maths; type text there instead.
    const text = TEXT_COMMANDS[template.replace(/\{#\{\}\}$/, "")];
    if (text && this.mf.selectionIsCollapsed) {
      this.mf.focus();
      this.startText(text);
      return;
    }
    let n = 0;
    const latex = template.replace(/[#$]\{[^{}]*\}/g, () => (n++ === 0 ? "#0" : "#?"));
    this.mf.insert(latex, { format: "latex", selectionMode: "placeholder", focus: true, scrollIntoView: true });
    this.flush();
  }

  /** Focus left the field for somewhere outside the editor. A window losing
   *  focus keeps it, so the field is there when the window comes back. */
  private focusLeft() {
    window.setTimeout(() => {
      if (!this.dom.isConnected || !document.hasFocus()) return;
      if (this.view.hasFocus || mathFieldFocused()) return;
      this.view.dispatch({ effects: setFocused.of(false) });
    }, 0);
  }

  destroy() {
    this.dead = true;
    cancelAnimationFrame(this.hintFrame);
    if (fields.get(this.view) === this) fields.delete(this.view);
    this.view.dom.classList.remove("cm-math-command");
    // Removed while typing in it (a command elsewhere moved the selection):
    // the note takes the keyboard back rather than the page.
    if (this.dom.contains(document.activeElement) || document.activeElement === this.mf) {
      window.setTimeout(() => {
        if (this.view.dom.isConnected && !mathFieldFocused() && !this.view.hasFocus) this.view.focus();
      }, 0);
    }
  }
}

type Direction = "forward" | "backward" | "upward" | "downward";

const controllers = new WeakMap<HTMLElement, FieldController>();

/** The field in place of a maths node. Keeps its DOM across the doc changes
 *  its own typing causes (`updateDOM`), or MathLive would lose its caret. */
export class MathFieldWidget extends WidgetType {
  constructor(
    readonly source: string,
    readonly display: boolean,
    readonly block: boolean,
    readonly id: number,
  ) {
    super();
  }

  eq(other: MathFieldWidget) {
    return (
      other.source === this.source && other.display === this.display && other.block === this.block && other.id === this.id
    );
  }

  toDOM(view: EditorView) {
    const field = new FieldController(view, this.source, this.display, this.block, this.id);
    controllers.set(field.dom, field);
    return field.dom;
  }

  /** Reused only for the same maths: another maths gets a field of its own. */
  updateDOM(dom: HTMLElement) {
    const field = controllers.get(dom);
    if (!field || field.display !== this.display || field.block !== this.block || field.id !== this.id) return false;
    field.sync(this.source);
    return true;
  }

  destroy(dom: HTMLElement) {
    controllers.get(dom)?.destroy();
  }

  ignoreEvent() {
    return true;
  }

  get estimatedHeight() {
    return this.block ? 56 : -1;
  }
}

/** A selection extended from inside the open field to past its maths starts
 *  at the maths' edge (`selectionPastField`). */
const fieldSelection = EditorState.transactionFilter.of((tr) => {
  if (tr.docChanged || !tr.selection || !tr.isUserEvent("select")) return tr;
  const v = visualMath(tr.startState);
  const sel = v && selectionPastField(tr.startState.doc, v, tr.selection);
  return sel ? [tr, { selection: sel, sequential: true }] : tr;
});

/** The visual-maths state, the loader and the keys into a field. Live mode
 *  only; the decorations are `livePreview.ts`'s. */
/** A block the field leaves loses empty rows at its end (Enter past its last
 *  line), which would draw as a blank line under the formula, where the
 *  caret beside the block then rests. Outside the history, after the update
 *  that closed the field. */
const dropEndRows = EditorView.updateListener.of((u) => {
  const left = visualMath(u.startState);
  if (!left?.block || visualMath(u.state)?.id === left.id) return;
  const start = u.changes.mapPos(left.start, 1);
  queueMicrotask(() => {
    const { state } = u.view;
    const node = ancestorAt(state, start, (n) => n.name === "BlockMath", [1]);
    const ctx = node && node.from === start ? mathContextOf(node) : null;
    if (!ctx || visualMath(state)?.start === ctx.start) return;
    const latex = state.sliceDoc(ctx.from, ctx.to);
    const change = minimalChange(latex, withoutEndRows(latex), ctx.from);
    if (change) u.view.dispatch({ changes: change, annotations: Transaction.addToHistory.of(false) });
  });
});

/** Inline maths left empty (`$$`, `$ $`, `\(\)`) goes when the field closes
 *  with the caret outside it, rather than staying as an invisible pair. A
 *  caret still inside (TeX mode, the window losing focus) keeps it. Outside
 *  the history, as `dropEndRows`. */
const dropEmptyInline = EditorView.updateListener.of((u) => {
  const left = visualMath(u.startState);
  if (!left || left.block || visualMath(u.state)?.id === left.id) return;
  const start = u.changes.mapPos(left.start, 1);
  const end = u.changes.mapPos(left.end, -1);
  queueMicrotask(() => {
    const { state } = u.view;
    if (end > state.doc.length || !/^(?:\$\s*\$|\\\(\s*\\\))$/.test(state.sliceDoc(start, end))) return;
    if (state.selection.ranges.some((r) => r.to > start && r.from < end)) return;
    u.view.dispatch({ changes: { from: start, to: end }, annotations: Transaction.addToHistory.of(false) });
  });
});

export function mathField(): Extension {
  return [visualMathField, loader, entryKeys, fieldSelection, dropEndRows, dropEmptyInline];
}
