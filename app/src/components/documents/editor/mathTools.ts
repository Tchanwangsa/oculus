import {
  snippet,
  snippetCompletion,
  completionStatus,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from "@codemirror/autocomplete";
import {
  StateEffect,
  StateField,
  type ChangeDesc,
  type EditorState,
  type Extension,
  type Transaction,
} from "@codemirror/state";
import {
  EditorView,
  keymap,
  repositionTooltips,
  showTooltip,
  type Rect,
  type Tooltip,
  type TooltipView,
  type ViewUpdate,
} from "@codemirror/view";
import katex from "katex";
import { syncScrollFade } from "@/lib/scrollFade";
import { noteHost } from "./host";

import { mathFieldFocused } from "./liveFocus";
import { continuation, mathAt, mathContextOf } from "./mathContext";
import { ancestorAt } from "./syntax";
import { activeMathField, fieldKeys, setMathMode, visualToggle, type FieldController } from "./mathField";
import {
  COMMON_COMMANDS,
  MATH_COMMANDS,
  MATH_TABS,
  matrixTemplate,
  previewOf,
  snippetTemplate,
  type MathEntry,
  type MatrixKind,
} from "./mathPalette";
import {
  commandOf,
  popularEntries,
  quickPicks,
  readRecents,
  recordCommand,
  recordUse,
  usageVersion,
} from "./mathUsage";

/**
 * The maths toolbox: a popover centred under the maths the caret is in (live
 * KaTeX preview, recents, a tabbed palette, an inline/block switch and a
 * close button), and `\` completion inside maths. Both insert `snippet()`s,
 * so a template's `{}` slots are Tab fields. The popover opens only when
 * asked — Σ or Mod-Shift-Space in maths — and stays until Esc, its close
 * button or the caret leaving that maths. Under Live mode's MathLive field
 * (`mathField.ts`) it is lighter — no preview — and its cells insert into the
 * field; a control switches between the field and TeX. There Space first
 * opens quick picks at the field's caret: the five entries last used in the
 * note's subject, keyed 1–5, and a way on to the popover. The palette's
 * data is `mathPalette.ts`; the look is `theme.ts`.
 */

// ── KaTeX ─────────────────────────────────────────────────────────────────

/** Button and completion previews, by LaTeX; the set is small and fixed. */
const previewCache = new Map<string, string>();

function previewHtml(latex: string): string {
  let html = previewCache.get(latex);
  if (html === undefined) {
    html = katex.renderToString(latex, { throwOnError: false });
    previewCache.set(latex, html);
  }
  return html;
}

// ── Where the caret's maths is ────────────────────────────────────────────

/** The LaTeX range (`from`–`to`) and the whole node, delimiters included. */
interface MathRange {
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

/** "quick": the visual field's quick picks; "full": the popover. */
export type ToolsKind = "quick" | "full";

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

/** Open the quick picks or the popover on the caret's maths, or close. */
export const openMathTools = StateEffect.define<ToolsKind | null>();

const mathToolsField = StateField.define<ToolsState>({
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
 *  while it stays open; another for the quick picks. */
const createTools = (view: EditorView): TooltipView => new MathToolsView(view);
const createQuick = (view: EditorView): TooltipView => new QuickPicksView(view);

function toolsTooltip(open: ToolsOpen): Tooltip {
  return { pos: open.nodeFrom, above: false, create: open.kind === "full" ? createTools : createQuick };
}

/** Σ and Mod-Shift-Space in maths: open the popover, or close it. False when
 *  the caret isn't in maths. */
export function toggleMathTools(view: EditorView): boolean {
  const v = view.state.field(mathToolsField, false);
  if (!v?.math) return false;
  view.dispatch({ effects: openMathTools.of(v.open?.kind === "full" ? null : "full") });
  if (!view.hasFocus && !mathFieldFocused()) view.focus();
  return true;
}

// ── Inline ⇄ block ────────────────────────────────────────────────────────

/** What the switch offers for the caret's maths: block for inline maths,
 *  inline for a block with LaTeX in it, nothing inside a table row (a block
 *  would break it). */
function shapeToggle(state: EditorState, math: MathRange): "block" | "inline" | null {
  if (ancestorAt(state, math.nodeFrom, (n) => n.name === "Table")) return null;
  if (!math.display) return "block";
  return state.sliceDoc(math.from, math.to).trim() ? "inline" : null;
}

/** The caret's maths as a block on lines of its own, or as inline maths
 *  (`shapeChange`). The caret lands at the end of the LaTeX. */
function toggleShape(view: EditorView) {
  // The field's last keystrokes reach the note first.
  activeMathField(view)?.flush();
  const tools = view.state.field(mathToolsField, false);
  const change = tools?.math && shapeChange(view.state, tools.math);
  if (!change) return;
  // The rewritten maths starts elsewhere; the popover moves onto it.
  const effects = tools.open ? openMathTools.of(tools.open.kind) : [];
  view.dispatch({ changes: change.changes, selection: { anchor: change.caret }, effects, scrollIntoView: true, userEvent: "input" });
  view.focus();
}

/** Rewrite maths as a block on lines of its own (splitting the text around
 *  it) or as inline maths, keeping `\(`/`\[` or `$` delimiters: the change,
 *  the end of the LaTeX (`caret`) and the new maths' span. Null when
 *  `shapeToggle` offers nothing. */
function shapeChange(
  state: EditorState,
  math: MathRange,
): { changes: { from: number; to: number; insert: string }; caret: number; start: number; end: number } | null {
  const shape = shapeToggle(state, math);
  if (!shape) return null;
  const bracket = state.sliceDoc(math.nodeFrom, math.nodeFrom + 1) === "\\";
  const line = state.doc.lineAt(math.nodeFrom);
  const prefix = continuation(line.text);
  let from = math.nodeFrom;
  let to = math.nodeTo;
  let insert: string;
  let caret: number;
  let start: number;
  let end: number;
  if (shape === "block") {
    const latex = state.sliceDoc(math.from, math.to).trim();
    const nl = `\n${prefix}`;
    const before = state.sliceDoc(line.from, math.nodeFrom);
    const after = state.sliceDoc(math.nodeTo, state.doc.lineAt(math.nodeTo).to);
    insert = "";
    if (before.slice(prefix.length).trim()) {
      from = line.from + before.trimEnd().length;
      insert = nl;
    }
    start = from + insert.length;
    insert += `${bracket ? "\\[" : "$$"}${nl}${latex}`;
    caret = from + insert.length;
    insert += `${nl}${bracket ? "\\]" : "$$"}`;
    end = from + insert.length;
    const rest = after.trimStart();
    if (rest) {
      to = math.nodeTo + after.length - rest.length;
      insert += nl;
    }
  } else {
    // The block's later lines carry the container's markup; inline maths
    // is one line.
    const trimmed = prefix.trimEnd();
    const latex = state
      .sliceDoc(math.from, math.to)
      .split("\n")
      .map((l) => (l.startsWith(prefix) ? l.slice(prefix.length) : l.startsWith(trimmed) ? l.slice(trimmed.length) : l).trim())
      .filter(Boolean)
      .join(" ");
    insert = `${bracket ? "\\(" : "$"}${latex}`;
    let trail = bracket ? "\\)" : "$";
    // A paragraph line right above or below, in the same container, takes
    // the maths back into its sentence.
    const { doc } = state;
    const block = ancestorAt(state, math.nodeFrom, (n) => n.name === "BlockMath", [1]);
    const paragraphAt = (pos: number, side: -1 | 1) => {
      const p = ancestorAt(state, pos, (n) => n.name === "Paragraph", [side]);
      return p?.parent && block?.parent && p.parent.name === block.parent.name && p.parent.from === block.parent.from;
    };
    const first = doc.lineAt(math.nodeFrom);
    const last = doc.lineAt(math.nodeTo);
    if (block && !state.sliceDoc(first.from, math.nodeFrom).slice(prefix.length).trim() && first.number > 1) {
      const above = doc.line(first.number - 1);
      if (above.text.trim() && paragraphAt(above.to, -1)) {
        from = above.from + above.text.trimEnd().length;
        insert = ` ${insert}`;
      }
    }
    if (block && !state.sliceDoc(math.nodeTo, last.to).trim() && last.number < doc.lines) {
      const below = doc.line(last.number + 1);
      const start = below.from + continuation(below.text).length;
      if (below.text.trim() && paragraphAt(start, 1)) {
        to = start;
        trail += " ";
      }
    }
    caret = from + insert.length;
    start = from + (insert.startsWith(" ") ? 1 : 0);
    end = caret + (bracket ? 2 : 1);
    insert += trail;
  }
  return { changes: { from, to, insert }, caret, start, end };
}

// ── The popover ───────────────────────────────────────────────────────────

/** The palette tab and matrix brackets last picked, for the session; the
 *  Popular tab (`popularEntries`) leads and is where a session starts. */
const POPULAR = "popular";
let lastTab = POPULAR;
let matrixKind: MatrixKind = "pmatrix";

const MATRIX_MAX = 6;
const MATRIX_KINDS: { kind: MatrixKind; label: string }[] = [
  { kind: "pmatrix", label: "( )" },
  { kind: "bmatrix", label: "[ ]" },
  { kind: "vmatrix", label: "| |" },
];

function el<K extends keyof HTMLElementTagNameMap>(tag: K, className: string, text?: string) {
  const node = document.createElement(tag);
  node.className = className;
  if (text != null) node.textContent = text;
  return node;
}

/** A strip one row tall that a mouse wheel scrolls sideways. */
function sideways(strip: HTMLElement) {
  strip.addEventListener(
    "wheel",
    (e) => {
      if (Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;
      e.preventDefault();
      strip.scrollLeft += e.deltaY;
    },
    { passive: false },
  );
}

const CLOSE_ICON =
  '<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>';

/** A control that never takes focus: the root's mousedown is cancelled, so
 *  the editor keeps focus and its selection. */
function button(className: string, label: string, onClick: () => void): HTMLButtonElement {
  const b = el("button", className);
  b.type = "button";
  b.tabIndex = -1;
  b.setAttribute("aria-label", label);
  b.title = label;
  b.addEventListener("click", onClick);
  return b;
}

/** A template as the LaTeX it inserts, fields left empty. */
const sourceOf = (template: string) => template.replace(/[#$]\{[^{}]*\}/g, "");

/** A palette entry into the open field (slots become placeholders), else
 *  into the note as a snippet; either way it counts as used. */
function insertEntry(view: EditorView, entry: MathEntry) {
  const field = visualToggle(view.state) === "tex" ? activeMathField(view) : null;
  if (field) field.insertTemplate(entry.template);
  else {
    const { from, to } = view.state.selection.main;
    snippet(snippetTemplate(entry.template))(view, null, from, to);
  }
  recordUse(entry, subjectOf(view));
}

/** The note's subject, whose recents the quick picks show. */
const subjectOf = (view: EditorView) => view.state.facet(noteHost).subjectId;

/** A palette cell: the entry's KaTeX preview, inserting it on click. */
function cellButton(entry: MathEntry, onClick: () => void): HTMLButtonElement {
  const b = button(entry.wide ? "cm-math-cell cm-math-cell-wide" : "cm-math-cell", sourceOf(entry.template), onClick);
  b.innerHTML = previewHtml(previewOf(entry));
  return b;
}

/** Hidden while neither the editor nor its field has focus, and while the
 *  completion list is up. True when it has just come back. */
function syncHidden(view: EditorView, dom: HTMLElement, state: EditorState): boolean {
  const hide = !(view.hasFocus || mathFieldFocused()) || completionStatus(state) === "active";
  const was = dom.classList.contains("cm-math-tools-hidden");
  dom.classList.toggle("cm-math-tools-hidden", hide);
  return was && !hide;
}

class MathToolsView implements TooltipView {
  dom = el("div", "cm-math-tools");
  /** Left out of tooltip stacking: it hides while the completion list is up
   *  rather than pushing the list down. */
  overlap = true;
  offset = { x: 0, y: 6 };
  private preview = el("div", "cm-math-preview");
  private error = el("div", "cm-math-error-text");
  private recents = el("div", "cm-math-recents");
  private recentCells = el("div", "cm-math-recent-cells");
  private tabs = el("div", "cm-math-tabs");
  private body = el("div", "cm-math-body");
  private mode = button("cm-math-pill cm-math-mode", "", () => {
    const next = visualToggle(this.view.state);
    if (next === "tex") activeMathField(this.view)?.flush();
    if (next) setMathMode(this.view, next);
  });
  private shape = button("cm-math-pill cm-math-shape", "", () => toggleShape(this.view));
  private close = button("cm-math-close", "Close (Esc)", () => {
    this.view.dispatch({ effects: openMathTools.of(null) });
  });
  private shown: string | null = null;
  /** `usageVersion()` the Recent row was drawn at. */
  private usage = -1;
  /** The last render that parsed, kept (dimmed) while the LaTeX is broken. */
  private good: { nodeFrom: number; html: string } | null = null;

  constructor(readonly view: EditorView) {
    this.dom.setAttribute("role", "group");
    this.dom.setAttribute("aria-label", "Maths tools");
    this.dom.addEventListener("mousedown", (e) => e.preventDefault());

    this.close.innerHTML = CLOSE_ICON;
    const tabbar = el("div", "cm-math-tabbar");
    tabbar.append(this.tabs, this.mode, this.shape, this.close);
    this.tabs.addEventListener("scroll", () => this.syncTabFades());
    this.recentCells.addEventListener("scroll", () => syncScrollFade(this.recentCells, "x"));
    this.body.addEventListener("scroll", () => this.syncBodyFades());
    sideways(this.tabs);
    sideways(this.recentCells);

    this.dom.append(this.preview, this.error, this.recents, tabbar, this.body);
    this.renderRecents();
    this.renderTabs();
    this.refresh(view.state);
    this.syncMode(view.state);
    this.syncShape(view.state);
  }

  mount() {
    this.syncHidden(this.view.state);
    this.revealTab();
  }

  /** Fade whichever end of the tab strip has more tabs behind it. */
  private syncTabFades() {
    syncScrollFade(this.tabs, "x");
  }

  /** The same for the palette's top and bottom. */
  private syncBodyFades() {
    syncScrollFade(this.body, "y");
  }

  /** Scroll the chosen tab clear of the faded ends. */
  private revealTab() {
    const t = this.tabs;
    const active = t.querySelector<HTMLElement>("[aria-pressed=true]");
    if (active) {
      const pad = 24;
      if (active.offsetLeft - pad < t.scrollLeft) t.scrollLeft = active.offsetLeft - pad;
      else if (active.offsetLeft + active.offsetWidth + pad > t.scrollLeft + t.clientWidth) {
        t.scrollLeft = active.offsetLeft + active.offsetWidth + pad - t.clientWidth;
      }
    }
    this.syncTabFades();
    this.syncBodyFades();
    syncScrollFade(this.recentCells, "x");
  }

  update(u: ViewUpdate) {
    if (u.docChanged || u.selectionSet || u.startState.field(mathToolsField) !== u.state.field(mathToolsField)) {
      this.refresh(u.state);
    }
    // A command typed in the field or the note joins the Recent row.
    if (this.usage !== usageVersion()) this.renderRecents();
    this.syncMode(u.state);
    this.syncShape(u.state);
    this.syncHidden(u.state);
  }

  /** The inline/block switch names the shape it turns the maths into. */
  private syncShape(state: EditorState) {
    const math = state.field(mathToolsField).math;
    const next = math && shapeToggle(state, math);
    this.shape.hidden = next == null;
    const label = next === "inline" ? "Inline" : "Block";
    if (next && this.shape.textContent !== label) {
      this.shape.textContent = label;
      const hint = next === "inline" ? "Make inline maths, in the text line" : "Make block maths, on a line of its own";
      this.shape.title = hint;
      this.shape.setAttribute("aria-label", hint);
    }
  }

  /** Visual (the field is open): no preview, and the control offers TeX.
   *  TeX for maths the field can take: the control offers it back. */
  private syncMode(state: EditorState) {
    const next = visualToggle(state);
    this.dom.classList.toggle("cm-math-tools-visual", next === "tex");
    this.mode.hidden = next == null;
    const label = next === "tex" ? "TeX" : "Visual";
    if (this.mode.textContent !== label) {
      this.mode.textContent = label;
      const hint = next === "tex" ? "Edit as LaTeX source (⌘⇧M)" : "Edit visually (⌘⇧M)";
      this.mode.title = hint;
      this.mode.setAttribute("aria-label", hint);
    }
  }

  /** Centred on the maths — a block on the text column, inline maths on its
   *  span when that is one row, else its start — with the top edge under its
   *  last line, so a long formula or a `$$` block is never covered. */
  getCoords(pos: number): Rect {
    const { view } = this;
    const math = view.state.field(mathToolsField).math;
    const start = view.coordsAtPos(math ? math.nodeFrom : pos, 1);
    const end = math ? view.coordsAtPos(math.nodeTo, -1) : start;
    // Null hides the tooltip, which is right for maths scrolled out of view.
    if (!start || !end) return null as unknown as Rect;
    let centre = start.left;
    if (math?.display) {
      const box = view.contentDOM.getBoundingClientRect();
      const style = getComputedStyle(view.contentDOM);
      centre = (box.left + parseFloat(style.paddingLeft) + box.right - parseFloat(style.paddingRight)) / 2;
    } else if (end.top < start.bottom) {
      centre = (start.left + end.right) / 2;
    }
    const left = centre - this.dom.offsetWidth / 2;
    return { left, right: left + this.dom.offsetWidth, top: start.top, bottom: Math.max(start.bottom, end.bottom) };
  }

  private syncHidden(state: EditorState) {
    // Hidden, it measured zero wide; measure and centre again once it shows.
    if (syncHidden(this.view, this.dom, state)) {
      this.revealTab();
      repositionTooltips(this.view);
    }
  }

  private insert(entry: MathEntry) {
    insertEntry(this.view, entry);
    this.renderRecents();
  }

  private refresh(state: EditorState) {
    const math = state.field(mathToolsField).math;
    if (!math) return;
    const source = state.sliceDoc(math.from, math.to).trim();
    const key = `${math.display ? "D" : "I"}${math.nodeFrom}:${source}`;
    if (key === this.shown) return;
    this.shown = key;
    this.error.textContent = "";
    this.preview.classList.remove("cm-math-preview-stale", "cm-math-preview-empty");
    if (!source) {
      this.preview.textContent = "Empty equation";
      this.preview.classList.add("cm-math-preview-empty");
      this.good = null;
      return;
    }
    try {
      const html = katex.renderToString(source, { displayMode: math.display, throwOnError: true });
      this.preview.innerHTML = html;
      this.good = { nodeFrom: math.nodeFrom, html };
    } catch (e) {
      // Mid-typing LaTeX rarely parses; keep the last good render, dimmed.
      if (this.good?.nodeFrom === math.nodeFrom) {
        this.preview.innerHTML = this.good.html;
        this.preview.classList.add("cm-math-preview-stale");
      } else {
        this.preview.replaceChildren();
      }
      this.error.textContent = (e instanceof Error ? e.message : String(e)).replace(/^KaTeX parse error: /, "");
    }
  }

  private cell(entry: MathEntry): HTMLButtonElement {
    return cellButton(entry, () => this.insert(entry));
  }

  private renderRecents() {
    this.usage = usageVersion();
    const list = readRecents();
    this.recents.hidden = list.length === 0;
    this.recentCells.replaceChildren(...list.map((e) => this.cell(e)));
    this.recentCells.scrollLeft = 0;
    this.recents.replaceChildren(el("span", "cm-math-caption", "Recent"), this.recentCells);
    syncScrollFade(this.recentCells, "x");
  }

  /** Popular is rebuilt only here (opening, picking a tab), so cells never
   *  reorder under the pointer. */
  private renderTabs() {
    const tabs = [{ id: POPULAR, label: "Popular", entries: popularEntries() }, ...MATH_TABS];
    this.tabs.replaceChildren(
      ...tabs.map((tab) => {
        const b = button("cm-math-pill", tab.label, () => {
          lastTab = tab.id;
          this.renderTabs();
          this.body.scrollTop = 0;
          this.revealTab();
        });
        b.textContent = tab.label;
        b.setAttribute("aria-pressed", String(tab.id === lastTab));
        return b;
      }),
    );
    const tab = tabs.find((t) => t.id === lastTab) ?? tabs[0];
    const grid = el("div", "cm-math-grid");
    grid.append(...tab.entries.map((e) => this.cell(e)));
    this.body.replaceChildren(...(tab.id === "matrices" ? [this.matrixPicker()] : []), grid);
  }

  /** Hover an N × M grid, click to insert that matrix with every cell a field. */
  private matrixPicker(): HTMLElement {
    const wrap = el("div", "cm-math-matrix");
    const grid = el("div", "cm-math-matrix-grid");
    const readout = el("span", "cm-math-matrix-size");
    const cells: HTMLButtonElement[] = [];
    const light = (rows: number, cols: number) => {
      readout.textContent = `${rows} × ${cols}`;
      cells.forEach((c, i) => {
        const lit = Math.floor(i / MATRIX_MAX) < rows && i % MATRIX_MAX < cols;
        c.classList.toggle("cm-math-matrix-lit", lit);
      });
    };
    for (let i = 0; i < MATRIX_MAX * MATRIX_MAX; i++) {
      const rows = Math.floor(i / MATRIX_MAX) + 1;
      const cols = (i % MATRIX_MAX) + 1;
      const c = button("cm-math-matrix-cell", `${rows} by ${cols} matrix`, () =>
        this.insert(matrixTemplate(matrixKind, rows, cols)),
      );
      c.addEventListener("mouseenter", () => light(rows, cols));
      cells.push(c);
    }
    grid.append(...cells);
    grid.addEventListener("mouseleave", () => light(2, 2));
    light(2, 2);

    const kinds = el("div", "cm-math-matrix-kinds");
    const renderKinds = () =>
      kinds.replaceChildren(
        ...MATRIX_KINDS.map(({ kind, label }) => {
          const b = button("cm-math-pill", kind, () => {
            matrixKind = kind;
            renderKinds();
          });
          b.textContent = label;
          b.setAttribute("aria-pressed", String(kind === matrixKind));
          return b;
        }),
      );
    renderKinds();

    const side = el("div", "cm-math-matrix-side");
    side.append(readout, kinds);
    wrap.append(grid, side);
    return wrap;
  }
}

// ── Quick picks ───────────────────────────────────────────────────────────

const EXPAND_ICON =
  '<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>';

/**
 * The visual field's quick picks: a strip hanging from the field's caret
 * like a completion list (CodeMirror flips it above near the window's
 * bottom), with the note's subject's most recent entries keyed 1–5 and a
 * button on to the popover. Any caret move in the field closes it; its keys
 * are `fieldKey`'s.
 */
class QuickPicksView implements TooltipView {
  dom = el("div", "cm-math-tools cm-math-quick");
  overlap = true;
  offset = { x: -4, y: 4 };
  private field: FieldController | null;
  /** The field's selection the strip opened at. */
  private at: string;
  /** MathLive reports moves from inside its own updates, and late ones from
   *  before the strip opened; close after them, on a real move. */
  private moved = (e: Event) => {
    if (e.type === "selection-change" && this.selection() === this.at) return;
    queueMicrotask(() => {
      if (mathToolsOpen(this.view.state) === "quick") this.view.dispatch({ effects: openMathTools.of(null) });
    });
  };

  constructor(readonly view: EditorView) {
    this.dom.setAttribute("role", "group");
    this.dom.setAttribute("aria-label", "Quick maths picks");
    this.dom.addEventListener("mousedown", (e) => e.preventDefault());
    quickPicks(subjectOf(view)).forEach((entry, i) => {
      const cell = cellButton(entry, () => pickQuick(view, entry));
      cell.append(el("span", "cm-math-quick-key", String(i + 1)));
      this.dom.append(cell);
    });
    const expand = button("cm-math-close", "All maths tools (Space)", () =>
      view.dispatch({ effects: openMathTools.of("full") }),
    );
    expand.innerHTML = EXPAND_ICON;
    this.dom.append(expand);
    this.field = activeMathField(view);
    this.at = this.selection();
    this.field?.mf.addEventListener("selection-change", this.moved);
    this.field?.dom.addEventListener("pointerdown", this.moved);
  }

  private selection(): string {
    return JSON.stringify(this.field?.mf.selection ?? null);
  }

  mount() {
    this.update();
  }

  update(u?: ViewUpdate) {
    syncHidden(this.view, this.dom, u?.state ?? this.view.state);
  }

  /** Under the field's caret, left edges lined up; right edges instead when
   *  it would run past the edge the editor is clipped at. */
  getCoords(): Rect {
    const caret = activeMathField(this.view)?.caretRect();
    // Null hides the tooltip, as for maths scrolled out of view.
    if (!caret) return null as unknown as Rect;
    const width = this.dom.offsetWidth;
    if (caret.left + this.offset.x + width <= clipRight(this.view.dom)) return caret;
    const left = caret.left - this.offset.x * 2 - width;
    return { ...caret, left, right: left + width };
  }

  destroy() {
    this.field?.mf.removeEventListener("selection-change", this.moved);
    this.field?.dom.removeEventListener("pointerdown", this.moved);
  }
}

/** The right edge `dom` shows to: the window's, or the nearest clipping or
 *  scrolling ancestor's. */
function clipRight(dom: HTMLElement): number {
  let right = dom.ownerDocument.documentElement.clientWidth;
  for (let n: HTMLElement | null = dom; n; n = n.parentElement) {
    if (getComputedStyle(n).overflowX !== "visible") right = Math.min(right, n.getBoundingClientRect().right);
  }
  return right;
}

/** Insert a quick pick into the field, closing the strip. */
function pickQuick(view: EditorView, entry: MathEntry) {
  view.dispatch({ effects: openMathTools.of(null) });
  insertEntry(view, entry);
}

const MODIFIER_KEYS = new Set(["Shift", "Meta", "Control", "Alt", "CapsLock", "Fn"]);

/** Mod-Shift-Space, the popover's key in every mode (`toggleMathTools`). */
const toolsShortcut = (e: KeyboardEvent) => (e.metaKey || e.ctrlKey) && e.shiftKey && !e.altKey && e.code === "Space";

/**
 * The toolbox's keys in the visual field, ahead of the field's own. Space
 * opens the quick picks, then the popover; while the picks are up 1–5
 * insert one and Esc closes them, and any other key closes them and goes on
 * to the field. Esc closes the popover before it leaves the field.
 */
function fieldKey(view: EditorView, e: KeyboardEvent, field: FieldController): boolean {
  const open = mathToolsOpen(view.state);
  if (toolsShortcut(e)) return toggleMathTools(view);
  const close = () => view.dispatch({ effects: openMathTools.of(null) });
  const plain = !e.metaKey && !e.ctrlKey && !e.altKey;
  // A `\command` being typed keeps its keys, Esc included.
  if (field.mf.mode === "latex") {
    if (open === "quick") close();
    return false;
  }
  if (open === "quick") {
    if (MODIFIER_KEYS.has(e.key)) return false;
    if (e.key === " " && plain && !e.shiftKey) {
      view.dispatch({ effects: openMathTools.of("full") });
      return true;
    }
    const pick = plain && /^[1-9]$/.test(e.key) ? quickPicks(subjectOf(view))[Number(e.key) - 1] : undefined;
    if (pick) {
      pickQuick(view, pick);
      return true;
    }
    close();
    return e.key === "Escape";
  }
  if (open === "full" && e.key === "Escape") {
    close();
    return true;
  }
  if (!open && e.key === " " && plain && !e.shiftKey && field.spaceFree() && view.state.field(mathToolsField).math) {
    view.dispatch({ effects: openMathTools.of("quick") });
    return true;
  }
  return false;
}

// ── Shape after a paste ───────────────────────────────────────────────────

/** How long the paste chip stays after its last use, pointer off it. */
const PASTED_LINGER = 5000;

/** Offer the shape switch on the maths starting at a position, or close it. */
const pastedMath = StateEffect.define<number | null>();

/** The pasted maths' start and its chip, until it is dismissed, times out or
 *  the note is edited other than through the chip. */
const pastedField = StateField.define<{ pos: number; tooltip: Tooltip } | null>({
  create: () => null,
  update(prev, tr) {
    for (const e of tr.effects) {
      if (e.is(pastedMath)) return e.value == null ? null : { pos: e.value, tooltip: { pos: e.value, create: createPasted } };
    }
    return tr.docChanged ? null : prev;
  },
  provide: (f) => showTooltip.from(f, (v) => v?.tooltip ?? null),
});

/** One `create`, so CodeMirror keeps the chip's DOM (and its timer) as a
 *  switch moves it onto the rewritten maths. */
const createPasted = (view: EditorView): TooltipView => new PastedView(view);

function mathStartingAt(state: EditorState, pos: number): MathRange | null {
  const node = ancestorAt(state, pos, (n) => (n.name === "InlineMath" || n.name === "BlockMath") && n.from === pos, [1]);
  const ctx = node && mathContextOf(node);
  return ctx && { from: ctx.from, to: ctx.to, display: ctx.display, nodeFrom: ctx.start, nodeTo: ctx.end };
}

/** After a paste of maths ending at `end`: the chip that switches it
 *  between inline and block, when `shapeToggle` offers a switch. */
export function offerShapeSwitch(view: EditorView, end: number) {
  const { state } = view;
  const node = ancestorAt(state, end, (n) => (n.name === "InlineMath" || n.name === "BlockMath") && n.to === end, [-1]);
  const math = node && mathStartingAt(state, node.from);
  if (math && shapeToggle(state, math)) view.dispatch({ effects: pastedMath.of(math.nodeFrom) });
}

/**
 * The chip under pasted maths: "Convert to block" or "Convert to inline",
 * and a close button. A switch keeps it up, offering the way back; it goes
 * `PASTED_LINGER` after its last use with the pointer off it, on Esc, or
 * with any other edit.
 */
class PastedView implements TooltipView {
  dom = el("div", "cm-math-tools cm-math-quick cm-math-pasted");
  private shape = button("cm-math-pill cm-math-shape", "", () => this.toggle());
  private timer = 0;
  private hovered = false;

  constructor(readonly view: EditorView) {
    this.dom.setAttribute("role", "group");
    this.dom.setAttribute("aria-label", "Pasted maths");
    this.dom.addEventListener("mousedown", (e) => e.preventDefault());
    this.dom.addEventListener("pointerenter", () => {
      this.hovered = true;
      window.clearTimeout(this.timer);
    });
    this.dom.addEventListener("pointerleave", () => {
      this.hovered = false;
      this.linger();
    });
    const close = button("cm-math-close", "Dismiss", () => view.dispatch({ effects: pastedMath.of(null) }));
    close.innerHTML = CLOSE_ICON;
    this.dom.append(this.shape, close);
    this.sync(view.state);
    this.linger();
  }

  private math(state: EditorState): MathRange | null {
    const pasted = state.field(pastedField, false);
    return pasted ? mathStartingAt(state, pasted.pos) : null;
  }

  private sync(state: EditorState) {
    const math = this.math(state);
    const shape = math && shapeToggle(state, math);
    this.shape.hidden = !shape;
    const label = shape === "block" ? "Convert to block" : "Convert to inline";
    if (this.shape.textContent !== label) {
      this.shape.textContent = label;
      this.shape.title = label;
      this.shape.setAttribute("aria-label", label);
    }
  }

  update(u: ViewUpdate) {
    if (u.docChanged) this.sync(u.state);
  }

  /** Switch the shape, the caret just after the maths as the paste left it. */
  private toggle() {
    const { state } = this.view;
    const math = this.math(state);
    const change = math && shapeChange(state, math);
    if (!change) return;
    this.view.dispatch({
      changes: change.changes,
      selection: { anchor: change.end },
      effects: pastedMath.of(change.start),
      scrollIntoView: true,
      userEvent: "input",
    });
    this.view.focus();
    this.linger();
  }

  private linger() {
    window.clearTimeout(this.timer);
    if (this.hovered) return;
    this.timer = window.setTimeout(() => this.view.dispatch({ effects: pastedMath.of(null) }), PASTED_LINGER);
  }

  /** Under the maths, at its left edge. */
  getCoords(pos: number): Rect {
    const math = this.math(this.view.state);
    const start = this.view.coordsAtPos(pos, 1);
    const end = math ? this.view.coordsAtPos(math.nodeTo, -1) : start;
    // Null hides the tooltip, as for maths scrolled out of view.
    if (!start || !end) return null as unknown as Rect;
    const width = this.dom.offsetWidth;
    return { left: start.left, right: start.left + width, top: start.top, bottom: Math.max(start.bottom, end.bottom) };
  }

  destroy() {
    window.clearTimeout(this.timer);
  }
}

/** Esc closes what is open, the paste chip last. Below the completion and snippet keymaps, whose
 *  Esc goes first. */
const toolsKeymap = keymap.of([
  {
    key: "Escape",
    run: (view) => {
      if (mathToolsOpen(view.state)) view.dispatch({ effects: openMathTools.of(null) });
      else if (view.state.field(pastedField, false)) view.dispatch({ effects: pastedMath.of(null) });
      else return false;
      return true;
    },
  },
  { key: "Mod-Shift-Space", run: toggleMathTools },
  {
    // The field has the same key for the other way (`mathField.ts`).
    key: "Mod-Shift-m",
    run: (view) => {
      if (visualToggle(view.state) !== "visual") return false;
      setMathMode(view, "visual");
      return true;
    },
  },
]);

/** Where the last accepted completion left the caret, so the character
 *  typed after a field-less one (`\alpha` then a space) doesn't count twice. */
const completedAt = new WeakMap<EditorView, number>();

/** A `\command` typed out in maths counts as used once a character that
 *  can't continue its name follows it. */
const typedCommands = EditorView.inputHandler.of((view, from, to, text) => {
  if (from !== to || text.length !== 1 || /[a-zA-Z]/.test(text)) return false;
  const done = completedAt.get(view) === from;
  completedAt.delete(view);
  if (done || !mathAt(view.state, from)) return false;
  const name = /(?<!\\)\\[a-zA-Z]+$/.exec(view.state.sliceDoc(Math.max(0, from - 32), from))?.[0];
  if (name) recordCommand(name, subjectOf(view));
  return false;
});

/** The popover, in both modes, the quick picks in the visual field, and the
 *  chip on pasted maths. */
export function mathTools(): Extension {
  return [mathToolsField, pastedField, toolsKeymap, fieldKeys.of(fieldKey), typedCommands];
}

// ── `\` completion ────────────────────────────────────────────────────────

/** Completion rows' KaTeX previews, by option. */
const optionPreviews = new WeakMap<Completion, string>();
let options: Completion[] | null = null;

/** Every palette entry and extra command led by a `\command`, once each. */
function mathOptions(): Completion[] {
  if (options) return options;
  const seen = new Set<string>();
  options = [];
  for (const entry of [...MATH_TABS.flatMap((t) => t.entries), ...MATH_COMMANDS]) {
    const name = commandOf(entry.template);
    if (!name || seen.has(entry.template)) continue;
    seen.add(entry.template);
    const rest = sourceOf(entry.template).slice(name.length).trim();
    const option = snippetCompletion(snippetTemplate(entry.template), {
      label: name,
      detail: rest.length > 18 ? `${rest.slice(0, 17)}…` : rest || undefined,
      boost: COMMON_COMMANDS.has(name) ? 1 : 0,
    });
    const insert = option.apply as (view: EditorView, c: Completion, from: number, to: number) => void;
    option.apply = (view, c, from, to) => {
      insert(view, c, from, to);
      completedAt.set(view, view.state.selection.main.head);
      recordUse(entry, subjectOf(view));
    };
    optionPreviews.set(option, previewOf(entry));
    options.push(option);
  }
  return options;
}

/**
 * Answers only inside maths (`mathAt`): after `\` and a letter while typing,
 * or anywhere on Ctrl-Space. CodeMirror's fuzzy match ranks prefix matches
 * first. Exported so the editor's one `autocompletion()` can hold other sources.
 */
export function mathCompletionSource(cx: CompletionContext): CompletionResult | null {
  const math = mathAt(cx.state, cx.pos);
  if (!math) return null;
  const word = cx.matchBefore(/\\[a-zA-Z]*/);
  // `\\` is a line break, not the start of a command.
  if (word && word.from >= math.from && cx.state.sliceDoc(word.from - 1, word.from) !== "\\") {
    if (word.text.length < 2 && !cx.explicit) return null;
    return { from: word.from, options: mathOptions(), validFor: /^\\[a-zA-Z]*$/ };
  }
  if (!cx.explicit) return null;
  return { from: cx.pos, options: mathOptions(), validFor: /^\\[a-zA-Z]*$/ };
}

/** `addToOptions` entry: a KaTeX preview left of each maths option. */
export const mathOptionPreview = {
  position: 10,
  render(completion: Completion): Node | null {
    const latex = optionPreviews.get(completion);
    if (latex == null) return null;
    const dom = el("span", "cm-math-option-preview");
    dom.innerHTML = previewHtml(latex);
    return dom;
  },
};
