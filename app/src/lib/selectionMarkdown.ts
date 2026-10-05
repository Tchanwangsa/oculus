import type { ClipboardEvent, DragEvent } from "react";

/**
 * A partial selection in rendered markdown (a chat reply, a parsed PDF),
 * walked back into markdown (a whole message copies its source instead).
 * Special cases:
 * - KaTeX renders twice (MathML + spans); the TeX comes from its `<annotation>`
 *   and the subtree is never descended into.
 * - A mermaid figure carries its fence source in `data-md` (`Mermaid.tsx`), as
 *   does an embedded picture or page (`OutputEmbed.tsx`) — inline, since it
 *   usually sits in a `<p>`.
 * - `data-copy-skip` marks chrome that happens to be selectable.
 */

/** Tags that start a new block; everything else is inline. */
const BLOCK = new Set([
  "ADDRESS", "ARTICLE", "ASIDE", "BLOCKQUOTE", "DIV", "DL", "DD", "DT",
  "FIELDSET", "FIGCAPTION", "FIGURE", "FOOTER", "FORM", "H1", "H2", "H3",
  "H4", "H5", "H6", "HEADER", "HR", "LI", "MAIN", "NAV", "OL", "P", "PRE",
  "SECTION", "TABLE", "UL",
]);

function skipped(el: Element): boolean {
  return (
    el.hasAttribute("data-copy-skip") ||
    el.tagName === "SCRIPT" ||
    el.tagName === "STYLE" ||
    // An SVG `tagName` keeps its source case.
    el.localName === "svg"
  );
}

function intersects(node: Node, range: Range): boolean {
  try {
    return range.intersectsNode(node);
  } catch {
    return false;
  }
}

function clip(node: Text, range: Range): string {
  const start = node === range.startContainer ? range.startOffset : 0;
  const end = node === range.endContainer ? range.endOffset : node.data.length;
  return node.data.slice(start, end);
}

/** Whitespace collapsed as drawn, unless the box is `pre*` (question bubbles). */
function clipText(node: Text, range: Range): string {
  const raw = clip(node, range);
  const ws = node.parentElement ? getComputedStyle(node.parentElement).whiteSpace : "normal";
  return ws.startsWith("pre") ? raw : raw.replace(/[\t\n ]+/g, " ");
}

function tex(el: Element): string | null {
  const annotation = el.querySelector('annotation[encoding="application/x-tex"]');
  const source = annotation?.textContent?.trim();
  return source ? source : null;
}

function isKatex(el: Element): boolean {
  return el.classList.contains("katex") || el.classList.contains("katex-display");
}

/** Edge spaces go outside the markers: `** bold **` is not bold. */
function wrap(marker: string, body: string): string {
  const m = /^(\s*)([\s\S]*?)(\s*)$/.exec(body);
  if (!m || !m[2]) return body;
  return `${m[1]}${marker}${m[2]}${marker}${m[3]}`;
}

function inlineChildren(el: Element, range: Range): string {
  let out = "";
  for (const child of Array.from(el.childNodes)) {
    if (!intersects(child, range)) continue;
    out += inline(child, range);
  }
  return out;
}

function inline(node: Node, range: Range): string {
  if (node.nodeType === Node.TEXT_NODE) return clipText(node as Text, range);
  if (node.nodeType !== Node.ELEMENT_NODE) return "";
  const el = node as Element;
  if (skipped(el)) return "";
  if (isKatex(el)) {
    const source = tex(el);
    return source ? `$${source}$` : "";
  }
  const md = el.getAttribute("data-md");
  if (md) return md.trim();
  // A mention chip (`FileChip.tsx`) copies as its path.
  const path = el.getAttribute("data-path");
  if (path) return `\`${path}\``;

  switch (el.tagName) {
    case "BR":
      return "\n";
    case "STRONG":
    case "B":
      return wrap("**", inlineChildren(el, range));
    case "EM":
    case "I":
      return wrap("*", inlineChildren(el, range));
    case "DEL":
    case "S":
      return wrap("~~", inlineChildren(el, range));
    case "CODE":
      return wrap("`", inlineChildren(el, range));
    case "IMG": {
      const img = el as HTMLImageElement;
      return `![${img.alt}](${img.getAttribute("src") ?? ""})`;
    }
    case "A": {
      const href = el.getAttribute("href");
      const body = inlineChildren(el, range);
      return href ? `[${body}](${href})` : body;
    }
    default:
      return inlineChildren(el, range);
  }
}

function tidy(text: string): string {
  return text.replace(/[ \t]+\n/g, "\n").replace(/\n[ \t]+/g, "\n").trim();
}

/** A container's contents as blocks; inline runs between blocks become
 *  paragraphs of their own. */
function container(el: Element, range: Range): string[] {
  const out: string[] = [];
  let buffer = "";
  const flush = () => {
    const text = tidy(buffer);
    if (text) out.push(text);
    buffer = "";
  };
  for (const child of Array.from(el.childNodes)) {
    if (!intersects(child, range)) continue;
    if (child.nodeType === Node.TEXT_NODE) {
      buffer += clipText(child as Text, range);
      continue;
    }
    if (child.nodeType !== Node.ELEMENT_NODE) continue;
    const kid = child as Element;
    if (skipped(kid)) continue;
    if (BLOCK.has(kid.tagName) || kid.classList.contains("katex-display") || kid.hasAttribute("data-md")) {
      flush();
      out.push(...block(kid, range));
    } else {
      buffer += inline(kid, range);
    }
  }
  flush();
  return out;
}

