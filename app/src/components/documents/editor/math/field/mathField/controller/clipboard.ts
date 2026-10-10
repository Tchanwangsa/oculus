import { fromField, layoutBlock, tidy } from "../serialize";
import type { FieldController } from "./field-controller";

/** Clipboard type of a block field's copy: the block's source, `$$` lines
 *  included, for a paste outside maths (`live-preview/livePreview/edges.ts`). */
export const BLOCK_MATH_TYPE = "application/x-oculus-math-block";

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

/** Maths pasted here goes in at the caret (MathLive drops `$` delimiters);
 *  markdown with prose around maths can't live inside maths, so it goes
 *  into the note just after this maths. */
export function paste(ctl: FieldController, e: ClipboardEvent) {
  const data = e.clipboardData;
  const text = data?.getData("text/plain") ?? "";
  if (!data || !text || data.types.includes("application/json+mathlive") || mathOnly(text)) return;
  e.preventDefault();
  e.stopPropagation();
  ctl.flush();
  const { view } = ctl;
  const target = ctl.target();
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
export function copied(ctl: FieldController, e: ClipboardEvent) {
  const data = e.clipboardData;
  const text = data?.getData("text/plain");
  if (!data || !text) return;
  const latex = fromField(tidy(text, !ctl.display));
  data.setData("text/plain", latex);
  if (ctl.display) data.setData(BLOCK_MATH_TYPE, `$$\n${layoutBlock(latex)}\n$$`);
}
