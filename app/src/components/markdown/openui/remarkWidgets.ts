import type { Code, Paragraph, Root } from "mdast";
import type { VFile } from "vfile";
import { visitParents } from "unist-util-visit-parents";

/** The fences that hold a widget: OpenUI's and the sandboxed ```widget. */
const NUMBERED = /^(?:openui(?:-lang)?|widget)$/;

/** Whether `text` may hold a widget fence: a superset of what `remarkWidgets`
 *  numbers (it also matches one inside a longer code block), so a fold that
 *  trusts it never hides one. Cheap enough for every row on every rebuild. */
export const mayHoldWidget = (text: string | null | undefined) =>
  !!text && /(?:`{3,}|~{3,})[ \t]*(?:openui|widget)\b/.test(text);

/**
 * Index widget fences in syntax order, independent of React render/StrictMode.
 * One counter for both kinds, so a reply's `widgetKey`s never collide.
 */
export function remarkWidgets() {
  return (tree: Root, file: VFile) => {
    let index = 0;
    visitParents(tree, "code", (node) => {
      if (!NUMBERED.test(node.lang ?? "")) return;
      node.data ??= {};
      const raw = String(file.value).slice(node.position?.start.offset, node.position?.end.offset);
      const lines = raw.split("\n");
      const opening = /^\s*(`{3,}|~{3,})/.exec(lines[0] ?? "")?.[1];
      const closing = /^(?:[ \t]*>[ \t]*)*\s*(`{3,}|~{3,})\s*$/.exec(lines[lines.length - 1] ?? "")?.[1];
      const closed = lines.length > 1 && !!opening && !!closing && opening[0] === closing[0] && closing.length >= opening.length;
      node.data.hProperties = { ...node.data.hProperties, "data-widget-index": index++, "data-fence-closed": closed };
    });
  };
}

/** Which part of a reply to draw once its fences are numbered: only its widget
 *  fences (`artifacts`), or everything else with each fence a pointer to where
 *  it is drawn (`prose`). Splits the parsed tree, so the source is never
 *  reserialised and no fence is renumbered. */
export type FenceSplit = "artifacts" | "prose";

/** Runs after `remarkWidgets`, which has given each fence its index. */
export function remarkFenceSplit(split: FenceSplit) {
  return (tree: Root) => {
    const fences: Code[] = [];
    visitParents(tree, "code", (node, parents) => {
      if (!NUMBERED.test(node.lang ?? "")) return;
      if (split === "artifacts") {
        fences.push(node);
        return;
      }
      const parent = parents[parents.length - 1];
      const at = parent.children.indexOf(node as never);
      const pointer: Paragraph = {
        type: "paragraph",
        children: [],
        data: { hName: "widget-pointer", hProperties: { "data-lang": node.lang ?? "" } },
      };
      parent.children.splice(at, 1, pointer as never);
    });
    if (split === "artifacts") tree.children = fences;
  };
}
