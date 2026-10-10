import "@/styles/katex/katex.min.css";

import { MathField, MathsTrap, type FieldCommand, type FieldMode, type Step } from "@/lib/maths";
import {
  frameOrigin,
  framePoint,
  measure,
  readLayout,
  stopAt,
  type Box,
  type Layout,
  type Measured,
} from "@/lib/maths/geometry";
import { drawCaret, restartBlink } from "./caret";
import { compositionEnd, compositionStart, beforeInput, inputEvent } from "./input";
import { keyDown } from "./keys";
import { drawPending, pendingHtml } from "./pending";
import { mouseDown, pointerDown } from "./pointer";
import { createPopover, drawPopover } from "./popover";
import { fieldHtml, markEmptyRows } from "./render";
import { drawBands } from "./selection";

/** What the view tells its host, and the keys the host takes first. */
export interface MathViewHost {
  /** After each step, once the view has redrawn: apply its `changes`, then
   *  its `rewrite`, and act on its `effect`. */
  onStep?(step: Step, view: MathView): void;
  /** The selection, the mode or the pending command changed. */
  onSelectionChange?(view: MathView): void;
  /** The engine trapped on this formula: the view takes no more input;
   *  edit the formula as TeX. */
  onTrap?(error: MathsTrap, view: MathView): void;
  /** A key the host takes before the view (true when taken), as the
   *  toolbox and the note's undo do. */
  onKey?(e: KeyboardEvent, view: MathView): boolean;
}

const EMPTY: Measured = { x: new Float64Array(), top: new Float64Array(), bottom: new Float64Array() };

/**
 * The visual maths field over the Rust edit model (`MathField`), without
 * CodeMirror: the rendered maths with the source map on, a caret, one
 * selection band per row, the pending `\command` and its options, and a focused
 * hidden textarea at the caret that takes keys, typing and IME input (so
 * the OS candidate window sits at the caret). It never edits the source
 * itself: each key runs a model command, the view redraws from the step's
 * field and hands the step on (`onStep`). Opening throws the model's
 * `ParseError` for maths it cannot read (edit it as TeX) or a `MathsTrap`.
 * Keys, typing, the pointer, the caret, the bands and the popover are the
 * sibling modules, each taking the view.
 */
export class MathView {
  readonly dom: HTMLElement;
  /** The positioned box the overlays share coordinates with (a block's
   *  scrolls sideways). */
  readonly frame: HTMLElement;
  readonly rendered: HTMLElement;
  readonly bandLayer: HTMLElement;
  readonly caret: HTMLElement;
  readonly input: HTMLTextAreaElement;
  /** An IME's text while it composes, drawn at the caret. */
  readonly preedit: HTMLElement;
  /** The pending command's options (`popover/`). */
  readonly popover: HTMLElement;
  layout: Layout = { items: [] };
  measured: Measured = EMPTY;
  /** The IME is composing: its keys and input are its own. */
  composing = false;
  /** Text a composition just committed, which WebKit may send again as
   *  `insertText`. */
  composed: string | null = null;
  /** Trapped or destroyed: no more input. */
  dead = false;
  /** The `\command` pending before the last `run`, so the host can tell a
   *  step that committed one. */
  pendingBefore: string | undefined;
  #field: MathField;
  /** The source and pending `\name` `rendered` holds, so a move alone
   *  never re-renders. */
  #drawn: string | null = null;
  #resize: ResizeObserver;

  constructor(
    source: string,
    readonly display: boolean,
    /** On lines of its own (a `div`), else in a line (a `span`). */
    readonly block: boolean,
    readonly host: MathViewHost = {},
  ) {
    this.#field = MathField.open(source, display);
    const el = <K extends keyof HTMLElementTagNameMap>(tag: K, cls: string) => {
      const e = document.createElement(tag);
      e.className = cls;
      return e;
    };
    this.dom = el(block ? "div" : "span", block ? "cm-math-view cm-math-view-block" : "cm-math-view");
    this.frame = el(block ? "div" : "span", "cm-math-view-frame");
    this.rendered = el(block ? "div" : "span", "cm-math-view-maths");
    this.bandLayer = el("span", "cm-math-view-bands");
    this.caret = el("span", "cm-math-view-caret");
    this.preedit = el("span", "cm-math-view-preedit");
    this.preedit.hidden = true;
    this.input = el("textarea", "cm-math-view-input");
    this.input.setAttribute("autocapitalize", "off");
    this.input.setAttribute("autocomplete", "off");
    this.input.setAttribute("autocorrect", "off");
    this.input.spellcheck = false;
    this.input.setAttribute("aria-label", "Maths");
    this.popover = createPopover(this);
    this.frame.append(this.bandLayer, this.rendered, this.caret, this.preedit, this.input);
    this.dom.append(this.frame, this.popover);

    this.input.addEventListener("keydown", (e) => keyDown(this, e));
    this.input.addEventListener("beforeinput", (e) => beforeInput(this, e));
    this.input.addEventListener("input", () => inputEvent(this));
    this.input.addEventListener("compositionstart", () => compositionStart(this));
    this.input.addEventListener("compositionend", (e) => compositionEnd(this, e));
    this.input.addEventListener("focus", () => this.focusChanged());
    this.input.addEventListener("blur", () => this.focusChanged());
    this.dom.addEventListener("pointerdown", (e) => pointerDown(this, e));
    this.dom.addEventListener("mousedown", (e) => mouseDown(this, e));
    // Fonts arriving, or the frame first laid out: the boxes moved.
    this.#resize = new ResizeObserver(() => this.redraw());
    this.#resize.observe(this.rendered);
    const trap = this.#show(this.#field);
    if (trap) this.#trap(trap);
  }

