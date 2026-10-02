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
  type EditorState,
  type Extension,
  type Transaction,
} from "@codemirror/state";
import {
  EditorView,
  keymap,
  showTooltip,
  type Rect,
  type Tooltip,
  type TooltipView,
  type ViewUpdate,
} from "@codemirror/view";
import katex from "katex";
import { syncScrollFade } from "@/lib/scrollFade";

import { mathAt } from "./mathContext";
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

/**
 * The maths toolbox: a popover under the maths the caret is in (live KaTeX
 * preview, recents and a tabbed palette), and `\`
 * completion inside maths. Both insert `snippet()`s, so a template's `{}`
 * slots are Tab fields. The palette's data is `mathPalette.ts`; the look is
 * `theme.ts`.
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

interface ToolsState {
  math: MathRange | null;
  /** False while `math` is carried over an edit rather than parsed. */
  parsed: boolean;
  /** The `nodeFrom` of maths whose popover Esc hid, mapped through edits. */
  dismissed: number | null;
  tooltip: Tooltip | null;
}

const dismissTools = StateEffect.define<number>();

const mathToolsField = StateField.define<ToolsState>({
  create(state) {
    const math = caretMath(state);
    return { math, parsed: true, dismissed: null, tooltip: math ? toolsTooltip(math.nodeFrom) : null };
  },
  update(prev, tr) {
    let dismissed = prev.dismissed != null && tr.docChanged ? tr.changes.mapPos(prev.dismissed, 1) : prev.dismissed;
    for (const e of tr.effects) if (e.is(dismissTools)) dismissed = e.value;
    let math = caretMath(tr.state);
    const parsed = math != null;
    if (!math && prev.math && prev.parsed && tr.docChanged) math = carriedMath(prev.math, tr);
    if (!math || (dismissed != null && dismissed !== math.nodeFrom)) dismissed = null;
    const tooltip =
      !math || dismissed != null ? null
      : prev.tooltip?.pos === math.nodeFrom ? prev.tooltip
      : toolsTooltip(math.nodeFrom);
    if (sameRange(math, prev.math) && parsed === prev.parsed && dismissed === prev.dismissed && tooltip === prev.tooltip) {
      return prev;
    }
    return { math, parsed, dismissed, tooltip };
  },
  provide: (f) => showTooltip.from(f, (v) => v.tooltip),
});

/** One `create` for every popover, so CodeMirror keeps the open one's DOM as
 *  the caret moves between maths. */
const createTools = (view: EditorView): TooltipView => new MathToolsView(view);

function toolsTooltip(pos: number): Tooltip {
  return { pos, above: false, create: createTools };
}

// ── Recents ───────────────────────────────────────────────────────────────

const RECENTS_KEY = "oculus-math-recents";
const RECENTS_MAX = 8;

function readRecents(): MathEntry[] {
  try {
    const list: unknown = JSON.parse(localStorage.getItem(RECENTS_KEY) ?? "[]");
    if (!Array.isArray(list)) return [];
    return list
      .filter((e): e is MathEntry => typeof e?.template === "string")
      .slice(0, RECENTS_MAX);
  } catch {
    return [];
  }
}

function pushRecent(entry: MathEntry) {
  const kept: MathEntry = { template: entry.template, label: entry.label, wide: entry.wide };
  const list = [kept, ...readRecents().filter((e) => e.template !== entry.template)].slice(0, RECENTS_MAX);
  try {
    localStorage.setItem(RECENTS_KEY, JSON.stringify(list));
  } catch {
    // Storage full or blocked: recents just don't persist.
  }
}

// ── The popover ───────────────────────────────────────────────────────────

/** The palette tab and matrix brackets last picked, for the session. */
let lastTab = MATH_TABS[0].id;
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

class MathToolsView implements TooltipView {
  dom = el("div", "cm-math-tools");
  /** Left out of tooltip stacking: it hides while the completion list is up
   *  rather than pushing the list down. */
  overlap = true;
  offset = { x: 0, y: 6 };
  private preview = el("div", "cm-math-preview");
  private error = el("div", "cm-math-error-text");
  private recents = el("div", "cm-math-recents");
  private tabs = el("div", "cm-math-tabs");
  private body = el("div", "cm-math-body");
  private shown: string | null = null;
  /** The last render that parsed, kept (dimmed) while the LaTeX is broken. */
  private good: { nodeFrom: number; html: string } | null = null;

