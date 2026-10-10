import { noteHost } from "@/components/documents/editor/core/host";
import { recordCommand } from "../../../tools/mathUsage";
import { modelOf } from "../model";
import { syncHint } from "./hint";
import type { FieldController } from "./field-controller";

export type TextStyle = { fontSeries?: "b"; fontShape?: "it" };

/** Commands whose argument is text: committing one bare, or its palette
 *  entry, starts typing text in that style (`startText`). */
export const TEXT_COMMANDS: Record<string, TextStyle> = {
  "\\text": {},
  "\\textrm": {},
  "\\textnormal": {},
  "\\mbox": {},
  "\\textbf": { fontSeries: "b" },
  "\\textit": { fontShape: "it" },
};

/** MathLive's command list sits where the toolbox does; one at a time.
 *  Leaving command mode inserts what was typed, reported by the next
 *  `input` (a timeout away); a cancelled command reports nothing, so the
 *  window closes after that timeout. */
export function modeChanged(ctl: FieldController) {
  const latex = ctl.mf.mode === "latex";
  ctl.view.dom.classList.toggle("cm-math-command", latex);
  if (ctl.typingCommand && !latex) {
    ctl.committing = true;
    queueMicrotask(() => window.setTimeout(() => (ctl.committing = false), 0));
  }
  ctl.typingCommand = latex;
  syncHint(ctl);
}

/** A command committed from command mode counts toward the toolbox's
 *  Recent row, Popular tab and quick picks (`mathUsage.ts`). */
export function committed(ctl: FieldController, e: InputEvent) {
  // `data` is the inserted LaTeX (WebKit strips `inputType`); keystrokes
  // still queued from command mode carry none.
  const names = ctl.committing ? e.data?.match(/\\[a-zA-Z]+/g) : null;
  if (!names) return;
  ctl.committing = false;
  const subject = ctl.view.state.facet(noteHost).subjectId;
  for (const name of new Set(names)) recordCommand(name, subject);
}

/** A bare `\text` committed from command mode inserts nothing (MathLive
 *  drops the empty argument), so text is typed there instead. The commit's
 *  `beforeinput` carries the command, inside the commit itself. */
export function committingText(ctl: FieldController, e: InputEvent) {
  const text = ctl.committing ? TEXT_COMMANDS[e.data ?? ""] : undefined;
  if (text) queueMicrotask(() => startText(ctl, text));
}

/** Type text at the caret, as inside `\text{}` (or `\textbf{}`…): MathLive's
 *  text mode, which → or Tab at the run's end leaves (`onKey`). */
export function startText(ctl: FieldController, style: TextStyle) {
  ctl.mf.executeCommand(["switchMode", "text"]);
  // MathLive's insert style sticks until the caret moves: set it outright.
  ctl.mf.applyStyle({ fontSeries: "auto", fontShape: "auto", ...style });
}

/** In text mode: whether the caret is at the end of its run of text. */
export function atTextEnd(ctl: FieldController): boolean {
  return modelOf(ctl.mf)?.at(ctl.mf.position + 1)?.mode !== "text";
}
