import { BLOCK_MATH_TYPE } from "@/lib/markdown/math";
import { mathOnly, pasteBeside } from "../../fieldNote";
import { fromField, layoutBlock, tidy } from "../serialize";
import type { FieldController } from "./field-controller";

export { BLOCK_MATH_TYPE };

/** Maths pasted here goes in at the caret (MathLive drops `$` delimiters);
 *  markdown with prose around maths goes beside it (`pasteBeside`). */
export function paste(ctl: FieldController, e: ClipboardEvent) {
  const data = e.clipboardData;
  const text = data?.getData("text/plain") ?? "";
  if (!data || !text || data.types.includes("application/json+mathlive") || mathOnly(text)) return;
  e.preventDefault();
  e.stopPropagation();
  ctl.flush();
  pasteBeside(ctl.view, ctl.target(), text);
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
