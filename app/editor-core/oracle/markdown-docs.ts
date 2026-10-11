// Document and edit generators for the markdown oracle (`markdown.ts`):
// line-structured markdown, tables, mutations, markup soups, limit cases, the
// nodes to probe, and edit sequences.

import { ChangeSet, Text } from "@codemirror/state";

import type { Rng } from "./driver";

const PREFIXES = [
  "", "", "", "", "", "> ", ">", "> > ", "- ", "* ", "+ ", "1. ", "2) ", "10. ", "  ", "   ", "    ", "\t",
  "> - ", "- > ", "  - ", "    - ", " > ", ">\t", "-\t", "- [ ] ", "- [x] ", "1. [X] ", " ", "     ", "      ",
  "\t\t", " \t", "1.", "-", ">>", "*\t", "123456789. ", "1234567890. ", "0. ", "-    ", "-     x",
];

const BLOCKS = [
  "# ", "## ", "### ", "#### ", "##### ", "###### ", "####### ", "#", "#\t", "```", "```js", "``` python ", "~~~", "~~~~ md", "````",
  "    ", "---", "***", "___", "- - -", " * * *", "===", "--", "-", "$$", "$$ x^2 $$", "$$x", "x$$", "\\[",
  "\\]", "\\[ a \\]", "<div>", "<div class=\"a\">", "</div>", "<span a='1'>", "<!-- c", "-->", "<?php", "?>",
  "<!DOCTYPE html>", "<![CDATA[", "]]>", "<script>", "</script>", "<pre", "[ref]: /url", "[ref]: <u v> 'title'",
  "[ref]:", "[a b]: x \"t\"", "[ ]: x", "| a | b |", "|---|:-:|", "| --- | --- |", "a|b", "-|-", ":-|-:", "a \\| b",
  "[ ] ", "[x] ", "...", "",
];

const INLINE = [
  "word", "word", "a", "b c", " ", "  ", "\t", "*", "**", "***", "_", "__", "~~", "~", "~~~", "`", "``", "[", "]",
  "](", "](/u)", "](/u \"t\")", "][r]", "][]", "(", ")", "![", "<", ">", "&amp;", "&#123;", "&#x1F;", "&bogus",
  "&;", "\\", "\\*", "\\[", "\\\\", "$", "$$", "\\(", "\\)", "x$", "$1", "$ ", "www.ex.com", "https://a.b/c?d.",
  "http://x.y/(z)", "www.a_b.c", "a@b.co", "a.b@c.d.", "mailto:a@b.cd", "xmpp:a@b.cd/r", "<http://x.y>",
  "<a@b.c>", "<a href=\"x\">", "</b>", "<b/>", "<!-- c -->", "<?p?>", "ไทย", "ที่", "😀", "𝒳", "é", "é",
  " ", "　", "|", "\\|", "'t'", "\"t\"", ".", ",", "!", "?", "-", "1", "—", "«", "€",
  "$a$0", "$a$ 0", "$ a$", "$a $",
];

const BREAKS = ["\n", "\n", "\n", "\n", "\r\n", "\r"];

function inlineRun(rng: Rng, n: number): string {
  let s = "";
  for (let i = 0; i < n; i++) s += rng.pick(INLINE);
  return s;
}

/** A document of lines, each a container prefix and a block start and/or
 *  inline text; sometimes with frontmatter. */
export function structured(rng: Rng): string {
  const lines: string[] = [];
  if (rng.chance(0.15)) {
    lines.push("---");
    for (let i = rng.int(4); i > 0; i--) lines.push(rng.pick(["title: x", "tags: [a, b]", "", "- a", "a: 'b'"]));
    if (rng.chance(0.8)) lines.push(rng.pick(["---", "...", "--- ", "----"]));
  }
  const count = rng.chance(0.05) ? rng.logInt(30, 400) : 1 + rng.int(12);
  for (let i = 0; i < count; i++) {
    let line = "";
    const depth = rng.pick([0, 0, 1, 1, 2, 3]);
    for (let d = 0; d < depth; d++) line += rng.pick(PREFIXES);
    if (rng.chance(0.35)) line += rng.pick(BLOCKS);
    if (rng.chance(0.75)) line += inlineRun(rng, rng.int(8));
    if (rng.chance(0.05)) line += rng.pick([" ", "  ", "\t", "\\"]);
    lines.push(line);
  }
  let out = lines[0];
  for (let i = 1; i < lines.length; i++) out += rng.pick(BREAKS) + lines[i];
  return out;
}

const SOUP = "**__~~``[]()!<>#-=+*_\\$|:>  \n\n\n\t1.a)xé😀&;'\"/.@w~$?\r";

/** A short run of markup characters. */
export function soup(rng: Rng): string {
  const chars = Array.from(SOUP);
  let s = "";
  for (let i = rng.int(rng.chance(0.2) ? 160 : 40); i > 0; i--) s += rng.pick(chars);
  return s;
}

