import type { FieldChange, MathField } from "@/lib/maths";
import { frameOrigin, union, type Box } from "@/lib/maths/geometry";
import type { MathView } from "./index";
import { fieldHtml } from "./render";

/** The LaTeX the pending `\name` is drawn as: its source, in typewriter. */
export const pendingTex = (name: string) => `\\texttt{\\textbackslash ${name}}`;

/** Each offset of `next` back in `source`, or -1 for a character the change
 *  inserted: the change's own text, with `marked` (the pending LaTeX at
 *  `at` in `next`) left out, holds the old text it replaced in order, with
 *  braces or spaces the model added around it (`x^2` → `x^{2…}`). */
function backMap(source: string, change: FieldChange, at: number, marked: number): Int32Array {
  const shift = change.insert.length - (change.to - change.from);
  const map = new Int32Array(source.length + shift).fill(-1);
  for (let i = 0; i < change.from; i++) map[i] = i;
  for (let i = change.to; i < source.length; i++) map[i + shift] = i;
  let old = change.from;
  for (let j = change.from; j < change.from + change.insert.length && old < change.to; j++) {
    if (j >= at && j < at + marked) continue;
    if (change.insert[j - change.from] === source[old]) map[j] = old++;
  }
  return map;
}

/** The source map of `html`, rendered from `next`, moved back onto the
 *  field's own source: an element of only the pending LaTeX loses its range
 *  and is marked `data-pending`, one that is only inserted braces loses its
 *  range, and every other keeps the source it covers. */
function unpendMap(html: string, map: Int32Array, sourceLength: number, at: number, marked: number): string {
  const startOf = (t: number) => {
    while (t < map.length && map[t] < 0) t++;
    return t < map.length ? map[t] : sourceLength;
  };
  const endOf = (t: number) => {
    while (t > 0 && map[t - 1] < 0) t--;
    return t > 0 ? map[t - 1] + 1 : 0;
  };
  return html.replace(/ data-s="(\d+)" data-e="(\d+)"/g, (_, sText: string, eText: string) => {
    const s = Number(sText);
    const e = Number(eText);
    if (s < e && s >= at && e <= at + marked) return " data-pending";
    if (s === e) {
      const t = startOf(s);
      return ` data-s="${t}" data-e="${t}"`;
    }
    const from = startOf(s);
    const to = endOf(e);
    return from < to ? ` data-s="${from}" data-e="${to}"` : "";
  });
}

/**
 * The field's rendering with the pending `\name` typed in at the caret as the
 * model would type it (`pendingTex` as a template at a collapsed caret), so
 * the maths after it moves aside; display only, never the field's source.
 * Every other element keeps its range in the field's source. Null when the
 * model would not take it there or the engine fails on it.
 */
export function pendingHtml(field: MathField, display: boolean): string | null {
  const name = field.pending;
  if (name == null) return null;
  const marked = pendingTex(name);
  const plain = field.withPending(undefined);
  const caret = plain.select(field.head, field.head);
  try {
    const step = caret.run({ template: marked });
    const next = step.field.source;
    step.field.free();
    const [change] = step.changes;
    if (step.changes.length !== 1 || !change) return null;
    const at = next.indexOf(marked, change.from);
    if (at < 0) return null;
    const html = fieldHtml(next, display);
    return unpendMap(html, backMap(field.source, change, at, marked.length), field.source.length, at, marked.length);
  } catch {
    // Display only: a trap here leaves the plain rendering.
    return null;
  } finally {
    plain.free();
    caret.free();
  }
}

/** The pending `\name`'s box in the frame, or null when none is drawn. */
export function pendingBox(view: MathView): Box | null {
  const marks = view.rendered.querySelectorAll("[data-pending]");
  if (!marks.length) return null;
  const o = frameOrigin(view.frame);
  let box: Box | null = null;
  for (const el of marks) {
    const r = el.getBoundingClientRect();
    if (!r.width && !r.height) continue;
    const b = { left: r.left - o.x, top: r.top - o.y, right: r.right - o.x, bottom: r.bottom - o.y };
    box = box ? union(box, b) : b;
  }
  return box;
}

/** In command mode the caret and the textarea sit after the `\name`, which
 *  the rendering holds; the caret is hidden while a selection is drawn. */
export function drawPending(view: MathView) {
  const box = view.field.mode === "command" ? pendingBox(view) : null;
  if (!box) return;
  const { caret, input, field } = view;
  const [from, to] = field.selected;
  caret.hidden = from !== to;
  caret.style.left = input.style.left = `${box.right}px`;
  if (!Number.isFinite(view.measured.x[field.head])) {
    caret.style.top = input.style.top = `${box.top}px`;
    caret.style.height = input.style.height = `${box.bottom - box.top}px`;
  }
}
