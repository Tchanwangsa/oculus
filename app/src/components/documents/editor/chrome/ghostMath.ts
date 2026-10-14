import { commonmarkLanguage } from "@codemirror/lang-markdown";
import type { MarkdownParser } from "@lezer/markdown";
import { mathsReady } from "@/lib/maths";
import { mathContextOf } from "../math/mathContext";
import { MathSyntax } from "../math/mathSyntax";
import { fieldHtml } from "../math/field/mathView/render";

const parser = (commonmarkLanguage.parser as MarkdownParser).configure(MathSyntax);

export interface GhostPart {
  text: string;
  html?: string;
  display?: boolean;
}

/** Render only complete maths; code, escapes and unrenderable TeX keep their
 *  exact source. Prose stays text, so suggestions never inject HTML. */
export function ghostParts(text: string, render: boolean): GhostPart[] {
  if (!render || !mathsReady()) return [{ text }];
  const parts: GhostPart[] = [];
  let end = 0;
  parser.parse(text).iterate({
    enter(node) {
      if (node.name !== "InlineMath" && node.name !== "BlockMath") return;
      const math = mathContextOf(node.node);
      if (!math) return false;
      let html: string;
      try {
        html = fieldHtml(text.slice(math.from, math.to), math.display);
      } catch {
        return false;
      }
      if (node.from > end) parts.push({ text: text.slice(end, node.from) });
      parts.push({ text: text.slice(node.from, node.to), html, display: math.display });
      end = node.to;
      return false;
    },
  });
  if (end < text.length) parts.push({ text: text.slice(end) });
  return parts;
}