/** `n` UTF-16 units of mixed-width filler. */
function filler(rng: Rng, n: number): string {
  let s = "";
  while (s.length < n) s += rng.pick(["a", "ไ", "😀", "é", " ", "b"]);
  return s;
}

/** Constructs whose limits are counted in UTF-16 units, at their edges:
 *  the 64 KiB frontmatter scan, 999-unit link labels, the 30-unit entity
 *  window, 100-char email locals, deep nesting, many sibling blocks. */
export function edges(rng: Rng): string {
  switch (rng.int(10)) {
    case 0: {
      if (rng.chance(0.5)) {
        // The closer's line around the scan's last whole line (65527 is the
        // most `a`s that still find it).
        return `---\n${"a".repeat(65525 + rng.int(9))}\n${rng.pick(["---", "..."])}\nz`;
      }
      const body = filler(rng, 65536 - 12 + rng.int(24) - 4);
      return `---\n${body}\n${rng.pick(["---", "..."])}\nafter *x*`;
    }
    case 6:
    case 7: {
      const label = filler(rng, 995 + rng.int(8));
      return rng.pick([`[${label}]: /u\n`, `[a][${label}]`, `[x]\n\n[${label}]: /u`, `[${label}]`]);
    }
    case 8:
      return rng.pick([`&${"a".repeat(27 + rng.int(4))};`, `&#${"1".repeat(27 + rng.int(4))};`, `&#x${"f".repeat(26 + rng.int(4))};`]);
    case 9:
      return rng.pick([`${"a".repeat(99 + rng.int(3))}@b.co`, `x ${"a.".repeat(50)}${rng.pick(["", "a"])}@b.co`]);
    case 1: {
      const label = filler(rng, 990 + rng.int(20));
      return rng.pick([`[${label}]: /u\n`, `[a][${label}]`, `[x]\n\n[${label}]: /u`]);
    }
    case 2:
      return `&${"a".repeat(26 + rng.int(6))};`;
    case 3:
      return `${"a".repeat(98 + rng.int(4))}@b.co`;
    case 4: {
      let s = "";
      for (let i = rng.int(60); i > 0; i--) s += rng.pick(["> ", "- ", "1. ", "* "]);
      return s + inlineRun(rng, 4);
    }
    default: {
      const parts: string[] = [];
      for (let i = 20 + rng.int(200); i > 0; i--) parts.push(rng.pick(["# h", "para *x*", "---", "> q", "- l"]));
      return parts.join(rng.pick(["\n", "\n\n"]));
    }
  }
}

const TABLE_ATOMS = [
  "a", "b c", " ", "  ", "\t", "*x*", "**y**", "`c|d`", "`c`", "\\|", "|", "\\", "[l](u)", "[l]", "![i](u)", "<b>",
  "&amp;", "~~s~~", "$m$", "$a|b$", "😀", "ไทย", "http://a.b", "www.a.b|", "\\\\|", "\\\\\\|", ":", "-", "[ ]",
  "<a|b>", "\\(|\\)",
];
const DELIMITER_CELLS = ["---", ":--", "--:", ":-:", "-", " --- ", ":---:", "", " ", "--", "---x", "::", ":", "-:-", "—"];

/** A table (or near-table) inside random containers, with odd cell
 *  counts, escaped pipes, interrupting blocks and lazy lines. */
export function table(rng: Rng): string {
  const row = (n: number, cell: () => string) => {
    const parts: string[] = [];
    for (let i = 0; i < n; i++) parts.push(cell());
    return (rng.chance(0.7) ? "|" : "") + parts.join("|") + (rng.chance(0.7) ? "|" : "");
  };
  const cols = 1 + rng.int(4);
  const pre = rng.pick(["", "", "", "> ", "- ", "  ", "> > ", "1. ", ">", " > "]);
  const cell = () => {
    let s = rng.pick(["", " "]);
    for (let i = rng.int(3); i > 0; i--) s += rng.pick(TABLE_ATOMS);
    return s + rng.pick(["", " "]);
  };
  const width = (spread: number) => Math.max(0, cols + (rng.chance(0.15) ? rng.int(spread) - 1 : 0));
  const lines: string[] = [];
  if (rng.chance(0.3)) lines.push(rng.pick(["para", "# h", "> q", "para *x*", "- li", "```", "$$", "<div>", "a|b"]));
  lines.push(row(width(3), cell));
  lines.push(row(width(3), () => rng.pick(DELIMITER_CELLS)));
  for (let i = rng.int(5); i > 0; i--) {
    lines.push(
      rng.chance(0.12)
        ? rng.pick(["", "text", "> q", "# h", "---", "- x", "```", "    code", "a | b", "$$", "<div>", "***", "|", "\\|"])
        : row(cols + rng.int(3) - 1, cell),
    );
  }
  const out = lines.map((l, i) => (i && rng.chance(0.85) ? pre : i ? rng.pick(["", pre.trim(), "> >"]) : pre) + l);
  return out.join(rng.chance(0.05) ? "\r\n" : "\n") + rng.pick(["", "\n", "\n\n", "\nx"]);
}

