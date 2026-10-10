import type { EditorView } from "@codemirror/view";

import { mathFieldFocused, setFocused } from "@/components/documents/editor/core/liveFocus";
import { noteHost } from "@/components/documents/editor/core/host";
import type { Step } from "@/lib/maths";
import { recordCommand } from "../../tools/mathUsage";
import { markFieldTrap } from "../fieldEngine";
import { dropBlankLines, leaveMaths, removeMaths, type Direction } from "../fieldNote";
import type { Box } from "../mathField/geometry";
import { fields, type VisualField } from "../mathField/registry";
import { setMathMode, type ActiveMath } from "../mathField/visual-state";
import { MathView } from "../mathView";
import { beforeClipboard, copy, paste } from "./clipboard";
import { syncHint } from "./hint";
import { historyInput, hostKey } from "./keys";
import { placeCaret, pressBeside } from "./mount";
import { resync, writableTarget, writeStep, type FieldText } from "./write";

/** The step's effect as the side `leave` goes to. */
const LEAVE: Record<string, Direction> = {
  leaveLeft: "backward",
  leaveRight: "forward",
  leaveUp: "upward",
  leaveDown: "downward",
};

/**
 * The Rust field (`MathView` over the maths engine's edit model) on the
 * note's maths: the Rust engine's `VisualField`. Each step the view hands
 * over is written into the note as it happens (`writeStep`): its changes
 * as one transaction, a shortcut's rewrite as a second that is an undo step
 * of its own, then its effect. Keys, the clipboard, the empty-line hint and
 * the caret's first place are the sibling modules, each taking this.
 */
export class RustFieldController implements VisualField {
  readonly dom: HTMLElement;
  readonly mv: MathView;
  /** The maths' LaTeX as last written or loaded (`writeStep`). */
  readonly text: FieldText;
  /** The `\command` pending when the last key went down, so a step that
   *  commits it counts the command as used. */
  pendingAtKey: string | undefined;
  readonly hint: HTMLElement;
  dead = false;
  private listeners = new Set<() => void>();

  /** Throws the model's `ParseError`, or a `MathsTrap`, for maths the field
   *  can't open (`openRustField` drops it to TeX). */
  constructor(
    readonly view: EditorView,
    source: string,
    readonly display: boolean,
    readonly block: boolean,
    /** The maths this field edits (`ActiveMath.id`), for its whole life. */
    readonly id: number,
  ) {
    this.text = { id, shown: source, written: source };
    this.mv = new MathView(source, display, block, {
      onStep: (step) => this.step(step),
      onSelectionChange: () => this.selectionChanged(),
      onTrap: (_error, view) => this.trapped(view),
      onKey: (e) => hostKey(this, e),
    });
    this.dom = this.mv.dom;
    this.hint = document.createElement("span");
    this.hint.className = "cm-math-hint";
    this.hint.textContent = block ? "Start typing or Space (␣) for math tools" : "Space (␣) for math tools";
    this.hint.hidden = true;
    this.dom.append(this.hint);

    // Undo from the Edit menu arrives as `beforeinput`, which the view lets
    // bubble.
    this.dom.addEventListener("beforeinput", (e) => historyInput(this, e));
    this.dom.addEventListener("mousedown", (e) => pressBeside(this, e));
    this.dom.addEventListener("copy", (e) => copy(this, e, false));
    this.dom.addEventListener("cut", (e) => copy(this, e, true));
    this.dom.addEventListener("paste", (e) => paste(this, e));
    this.dom.addEventListener("beforecopy", (e) => beforeClipboard(this, e));
    this.dom.addEventListener("beforecut", (e) => beforeClipboard(this, e));
    this.mv.input.addEventListener("blur", () => this.focusLeft());

    fields.set(view, this);
    queueMicrotask(() => this.mount());
  }

  /** Focus and place the caret, once the widget is in the document. */
  private mount() {
    if (this.dead || this.mv.dead || !this.dom.isConnected) return;
    if (this.block) {
      const latex = dropBlankLines(this.view, this.target(), this.id, (tidied) => (this.text.shown = tidied));
      if (latex != null) {
        this.text.written = latex;
        this.mv.setSource(latex, latex.length);
      }
    }
    this.mv.redraw();
    this.mv.focus();
    placeCaret(this);
    syncHint(this);
  }

  mode(): "math" | "text" | "command" {
    return this.mv.mode;
  }

