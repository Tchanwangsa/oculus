import { syntaxTree } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";
import type { SyntaxNode } from "@lezer/common";

import type { BlockType } from "./lines";
import { enclosing } from "./shared";

export interface ActiveFormats {
  block: BlockType;
  bold: boolean;
  italic: boolean;
  strike: boolean;
  code: boolean;
  link: boolean;
  bullet: boolean;
  ordered: boolean;
  task: boolean;
  quote: boolean;
  codeBlock: boolean;
  math: boolean;
}

export const NO_FORMATS: ActiveFormats = {
  block: "p",
  bold: false,
  italic: false,
  strike: false,
  code: false,
  link: false,
  bullet: false,
  ordered: false,
  task: false,
  quote: false,
  codeBlock: false,
  math: false,
};

/** What the main selection sits in, from the syntax tree. */
export function activeFormats(state: EditorState): ActiveFormats {
  const main = state.selection.main;
  const ln = state.doc.lineAt(main.head);
  const start = ln.from + (ln.text.length - ln.text.trimStart().length);
  const blocks = new Set<string>();
  let item: SyntaxNode | null = null;
  for (let n: SyntaxNode | null = syntaxTree(state).resolveInner(start, 1); n; n = n.parent) {
    blocks.add(n.name);
    if (n.name === "ListItem" && !item) item = n;
  }
  const heading = [...blocks].map((b) => /^(?:ATX|Setext)Heading(\d)$/.exec(b)?.[1]).find(Boolean);
  const list = item?.parent?.name;
  const task = list === "BulletList" && item?.getChild("Task") != null;
  return {
    block: heading ? (`h${heading}` as BlockType) : "p",
    bold: enclosing(state, main, "StrongEmphasis") != null,
    italic: enclosing(state, main, "Emphasis") != null,
    strike: enclosing(state, main, "Strikethrough") != null,
    code: enclosing(state, main, "InlineCode") != null,
    link: enclosing(state, main, "Link") != null,
    bullet: list === "BulletList" && !task,
    ordered: list === "OrderedList",
    task,
    quote: blocks.has("Blockquote"),
    codeBlock: blocks.has("FencedCode"),
    math: blocks.has("BlockMath") || enclosing(state, main, "InlineMath") != null,
  };
}

export function sameFormats(a: ActiveFormats, b: ActiveFormats): boolean {
  return (Object.keys(a) as (keyof ActiveFormats)[]).every((k) => a[k] === b[k]);
}
