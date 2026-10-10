import { mathOnly, pasteBeside } from "../fieldNote";
import { BLOCK_MATH_TYPE } from "../mathField/controller/clipboard";
import type { RustFieldController } from "./controller";

/** The selection's LaTeX, or "" with none. */
function selected(ctl: RustFieldController): string {
  const [from, to] = ctl.mv.field.selected;
  return ctl.mv.source.slice(from, to);
}

/**
 * Copy and cut write the selection's LaTeX as `text/plain` and as
 * `application/x-latex` (the note's paste reads that as LaTeX copied from a
 * field, `fieldLatexPaste`), and a display field's also as its `$$` lines
 * (`BLOCK_MATH_TYPE`). Cut then deletes it as Backspace does.
 */
export function copy(ctl: RustFieldController, e: ClipboardEvent, cut: boolean) {
  const latex = selected(ctl).trim();
  const data = e.clipboardData;
  if (!latex || !data) return;
  e.preventDefault();
  e.stopPropagation();
  data.setData("text/plain", latex);
  data.setData("application/x-latex", latex);
  if (ctl.display) data.setData(BLOCK_MATH_TYPE, `$$\n${latex}\n$$`);
  if (cut) ctl.mv.run("backspace");
}

/** WebKit enables Copy and Cut only for a text selection, and the field's
 *  input holds none: cancelling `beforecopy`/`beforecut` says the page
 *  handles them. */
export function beforeClipboard(ctl: RustFieldController, e: Event) {
  if (selected(ctl)) e.preventDefault();
}

/** `$…$`, `$$…$$`, `\(…\)` or `\[…\]` around the whole text, taken off. */
function unwrapped(text: string): string {
  const t = text.trim();
  const m = /^(?:\$\$([\s\S]*)\$\$|\$([\s\S]*)\$|\\\[([\s\S]*)\\\]|\\\(([\s\S]*)\\\))$/.exec(t);
  return m ? (m[1] ?? m[2] ?? m[3] ?? m[4]).trim() : t;
}

/** Maths goes in at the caret through the model (refused unless it
 *  renders); markdown with prose around maths goes beside it. */
export function paste(ctl: RustFieldController, e: ClipboardEvent) {
  e.preventDefault();
  e.stopPropagation();
  const text = e.clipboardData?.getData("text/plain") ?? "";
  if (!text) return;
  if (mathOnly(text)) ctl.mv.run({ paste: unwrapped(text) });
  else pasteBeside(ctl.view, ctl.target(), text);
}
