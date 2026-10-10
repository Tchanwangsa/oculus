import { isolateHistory } from "@codemirror/commands";
import { EditorSelection, Transaction, type ChangeSpec } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";
import type { MathfieldElement } from "mathlive";

import { mathFieldFocused, setFocused } from "@/components/documents/editor/core/liveFocus";
import {
  FIELD_INPUT,
  caretAfterChange,
  fieldWrite,
  minimalChange,
  squeezeBlankLines,
  writableSpan,
} from "../../mathFieldEdits";
import { caretAt, type Box } from "../geometry";
import { takePress } from "../keys";
import { lib } from "../loader";
import { atomsOf, modelOf } from "../model";
import { fields } from "../registry";
import { centredRows } from "../rows";
import { fromField, layoutBlock, tidy, toField } from "../serialize";
import { shortcuts } from "../shortcuts";
import { staticMath } from "../static";
import { visualMath, type ActiveMath } from "../visual-state";
import { TEXT_COMMANDS, committed, committingText, modeChanged, startText } from "./command-mode";
import { copied, paste } from "./clipboard";
import { gridStep } from "./grid";
import { syncHint } from "./hint";
import { historyInput } from "./history";
import { moveOut, onKey } from "./keyboard";
import { press, pressBeside, selectionChanged } from "./pointer";

export type Direction = "forward" | "backward" | "upward" | "downward";

/**
 * The open `<math-field>` and the note's maths it edits. State lives here;
 * the keys, pointer, clipboard, history, grid, deletion, command-mode and
 * hint behaviour are the sibling modules, each taking the controller.
 */
export class FieldController {
  readonly dom: HTMLElement;
  readonly mf: MathfieldElement;
  /** The LaTeX (trimmed, as the widget sees it) last written or loaded, so
   *  the field's own writes don't echo back into it. */
  shown: string;
  /** The value last handed to MathLive or last flushed, which `getValue`
   *  returns verbatim until an edit: unchanged, there is nothing to write. */
  loaded: string;
  tabbing = false;
  tabFailed = false;
  /** In MathLive's command mode (`\lam…`), and just out of it. */
  typingCommand = false;
  committing = false;
  /** Unmounted: MathLive's late `input` must not write. */
  dead = false;
  /** The maths drawn statically, holding the field's box until it renders. */
  private standIn: HTMLElement | null;
  /** The prompt to Space for the toolbox on an empty line: centred on the
   *  line in a block, whose caret is hidden then, after the empty field in
   *  flow inline. The frame its placement waits for. */
  hint: HTMLElement | null = null;
  hintFrame = 0;

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
      committed(this, e as InputEvent);
      this.flush();
      syncHint(this);
    });
    mf.addEventListener("move-out", (e) => moveOut(this, e as CustomEvent<{ direction: Direction }>));
    mf.addEventListener("focusout", () => this.focusLeft());
    // MathLive's command list sits where the toolbox does; one at a time.
    mf.addEventListener("mode-change", () => modeChanged(this));
    mf.addEventListener("selection-change", () => {
      selectionChanged(this);
      syncHint(this);
    });
    // Capture, so these keys never reach MathLive's own handling.
    this.dom.addEventListener("keydown", (e) => onKey(this, e), true);
    // Undo from the Edit menu arrives as `beforeinput`, not a key.
    this.dom.addEventListener("beforeinput", (e) => historyInput(this, e), true);
    mf.addEventListener("beforeinput", (e) => committingText(this, e as InputEvent));
    // Bubbling, so MathLive (in its shadow root) has placed the caret first.
    this.dom.addEventListener("pointerdown", (e) => press(this, e));
    this.dom.addEventListener("mousedown", (e) => pressBeside(this, e));
    this.dom.addEventListener("paste", (e) => paste(this, e), true);
    // Bubbling, after MathLive has put its LaTeX on the clipboard.
    this.dom.addEventListener("copy", (e) => copied(this, e));
    this.dom.addEventListener("cut", (e) => copied(this, e));

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
    syncHint(this);
    const target = this.target();
    const sel = this.view.state.selection.main;
    const ink = mf.shadowRoot?.querySelector(".ML__latex")?.getBoundingClientRect();
    const pressed = takePress();
    if (pressed && ink && Date.now() - pressed.at < 600) {
      const x = ink.left + pressed.fx * ink.width;
      const y = ink.top + pressed.fy * ink.height;
      mf.position = caretAt(mf, x, y) ?? mf.getOffsetFromPoint(x, y);
    } else if (target && !sel.empty && sel.from <= target.from && sel.to >= target.to) {
      mf.select();
    } else {
      mf.position = target && sel.head <= target.from ? 0 : mf.lastOffset;
    }
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
    syncHint(this);
  }

  /** Nothing typed in the field (a bare `$$` inline pair, a fresh block). */
  isEmpty(): boolean {
    return this.mf.getValue("latex-without-placeholders") === "";
  }

  /** The maths this field may write to, or null when it is gone or was
   *  changed from outside since the field last saw it. */
  target(): ActiveMath | null {
    if (this.dead) return null;
    return writableSpan(this.view.state, visualMath(this.view.state), this.id, this.shown);
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
      !gridStep(this, " ")
    );
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
      startText(this, text);
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
