// The app's KaTeX call sites as option sets: each is the renders one call
// site makes for a formula. Paths are under app/src/components/.

import type { Formula } from "./corpus";

/** documents/editor/live-preview/widgets/math.ts's pre-pass:
 *  `\left[\begin{array}…\end{array}\right]` gets `\kern-0.5em` inside the
 *  brackets. A copy, kept in step by hand. */
const LEFT_BEFORE = /\\left\s*(?:\\[a-zA-Z]+|\\.|[^\s\\])\s*$/;
export function hugArrays(source: string): string {
  const BEGIN = "\\begin{array}";
  const END = "\\end{array}";
  if (!source.includes(BEGIN)) return source;
  let out = "";
  let done = 0;
  for (let at = source.indexOf(BEGIN); at >= 0; at = source.indexOf(BEGIN, at + 1)) {
    if (at < done || !LEFT_BEFORE.test(source.slice(0, at))) continue;
    let depth = 0;
    let end = -1;
    for (let i = at; i < source.length; i++) {
      if (source.startsWith(BEGIN, i)) depth++;
      else if (source.startsWith(END, i) && --depth === 0) {
        end = i + END.length;
        break;
      }
    }
    if (end < 0 || !/^\s*\\right/.test(source.slice(end))) continue;
    out += `${source.slice(done, at)}\\kern-0.5em${source.slice(at, end)}\\kern-0.5em`;
    done = end;
  }
  return out + source.slice(done);
}

export type Options = Record<string, unknown>;
export interface Step {
  tex: string;
  options: Options;
}
export interface OptionSet {
  name: string;
  where: string;
  /** The renders the call site makes; a later one runs only if the previous threw. */
  steps: (f: Formula) => Step[];
}

export const SETS: OptionSet[] = [
  {
    // documents/editor/live-preview/widgets/math.ts, `renderMath`.
    name: "a",
    where: "widgets/math.ts (Live render)",
    steps: (f) => [
      { tex: hugArrays(f.tex), options: { displayMode: f.display, throwOnError: true, macros: { "\\arraystretch": "1.2" } } },
    ],
  },
  {
    // documents/editor/math/field/mathField/visual-state.ts, the visual
    // field's parse gate.
    name: "b",
    where: "mathField/visual-state.ts (gate)",
    steps: (f) => [{ tex: f.tex, options: { displayMode: f.display, throwOnError: true, strict: "ignore" } }],
  },
  {
    // documents/editor/math/tools/mathTools/dom.ts, palette and completion
    // previews.
    name: "c",
    where: "mathTools/dom.ts (palette preview)",
    steps: (f) => [{ tex: f.tex, options: { throwOnError: false } }],
  },
  {
    // documents/editor/math/tools/mathTools/popover.ts, the toolbox preview.
    name: "d",
    where: "mathTools/popover.ts (toolbox)",
    steps: (f) => [{ tex: f.tex, options: { displayMode: f.display, throwOnError: true } }],
  },
  {
    // rehype-katex: chat (markdown/MdComponents.tsx) and files
    // (files/FileMarkdown.tsx). Strict first, then a lenient retry.
    name: "e",
    where: "rehype-katex (chat, files)",
    steps: (f) => [
      { tex: f.tex, options: { displayMode: f.display, throwOnError: true } },
      { tex: f.tex, options: { displayMode: f.display, strict: "ignore", throwOnError: false } },
    ],
  },
];
