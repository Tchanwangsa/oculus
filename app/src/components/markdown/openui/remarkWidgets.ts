import type { Root } from "mdast";
import { visitParents } from "unist-util-visit-parents";

/** Index fences in syntax order, independent of React render/StrictMode. */
export function remarkWidgets() {
  return (tree: Root) => {
    let index = 0;
    visitParents(tree, "code", (node) => {
      if (!/^openui(?:-lang)?$/.test(node.lang ?? "")) return;
      node.data ??= {};
      node.data.hProperties = { ...node.data.hProperties, "data-widget-index": index++ };
    });
  };
}
