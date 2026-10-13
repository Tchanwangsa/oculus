import { type SelectionRange } from "@codemirror/state";
import { Direction, EditorView, RectangleMarker } from "@codemirror/view";
import { frameOrigin } from "@/lib/maths/geometry";
import { paddedBands } from "@/lib/maths/geometry/paint";
import { selectionLines } from "@/lib/markdown/mathSelection/lines";
import type { SelectionGap } from "./selectionLayer";

/** CodeMirror supplies the selected source range and marker origin; the
 *  geometry is the same text-and-maths pipeline used by rendered paragraphs. */
export function selectedMathLines(view: EditorView, selection: SelectionRange) {
  const markers: RectangleMarker[] = [];
  const gaps: SelectionGap[] = [];
  const from = Math.max(selection.from, view.viewport.from);
  const to = Math.min(selection.to, view.viewport.to);
  if (to <= from) return { markers, gaps };
  const start = view.domAtPos(from), end = view.domAtPos(to);
  const range = document.createRange();
  range.setStart(start.node, start.offset);
  range.setEnd(end.node, end.offset);
  const scroll = view.scrollDOM;
  const rect = scroll.getBoundingClientRect();
  const left = view.textDirection === Direction.LTR ? rect.left : rect.right - scroll.clientWidth * view.scaleX;
  const base = { x: left - scroll.scrollLeft * view.scaleX, y: rect.top - scroll.scrollTop * view.scaleY };
  const origin = frameOrigin(scroll);
  for (const line of view.contentDOM.querySelectorAll<HTMLElement>(".cm-line")) {
    if (!line.querySelector(".cm-math .katex") || !range.intersectsNode(line)) continue;
    const boxes = selectionLines(line, range, scroll);
    if (!boxes?.length) continue;
    const source = view.state.doc.lineAt(view.posAtDOM(line));
    gaps.push({ from: source.from, to: Math.min(source.to + 1, view.state.doc.length) });
    for (const b of boxes) {
      markers.push(new RectangleMarker("cm-selectionBackground",
        b.left + origin.x - base.x, b.top + origin.y - base.y,
        b.right - b.left, b.bottom - b.top));
    }
  }
  const boxes = markers.map(m => ({
    left: m.left, right: m.left + (m.width ?? 0), top: m.top, bottom: m.top + m.height,
  })).sort((a, b) => a.top - b.top);
  return {
    markers: paddedBands(boxes, 0).map(b => new RectangleMarker("cm-selectionBackground",
      b.left, b.top, b.right - b.left, b.bottom - b.top)),
    gaps,
  };
}