  constructor(readonly view: EditorView) {
    this.dom.setAttribute("role", "group");
    this.dom.setAttribute("aria-label", "Maths tools");
    this.dom.addEventListener("mousedown", (e) => e.preventDefault());


    const tabbar = el("div", "cm-math-tabbar");
    tabbar.append(this.tabs);
    this.tabs.addEventListener("scroll", () => this.syncTabFades());
    this.body.addEventListener("scroll", () => this.syncBodyFades());
    // A mouse wheel scrolls the one-row tab strip sideways.
    this.tabs.addEventListener(
      "wheel",
      (e) => {
        if (Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;
        e.preventDefault();
        this.tabs.scrollLeft += e.deltaY;
      },
      { passive: false },
    );

    this.dom.append(this.preview, this.error, this.recents, tabbar, this.body);
    this.renderRecents();
    this.renderTabs();
    this.refresh(view.state);
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
  }

  update(u: ViewUpdate) {
    if (u.docChanged || u.selectionSet || u.startState.field(mathToolsField) !== u.state.field(mathToolsField)) {
      this.refresh(u.state);
    }
    this.syncHidden(u.state);
  }

  /** Left edge at the maths' start, top edge under its last line, so a long
   *  inline formula or a `$$` block is never covered. */
  getCoords(pos: number): Rect {
    const math = this.view.state.field(mathToolsField).math;
    const start = this.view.coordsAtPos(math ? math.nodeFrom : pos, 1);
    const end = math ? this.view.coordsAtPos(math.nodeTo, -1) : start;
    // Null hides the tooltip, which is right for maths scrolled out of view.
    if (!start || !end) return null as unknown as Rect;
    return { left: start.left, right: start.right, top: start.top, bottom: Math.max(start.bottom, end.bottom) };
  }

  private syncHidden(state: EditorState) {
    const hide = !this.view.hasFocus || completionStatus(state) === "active";
    const was = this.dom.classList.contains("cm-math-tools-hidden");
    this.dom.classList.toggle("cm-math-tools-hidden", hide);
    // Hidden, the strip measured zero wide; measure again once it shows.
    if (was && !hide) this.revealTab();
  }

  private insert(entry: MathEntry) {
    const { from, to } = this.view.state.selection.main;
    snippet(snippetTemplate(entry.template))(this.view, null, from, to);
    pushRecent(entry);
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
    const b = button(entry.wide ? "cm-math-cell cm-math-cell-wide" : "cm-math-cell", sourceOf(entry.template), () =>
      this.insert(entry),
    );
    b.innerHTML = previewHtml(previewOf(entry));
    return b;
  }

  private renderRecents() {
    const list = readRecents();
    this.recents.hidden = list.length === 0;
    this.recents.replaceChildren(el("span", "cm-math-caption", "Recent"), ...list.map((e) => this.cell(e)));
  }

  private renderTabs() {
    this.tabs.replaceChildren(
      ...MATH_TABS.map((tab) => {
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
    const tab = MATH_TABS.find((t) => t.id === lastTab) ?? MATH_TABS[0];
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

/** Esc hides the popover until the caret leaves that maths. Below the
 *  completion and snippet keymaps, whose Esc goes first. */
const toolsKeymap = keymap.of([
  {
    key: "Escape",
    run: (view) => {
      const v = view.state.field(mathToolsField, false);
      if (!v?.tooltip || !v.math) return false;
      view.dispatch({ effects: dismissTools.of(v.math.nodeFrom) });
      return true;
    },
  },
]);

/** The popover, in both modes. */
export function mathTools(): Extension {
  return [mathToolsField, toolsKeymap];
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
    const name = /^\\(?:begin\{[a-zA-Z*]+\}|[a-zA-Z]+)/.exec(entry.template)?.[0];
    if (!name || seen.has(entry.template)) continue;
    seen.add(entry.template);
    const rest = sourceOf(entry.template).slice(name.length).trim();
    const option = snippetCompletion(snippetTemplate(entry.template), {
      label: name,
      detail: rest.length > 18 ? `${rest.slice(0, 17)}…` : rest || undefined,
      boost: COMMON_COMMANDS.has(name) ? 1 : 0,
    });
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
