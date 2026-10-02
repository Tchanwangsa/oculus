import { syntaxTree } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";
import type { SyntaxNode } from "@lezer/common";

/** Search from the innermost node outwards. Bias matters at markup boundaries;
 *  callers choose which side owns a caret touching two adjacent constructs. */
export function ancestorAt(
  state: EditorState,
  pos: number,
  matches: (node: SyntaxNode) => boolean,
  sides: readonly (-1 | 1)[] = [1, -1],
): SyntaxNode | null {
  const tree = syntaxTree(state);
  for (const side of sides) {
    for (let node: SyntaxNode | null = tree.resolveInner(pos, side); node; node = node.parent) {
      if (matches(node)) return node;
    }
  }
  return null;
}
