import { EditorSelection, Facet, type Extension } from "@codemirror/state";
import { EditorView, RectangleMarker, drawSelection, layer } from "@codemirror/view";
import { selectedMathLines } from "./selectionMath";

/** A span the selection highlight skips. */
export interface SelectionGap {
  from: number;
  to: number;
}

/** Spans with their own selection drawing: Live mode’s display maths.
 *  Inline maths and its surrounding text use the shared line geometry. */
export const selectionGaps = Facet.define<(view: EditorView) => readonly SelectionGap[]>();

/** `[from, to)` less the gaps, in order. */
export function withoutGaps(from: number, to: number, gaps: readonly SelectionGap[]): SelectionGap[] {
  const out: SelectionGap[] = [];
  let at = from;
  for (const g of [...gaps].sort((a, b) => a.from - b.from)) {
    if (g.to <= at || g.from >= to) continue;
    if (g.from > at) out.push({ from: at, to: g.from });
    at = Math.max(at, g.to);
  }
  if (at < to) out.push({ from: at, to });
  return out;
}

/**
 * The note's selection highlight: CodeMirror's `drawSelection` (its cursor,
 * the native selection hidden) with its highlight layer swapped for one that
 * shares text-and-maths line geometry and skips the `selectionGaps`.
 * `drawSelection` offers no option to leave its
 * layer out, so the theme hides it (`.cm-selectionLayer`).
 */
export function noteSelection(): Extension {
  return [
    drawSelection(),
    layer({
      above: true,
      class: "cm-noteSelectionLayer",
      markers(view) {
        const gaps = view.state.facet(selectionGaps).flatMap((f) => f(view));
        return view.state.selection.ranges.flatMap((r) => {
          if (r.empty) return [];
          const shared = selectedMathLines(view, r);
          return [
            ...shared.markers,
            ...withoutGaps(r.from, r.to, [...gaps, ...shared.gaps]).flatMap((part) =>
              RectangleMarker.forRange(view, "cm-selectionBackground", EditorSelection.range(part.from, part.to)),
            ),
          ];
        });
      },
      update: (u) => u.docChanged || u.selectionSet || u.viewportChanged || u.geometryChanged || u.transactions.length > 0,
    }),
    // Above the text, so code's own fill (opaque) does not hide it; the tint is
    // translucent and the layer takes no clicks.
    EditorView.theme({ ".cm-selectionLayer": { display: "none" }, ".cm-noteSelectionLayer": { pointerEvents: "none" } }),
  ];
}
