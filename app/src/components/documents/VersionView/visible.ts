import { syntaxTree } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";

/** Markup Live mode hides while nothing reveals it (`live-preview/livePreview/inline.ts`). */
const HIDDEN = new Set(["EmphasisMark", "StrikethroughMark", "CodeMark", "LinkMark", "LinkTitle", "HeaderMark", "QuoteMark"]);

/**
 * Whether `from..to` draws anything a mark could tint: some character that
 * is neither whitespace nor hidden markup. A mark over nothing else would
 * draw as an empty box.
 */
export function visibleIn(state: EditorState, from: number, to: number): boolean {
  if (!/\S/.test(state.sliceDoc(from, to))) return false;
  const hidden: { from: number; to: number }[] = [];
  syntaxTree(state).iterate({
    from,
    to,
    enter: (node) => {
      const url = node.name === "URL" && node.node.parent?.name === "Link";
      if (HIDDEN.has(node.name) || url) hidden.push({ from: node.from, to: node.to });
    },
  });
  for (let pos = from; pos < to; pos++) {
    if (/\s/.test(state.sliceDoc(pos, pos + 1))) continue;
    if (!hidden.some((h) => h.from <= pos && pos < h.to)) return true;
  }
  return false;
}