/** A structured document with a few markup chars inserted or removed. */
export function mutated(rng: Rng): string {
  const chars = Array.from(structured(rng));
  const soupChars = Array.from(SOUP);
  for (let i = 1 + rng.int(6); i > 0; i--) {
    const at = rng.int(chars.length + 1);
    if (rng.chance(0.5)) chars.splice(at, 0, rng.pick(soupChars));
    else chars.splice(at, 1 + rng.int(3));
  }
  return chars.join("");
}

/** Probe positions: random ones, plus the boundaries of random nodes. */
export function probes(rng: Rng, len: number, nodes: string[]): [number, -1 | 0 | 1][] {
  const out: [number, -1 | 0 | 1][] = [];
  const side = () => rng.pick([-1, 0, 1] as const);
  for (let i = 0; i < 4; i++) out.push([rng.int(len + 1), side()]);
  for (let i = 0; i < 6 && nodes.length > 1; i++) {
    const [, from, to] = rng.pick(nodes).split(" ");
    out.push([Number(rng.chance(0.5) ? from : to), side()]);
  }
  return out;
}

/** A code-point boundary of `doc`. */
function boundary(rng: Rng, doc: Text): number {
  const pos = rng.int(doc.length + 1);
  if (pos <= 0 || pos >= doc.length) return pos;
  const c = doc.sliceString(pos, pos + 1).charCodeAt(0);
  return c >= 0xdc00 && c <= 0xdfff ? pos - 1 : pos;
}

/** A position biased toward where block structure turns: the start, the
 *  end, line starts and line ends; otherwise any code-point boundary. */
function editPos(rng: Rng, doc: Text): number {
  switch (rng.int(8)) {
    case 0:
      return 0;
    case 1:
      return doc.length;
    case 2:
    case 3:
      return doc.line(1 + rng.int(doc.lines)).from;
    case 4:
      return doc.line(1 + rng.int(doc.lines)).to;
    default:
      return boundary(rng, doc);
  }
}

/** The boundary after the code point at `pos` (or `pos` at the end). */
function nextBoundary(doc: Text, pos: number): number {
  if (pos >= doc.length) return pos;
  const c = doc.sliceString(pos, pos + 1).charCodeAt(0);
  return Math.min(doc.length, pos + (c >= 0xd800 && c <= 0xdbff ? 2 : 1));
}

const TOGGLES = [">", "-", "|", "`", "$", "=", "#", "*", "_", "[", "]", "\n", " ", "~", "<", "\\", ":", "1.", "\t"];

/** Text an edit inserts: inline pieces, block starts, line breaks, one
 *  markup character. */
function insertion(rng: Rng): string {
  if (rng.chance(0.35)) return rng.pick(TOGGLES);
  switch (rng.int(5)) {
    case 0:
      return "";
    case 1:
      return inlineRun(rng, 1 + rng.int(3));
    case 2:
      return `\n${rng.pick(BLOCKS)}${inlineRun(rng, rng.int(3))}`;
    case 3:
      return `${rng.pick(PREFIXES)}${rng.pick(BLOCKS)}\n`;
    default:
      return rng.pick(["\n", "\n\n", "|", "-|-\n", "```", "$$", "\n---\n", "> ", "- ", "`", "*", "[", "]"]);
  }
}

/** Up to four steps of one to three disjoint replacements each. */
export function editSteps(rng: Rng, start: Text): { steps: [number, number, string][][]; docs: Text[] } {
  const steps: [number, number, string][][] = [];
  const docs: Text[] = [];
  let doc = start;
  for (let n = 1 + rng.int(4); n > 0; n--) {
    // Frontmatter moved off the first line, or broken there.
    if (doc.sliceString(0, 3) === "---" && rng.chance(0.3)) {
      const step: [number, number, string][] = [
        rng.chance(0.7) ? [0, 0, rng.pick(["\n", "p\n\n", "\n---\n", "x\n", "> "])] : [0, 1, ""],
      ];
      doc = ChangeSet.of(step.map(([from, to, insert]) => ({ from, to, insert })), doc.length).apply(doc);
      steps.push(step);
      docs.push(doc);
      continue;
    }
    // Insertions, one-char deletions and ranges, sorted and disjoint.
    const edits: [number, number][] = Array.from({ length: 1 + rng.int(3) }, () => {
      const from = editPos(rng, doc);
      const kind = rng.int(4);
      const to = kind < 2 ? from : kind === 2 ? nextBoundary(doc, from) : Math.max(from, boundary(rng, doc));
      return [from, to];
    });
    edits.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
    const step: [number, number, string][] = [];
    let lastEnd = -1;
    for (const [from, to] of edits) {
      if (from < lastEnd || (from === lastEnd && step.length && step[step.length - 1][0] === from)) continue;
      step.push([from, to, insertion(rng)]);
      lastEnd = Math.max(to, from + 1);
    }
    doc = ChangeSet.of(step.map(([from, to, insert]) => ({ from, to, insert })), doc.length).apply(doc);
    steps.push(step);
    docs.push(doc);
  }
  return { steps, docs };
}
