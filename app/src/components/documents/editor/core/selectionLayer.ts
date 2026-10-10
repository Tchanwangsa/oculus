import { EditorSelection, Facet, type Extension } from "@codemirror/state";
import { EditorView, RectangleMarker, drawSelection, layer } from "@codemirror/view";

/** A span the selection highlight skips. */
export interface SelectionGap {
  from: number;
  to: number;
}

/** Spans whose own drawing shows them selected, which the highlight leaves
 *  out: Live mode's rendered maths, which paints bands over its atoms
 *  (`MathWidget`) rather than sitting in a box of highlight. */
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
 * skips the `selectionGaps`. `drawSelection` offers no option to leave its
 * layer out, so the theme hides it (`.cm-selectionLayer`).
 */
export function noteSelection(): Extension {
  return [
    drawSelection(),
    layer({
      above: false,
      class: "cm-noteSelectionLayer",
      markers(view) {
        const gaps = view.state.facet(selectionGaps).flatMap((f) => f(view));
        return view.state.selection.ranges.flatMap((r) =>
          r.empty
            ? []
            : withoutGaps(r.from, r.to, gaps).flatMap((part) =>
                RectangleMarker.forRange(view, "cm-selectionBackground", EditorSelection.range(part.from, part.to)),
              ),
        );
      },
      update: (u) => u.docChanged || u.selectionSet || u.viewportChanged || u.geometryChanged || u.transactions.length > 0,
    }),
    EditorView.theme({ ".cm-selectionLayer": { display: "none" } }),
  ];
}
