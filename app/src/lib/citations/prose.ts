import { fullCitation, ROOTS } from "./parse";

/** A full-shape library path sitting bare in prose: `courses/…`,
 *  `../courses/…`, or absolute (which may cross "Application Support").
 *  It must end in a file extension, plus an optional line suffix. */
const PROSE_PATH = new RegExp(
  "(?<![\\w/.`])(?:(?:\\.\\.\\/)?courses\\/[^\\s`]+|\\/(?:[^\\s/`]+\\/|Application Support\\/)+?" +
    `${ROOTS}\\/[^\\s\`]+)`,
  "g",
);
const TRAILING = /[.,;:!?)\]'"]+$/;
const ENDS_IN_FILE = /\.[A-Za-z0-9]{1,5}(?::L?\d+(?:-L?\d+)?)?$/;

type MdNode = { type: string; value?: string; children?: MdNode[] };

/** Prose text never rewritten: code and maths carry `value`, not children,
 *  so only links' text needs keeping out. */
const SKIP = new Set(["link", "linkReference", "definition", "footnoteDefinition"]);

/** Splits one text value into text and `inlineCode` nodes, or null. */
function splitProse(value: string): MdNode[] | null {
  const out: MdNode[] = [];
  let at = 0;
  for (const m of value.matchAll(PROSE_PATH)) {
    const path = m[0].replace(TRAILING, "");
    if (!ENDS_IN_FILE.test(path) || !fullCitation(path)) continue;
    const i = m.index ?? 0;
    if (i > at) out.push({ type: "text", value: value.slice(at, i) });
    out.push({ type: "inlineCode", value: path });
    at = i + path.length;
  }
  if (!out.length) return null;
  if (at < value.length) out.push({ type: "text", value: value.slice(at) });
  return out;
}

function walkProse(node: MdNode): void {
  const kids = node.children;
  if (!kids || SKIP.has(node.type)) return;
  for (let i = 0; i < kids.length; i++) {
    const kid = kids[i];
    if (kid.type === "text" && kid.value) {
      const parts = splitProse(kid.value);
      if (parts) {
        kids.splice(i, 1, ...parts);
        i += parts.length - 1;
      }
    } else {
      walkProse(kid);
    }
  }
}

/** remark plugin: bare library paths in prose become `inlineCode`, which the
 *  `code` renderer draws as a chip. Runs after remark-math, so maths is
 *  already out of the text nodes. */
export function remarkProsePaths() {
  return (tree: MdNode) => walkProse(tree);
}
