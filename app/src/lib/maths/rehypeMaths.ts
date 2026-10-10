/**
 * Renders maths in a hast tree with the maths engine: a port of rehype-katex
 * 7.0.1 (MIT, © Titus Wormer and contributors) onto `renderToString`, with
 * its tree shape kept, so `blockAnchorPlugins` still finds a display formula
 * at the index of the `<pre>` it replaced.
 *
 * Elements with a `math-inline`, `math-display` or `language-math` class are
 * maths, and `<pre><code class="language-math">` is display maths replacing
 * the `<pre>`. A formula renders strictly first; on an error the file gets a
 * message and a lenient retry (`strict: "ignore"`, `throwOnError: false`)
 * draws KaTeX's red error span; an error that retry throws too becomes a
 * similar span. Before the engine is ready a formula is a quiet placeholder
 * (`.md-math-pending`), and the components rendering it re-render on ready
 * (`useMathsReady`).
 */
import type { Element, ElementContent, Root } from "hast";
import { fromHtmlIsomorphic } from "hast-util-from-html-isomorphic";
import { toText } from "hast-util-to-text";
import { SKIP, visitParents } from "unist-util-visit-parents";
import type { VFile } from "vfile";
import { mathsReady } from "./engine";
import { renderToString } from "./render";

const NO_CLASSES: readonly unknown[] = [];

/** Where a failed formula is, for the file's message. */
type Where = { ancestors: (Root | Element)[]; place: Element["position"] };

function rendered(value: string, displayMode: boolean, file: VFile, where: Where): ElementContent[] {
  if (!mathsReady()) {
    const className = displayMode ? ["md-math-pending", "md-math-pending-display"] : ["md-math-pending"];
    return [{ type: "element", tagName: "span", properties: { className }, children: [{ type: "text", value }] }];
  }
  let result: string;
  try {
    result = renderToString(value, { displayMode, throwOnError: true });
  } catch (error) {
    const cause = error as Error;
    file.message("Could not render maths", {
      ...where,
      cause,
      ruleId: cause.name.toLowerCase(),
      source: "rehype-maths",
    });
    try {
      result = renderToString(value, { displayMode, strict: "ignore", throwOnError: false });
    } catch {
      return [
        {
          type: "element",
          tagName: "span",
          properties: { className: ["katex-error"], style: "color:#cc0000", title: String(error) },
          children: [{ type: "text", value }],
        },
      ];
    }
  }
  return fromHtmlIsomorphic(result, { fragment: true }).children as ElementContent[];
}

export default function rehypeMaths() {
  return (tree: Root, file: VFile) => {
    visitParents(tree, "element", (element, parents) => {
      const classes = Array.isArray(element.properties.className) ? element.properties.className : NO_CLASSES;
      const languageMath = classes.includes("language-math");
      const mathDisplay = classes.includes("math-display");
      const mathInline = classes.includes("math-inline");
      if (!languageMath && !mathDisplay && !mathInline) return;

      let parent = parents[parents.length - 1];
      let scope = element;
      let displayMode = mathDisplay;
      // A ```math fence: the `<pre>` goes, as display maths.
      if (element.tagName === "code" && languageMath && parent?.type === "element" && parent.tagName === "pre") {
        scope = parent;
        parent = parents[parents.length - 2];
        displayMode = true;
      }
      if (!parent) return;

      const value = toText(scope, { whitespace: "pre" });
      const where = { ancestors: [...parents, element], place: element.position };
      const result = rendered(value, displayMode, file, where);
      parent.children.splice(parent.children.indexOf(scope), 1, ...result);
      return SKIP;
    });
  };
}
