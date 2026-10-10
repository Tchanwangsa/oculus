import type { EditorState } from "@codemirror/state";
import { repositionTooltips, type EditorView, type Rect, type TooltipView, type ViewUpdate } from "@codemirror/view";
import katex from "katex";

import { syncScrollFade } from "@/lib/ui/scrollFade";
import { activeMathField, setMathMode, visualToggle } from "../../field/mathField";
import { MATH_TABS, matrixTemplate, type MathEntry, type MatrixKind } from "../mathPalette";
import { popularEntries, readRecents, usageVersion } from "../mathUsage";
import { CLOSE_ICON, button, cellButton, el, sideways, syncHidden } from "./dom";
import { insertEntry } from "./insert";
import { shapeToggle, toggleShape } from "./shape";
import { mathToolsField, openMathTools } from "./state";

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

export class MathToolsView implements TooltipView {
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