  get field(): MathField {
    return this.#field;
  }

  get source(): string {
    return this.#field.source;
  }

  /** What typing does at the caret: maths, text, or the pending command. */
  get mode(): FieldMode {
    return this.#field.mode;
  }

  /** Runs one model command (a key, typed text, a template, a paste) and
   *  redraws; null when the view is dead or the engine trapped. */
  run(command: FieldCommand): Step | null {
    if (this.dead) return null;
    this.pendingBefore = this.#field.pending;
    let step: Step;
    try {
      step = this.#field.run(command);
    } catch (e) {
      if (e instanceof MathsTrap) return this.#trap(e);
      throw e;
    }
    // A step whose result traps on rendering still reaches the note.
    const trap = this.#show(step.field);
    this.host.onStep?.(step, this);
    return trap ? this.#trap(trap) : step;
  }

  /** The selection between two stops (widened by the model to whole
   *  structures); the head is the caret. */
  select(anchor: number, head: number) {
    if (this.dead) return;
    try {
      const trap = this.#show(this.#field.select(anchor, head));
      if (trap) this.#trap(trap);
    } catch (e) {
      if (e instanceof MathsTrap) this.#trap(e);
      else throw e;
    }
  }

  /** The source changed from outside (an undo): a field on it, the caret at
   *  `caretOffset` (the end when that is no character boundary). Throws the
   *  model's `ParseError` when it no longer reads. */
  setSource(source: string, caretOffset: number) {
    if (this.dead) return;
    try {
      const opened = MathField.open(source, this.display);
      let field = opened;
      try {
        field = opened.caretAt(Math.max(0, Math.min(caretOffset, source.length)));
      } catch (e) {
        if (e instanceof MathsTrap) throw e;
      }
      if (field !== opened) opened.free();
      const trap = this.#show(field);
      if (trap) this.#trap(trap);
    } catch (e) {
      if (e instanceof MathsTrap) this.#trap(e);
      else throw e;
    }
  }

  /** The stop a press at this viewport point lands on. */
  stopAtPoint(clientX: number, clientY: number): number | null {
    this.#ensureLayout();
    const { x, y } = framePoint(this.frame, clientX, clientY);
    return stopAt(this.layout, this.#field, this.measured, x, y);
  }

  focus() {
    this.input.focus({ preventScroll: true });
  }

  hasFocus(): boolean {
    return document.activeElement === this.input;
  }

  /** The caret's viewport rect (the selection's head while one is drawn),
   *  for the quick picks to hang from; null before it is laid out. */
  caretRect(): Box | null {
    this.#ensureLayout();
    const id = this.#field.head;
    const x = this.measured.x[id];
    if (!Number.isFinite(x)) return null;
    const o = frameOrigin(this.frame);
    return { left: o.x + x, right: o.x + x, top: o.y + this.measured.top[id], bottom: o.y + this.measured.bottom[id] };
  }

  destroy() {
    this.dead = true;
    this.#resize.disconnect();
    this.#field.free();
  }

  /** Re-reads the boxes (fonts loaded, the frame resized) and redraws. */
  redraw() {
    if (this.dead || !this.dom.isConnected) return;
    this.layout = readLayout(this.rendered, this.frame);
    this.measured = measure(this.layout, this.#field);
    this.#draw();
  }

  /** The view drawn from `field`; the trap when its render trapped. */
  #show(field: MathField): MathsTrap | null {
    const old = this.#field;
    this.#field = field;
    if (old !== field) old.free();
    const drawn = field.pending == null ? field.source : `${field.source}\u0000${field.head}\u0000${field.pending}`;
    if (this.#drawn !== drawn) {
      let html: string;
      try {
        html = (field.pending != null && pendingHtml(field, this.display)) || fieldHtml(field.source, this.display);
      } catch (e) {
        if (e instanceof MathsTrap) return e;
        throw e;
      }
      this.rendered.innerHTML = html;
      markEmptyRows(this.rendered, field);
      this.#drawn = drawn;
      this.layout = { items: [] };
    }
    this.measured = EMPTY;
    if (this.dom.isConnected) this.redraw();
    if (
      old === field ||
      old.anchor !== field.anchor ||
      old.head !== field.head ||
      old.mode !== field.mode ||
      old.pending !== field.pending ||
      old.source !== field.source
    ) {
      this.host.onSelectionChange?.(this);
    }
    return null;
  }

  /** Measures now if the view was shown while out of the document. */
  #ensureLayout() {
    if (this.measured.x.length !== this.#field.stops.length && this.dom.isConnected) this.redraw();
  }

  #draw() {
    drawBands(this);
    drawCaret(this);
    drawPending(this);
    drawPopover(this);
  }

  private focusChanged() {
    this.dom.classList.toggle("cm-math-view-focused", this.hasFocus());
    if (this.hasFocus()) restartBlink(this);
  }

  #trap(e: MathsTrap): null {
    this.dead = true;
    this.host.onTrap?.(e, this);
    return null;
  }
}