function fence(el: Element, range: Range): string[] {
  const code = el.querySelector("code");
  const lang = /language-([\w-]+)/.exec(code?.className ?? "")?.[1] ?? "";
  // Whitespace is the content here, so the text nodes are taken raw.
  let body = "";
  const walk = (node: Node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      body += clip(node as Text, range);
      return;
    }
    for (const kid of Array.from(node.childNodes)) {
      if (intersects(kid, range)) walk(kid);
    }
  };
  walk(code ?? el);
  body = body.replace(/\n+$/, "");
  return body ? [`\`\`\`${lang}\n${body}\n\`\`\``] : [];
}

function list(el: Element, range: Range): string[] {
  const ordered = el.tagName === "OL";
  let n = Number(el.getAttribute("start") ?? 1);
  const rows: string[] = [];
  for (const item of Array.from(el.children)) {
    if (item.tagName !== "LI") continue;
    if (!intersects(item, range)) {
      n += 1;
      continue;
    }
    const blocks = container(item, range);
    if (blocks.length) {
      const marker = ordered ? `${n}. ` : "- ";
      const pad = " ".repeat(marker.length);
      rows.push(
        blocks
          // No blank line before a nested list, or the whole list turns loose.
          .reduce((acc, b) => (acc ? `${acc}${/^([-*]|\d+\.) /.test(b) ? "\n" : "\n\n"}${b}` : b), "")
          .split("\n")
          .map((line, i) => (i === 0 ? marker + line : line ? pad + line : line))
          .join("\n"),
      );
    }
    n += 1;
  }
  return rows.length ? [rows.join("\n")] : [];
}

function table(el: Element, range: Range): string[] {
  const rows: string[][] = [];
  for (const tr of Array.from(el.querySelectorAll("tr"))) {
    if (!intersects(tr, range)) continue;
    const cells = Array.from(tr.children).map((cell) =>
      tidy(inlineChildren(cell, range)).replace(/\n/g, " ").replace(/\|/g, "\\|"),
    );
    if (cells.length) rows.push(cells);
  }
  if (!rows.length) return [];
  const width = Math.max(...rows.map((r) => r.length));
  const pad = (r: string[]) => [...r, ...Array(width - r.length).fill("")];
  const line = (r: string[]) => `| ${pad(r).join(" | ")} |`;
  // The first selected row stands as the header.
  const head = line(rows[0]);
  const rule = `|${Array(width).fill(" --- ").join("|")}|`;
  return [[head, rule, ...rows.slice(1).map(line)].join("\n")];
}

function block(el: Element, range: Range): string[] {
  if (el.hasAttribute("data-md")) {
    const source = el.getAttribute("data-md") ?? "";
    return source.trim() ? [source.trim()] : [];
  }
  if (el.classList.contains("katex-display")) {
    const source = tex(el);
    return source ? [`$$\n${source}\n$$`] : [];
  }
  switch (el.tagName) {
    case "H1":
    case "H2":
    case "H3":
    case "H4":
    case "H5":
    case "H6": {
      const text = tidy(inlineChildren(el, range));
      return text ? [`${"#".repeat(Number(el.tagName[1]))} ${text}`] : [];
    }
    case "P": {
      const text = tidy(inlineChildren(el, range));
      return text ? [text] : [];
    }
    case "HR":
      return ["---"];
    case "PRE":
      return fence(el, range);
    case "UL":
    case "OL":
      return list(el, range);
    case "BLOCKQUOTE": {
      const quoted = container(el, range).join("\n\n");
      if (!quoted) return [];
      return [
        quoted
          .split("\n")
          .map((l) => (l ? `> ${l}` : ">"))
          .join("\n"),
      ];
    }
    case "TABLE":
      return table(el, range);
    default:
      return container(el, range);
  }
}

/** The selection as markdown, or "" to leave the browser's copy alone. */
export function selectionMarkdown(selection: Selection | null): string {
  if (!selection || selection.rangeCount === 0 || selection.isCollapsed) return "";
  const range = selection.getRangeAt(0);
  const root = range.commonAncestorContainer;
  if (root.nodeType === Node.TEXT_NODE) return tidy(clipText(root as Text, range));
  if (root.nodeType !== Node.ELEMENT_NODE) return "";
  return container(root as Element, range).join("\n\n").trim();
}

/** The selection under `target` as markdown, or "" — a selection inside a
 *  field belongs to the field, and it is already text. */
function markdownFor(target: EventTarget | null): string {
  if (target instanceof Element && target.closest("input, textarea, [contenteditable='true']")) {
    return "";
  }
  return selectionMarkdown(window.getSelection());
}

/** `onCopy` for rendered markdown: the selection goes out as `text/plain`
 *  markdown only. */
export function copyAsMarkdown(e: ClipboardEvent) {
  const md = markdownFor(e.target);
  if (!md) return;
  e.clipboardData.setData("text/plain", md);
  // Without this the browser writes its own flavours over ours.
  e.preventDefault();
}

/** The same, dragged out. No `preventDefault` — see docs/ui.md (WebKit drag). */
export function dragAsMarkdown(e: DragEvent) {
  const md = markdownFor(e.target);
  if (md) e.dataTransfer.setData("text/plain", md);
}
