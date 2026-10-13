import { EditorSelection } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import { currentHunk, setCurrentHunk, versionDiff } from "./diffDecorations";

/** The hunk after (`step` 1) or before (-1) the current one, wrapping;
 *  from none, the first or the last. Null when there are none. */
export function stepHunk(current: number | null, count: number, step: 1 | -1): number | null {
  if (!count) return null;
  if (current == null) return step > 0 ? 0 : count - 1;
  return (current + step + count) % count;
}

/** Move to the next or previous hunk: centre it in the page's scroller and
 *  flash it. Returns the new index. */
export function goToHunk(view: EditorView, step: 1 | -1): number | null {
  const hunks = view.state.facet(versionDiff)?.hunks ?? [];
  const next = stepHunk(currentHunk(view.state), hunks.length, step);
  if (next == null) return null;
  const { fromB, toB } = hunks[next];
  view.dispatch({
    effects: [
      setCurrentHunk.of(next),
      EditorView.scrollIntoView(EditorSelection.range(fromB, toB), { y: "center" }),
    ],
  });
  return next;
}