  /** Only a selection other than the one at subscribing counts. */
  onSelectionChange(listener: () => void): () => void {
    const at = this.selectionKey();
    const moved = () => {
      if (this.selectionKey() !== at) listener();
    };
    this.listeners.add(moved);
    return () => this.listeners.delete(moved);
  }

  private selectionKey(): string {
    const f = this.mv.field;
    return `${f.anchor}:${f.head}:${f.mode}:${f.pending ?? ""}:${f.source}`;
  }

  isEmpty(): boolean {
    return !this.mv.source.trim();
  }

  spaceFree(): boolean {
    return this.mv.mode === "math" && this.mv.field.spaceFree;
  }

  caretRect(): Box | null {
    return this.mv.caretRect();
  }

  insertTemplate(template: string) {
    let n = 0;
    const latex = template.replace(/[#$]\{[^{}]*\}/g, () => (n++ === 0 ? "#0" : "#?"));
    this.mv.focus();
    this.mv.run({ template: latex });
  }

  /** Steps are written as they happen: nothing waits. */
  flush() {}

  leave(dir: Direction) {
    leaveMaths(this.view, this.target(), dir);
  }

  /** The maths this field may write to, or null when it is gone or was
   *  changed from outside since the field last saw it. */
  target(): ActiveMath | null {
    return this.dead ? null : writableTarget(this.view, this.text);
  }

  /** The note's LaTeX changed under the field (an undo, a redo): the view
   *  opens on it, the caret at the end of what changed (`caretAfterEdit`). */
  sync(source: string) {
    if (this.dead) return;
    const f = this.mv.field;
    const caret = resync(this.text, source, f.stops[f.head]);
    if (caret == null) return;
    try {
      this.mv.setSource(source, caret);
    } catch {
      // It no longer opens (the visual state checks before it reuses the
      // field, so only a race gets here): edit it as TeX.
      this.toTex(source);
      return;
    }
    syncHint(this);
  }

  /** One step from the view: written into the note (`writeStep`), then
   *  its effect. */
  private step(step: Step) {
    const committed = this.pendingAtKey !== undefined;
    this.pendingAtKey = undefined;
    writeStep(this.view, this.text, step);
    if (committed) this.countCommands(step);
    if (step.effect === "removeMaths") removeMaths(this.view, this.target());
    else if (step.effect) this.leave(LEAVE[step.effect]);
  }

  /** A `\command` committed from command mode counts toward the toolbox's
   *  Recent row, Popular tab and quick picks (`mathUsage.ts`). */
  private countCommands(step: Step) {
    const names = step.changes.flatMap((c) => c.insert.match(/\\[a-zA-Z]+/g) ?? []);
    const subject = this.view.state.facet(noteHost).subjectId;
    for (const name of new Set(names)) recordCommand(name, subject);
  }

  private selectionChanged() {
    // The view's first draw reports from inside its constructor, before
    // this has its view or hint; `mount` draws those.
    if (!this.hint) return;
    this.view.dom.classList.toggle("cm-math-command", this.mv.mode === "command");
    syncHint(this);
    for (const listener of [...this.listeners]) listener();
  }

  /** The engine trapped on this formula: the view takes no more input, and
   *  the formula is edited as TeX from now on. */
  private trapped(view: MathView) {
    markFieldTrap(view.source, this.display);
    this.toTex(this.text.shown);
  }

  /** Drops this maths to TeX mode once the current update is over. */
  private toTex(source: string) {
    markFieldTrap(source, this.display);
    queueMicrotask(() => {
      if (fields.get(this.view) === this && !this.dead) setMathMode(this.view, "tex");
    });
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
    const focused = this.mv.hasFocus();
    this.dead = true;
    this.mv.destroy();
    this.listeners.clear();
    if (fields.get(this.view) === this) fields.delete(this.view);
    this.view.dom.classList.remove("cm-math-command");
    // Removed while typing in it (a command elsewhere moved the selection):
    // the note takes the keyboard back rather than the page.
    if (focused) {
      window.setTimeout(() => {
        if (this.view.dom.isConnected && !mathFieldFocused() && !this.view.hasFocus) this.view.focus();
      }, 0);
    }
  }
}

/** The Rust field on this maths, or null when it can't open: the maths
 *  then drops to TeX mode, and the widget shows its source meanwhile. */
export function openRustField(
  view: EditorView,
  source: string,
  display: boolean,
  block: boolean,
  id: number,
): RustFieldController | null {
  try {
    return new RustFieldController(view, source, display, block, id);
  } catch {
    markFieldTrap(source, display);
    queueMicrotask(() => setMathMode(view, "tex"));
    return null;
  }
}
