import { syntaxTree } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";
import type { SyntaxNodeRef } from "@lezer/common";

import { imageParts, isBlockImage, isDrawnMermaid, mathParts } from "../editor/live-preview/livePreview/shared";
import { parseTable } from "../editor/live-preview/tableModel";
import { ownsLines } from "../editor/math/mathContext";
import { inlineCodeCitation } from "../editor/syntax/mentionSyntax";

/** A construct Live mode draws as a widget in place of its source, so a
 *  mark or widget inside it would never show. `block` ones are block
 *  widgets, whole lines. */
export interface Construct {
  from: number;
  to: number;
  block: boolean;
}

/**
 * The widget construct `node` is, read with the rules of
 * `live-preview/livePreview/blocks.ts` and `inline.ts` for a view that is
 * never focused (nothing revealed), or null.
 */
function constructOf(state: EditorState, node: SyntaxNodeRef): Construct | null {
  const lineTo = () => state.doc.lineAt(node.to).to;
  switch (node.name) {
    case "BlockMath": {
      if (!mathParts(state, node.node)) return null;
      if (ownsLines(state, node)) return { from: node.from, to: lineTo(), block: true };
      const single = state.doc.lineAt(node.from).number === state.doc.lineAt(node.to).number;
      return single ? { from: node.from, to: node.to, block: false } : null;
    }
    case "InlineMath":
      return mathParts(state, node.node) ? { from: node.from, to: node.to, block: false } : null;
    case "HorizontalRule":
      return ownsLines(state, node) ? { from: node.from, to: lineTo(), block: true } : null;
    case "Table": {
      if (!ownsLines(state, node)) return null;
      const to = lineTo();
      return parseTable(state.sliceDoc(node.from, to)) ? { from: node.from, to, block: true } : null;
    }
    case "Frontmatter":
      return { from: node.from, to: lineTo(), block: true };
    case "Image": {
      if (!imageParts(state, node.node)) return null;
      if (!isBlockImage(state, node)) return { from: node.from, to: node.to, block: false };
      const ln = state.doc.lineAt(node.from);
      return { from: ln.from, to: ln.to, block: true };
    }
    case "FencedCode":
      return isDrawnMermaid(state, node) ? { from: node.from, to: lineTo(), block: true } : null;
    case "InlineCode":
      return inlineCodeCitation(state.sliceDoc(node.from, node.to)) ? { from: node.from, to: node.to, block: false } : null;
    case "TaskMarker":
      return { from: node.from, to: node.to, block: false };
  }
  return null;
}

/** The widget constructs overlapping `from..to`; for an empty range, those
 *  holding `from` strictly inside, since one at its edge still shows a
 *  widget placed there. Outermost only. */
export function constructsIn(state: EditorState, from: number, to: number): Construct[] {
  const out: Construct[] = [];
  syntaxTree(state).iterate({
    from,
    to,
    enter: (node) => {
      const inside = from === to ? node.from < from && node.to > from : node.from < to && node.to > from;
      if (!inside) return node.from <= to && node.to >= from ? undefined : false;
      const c = constructOf(state, node);
      if (!c) return undefined;
      // A widget's span can run past its node (a block's line end).
      const hit = from === to ? c.from < from && c.to > from : c.from < to && c.to > from;
      if (hit) out.push(c);
      return false;
    },
  });
  return out;
}
