import { mathOnly, pasteBeside } from "../fieldNote";
import { copied } from "@/lib/markdown/mathSelection/clipboard";
import type { RustFieldController } from "./controller";

/** The selection's LaTeX, or "" with none. */
function selected(ctl: RustFieldController): string {
  const [from, to] = ctl.mv.field.selected;
  return ctl.mv.source.slice(from, to);
}

/**
 * Copy and cut write what a chat formula's selection writes (`copied`):
 * the LaTeX wrapped by its shape as `text/plain`, bare as
 * `application/x-latex` (the note's paste reads that as LaTeX copied from a
 * field, `fieldLatexPaste`), and a block's `$$` lines (`BLOCK_MATH_TYPE`).
 * Cut then deletes it as Backspace does.
 */
export function copy(ctl: RustFieldController, e: ClipboardEvent, cut: boolean) {
  const entries = copied(ctl.mv.field, ctl.display);
  const data = e.clipboardData;
  if (!entries.length || !data) return;
  e.preventDefault();
  e.stopPropagation();
  for (const [type, value] of entries) data.setData(type, value);
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
