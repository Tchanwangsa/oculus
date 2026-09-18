/**
 * A selection in a rendered reply, back as the markdown it was written as.
 *
 * The timeline draws an answer through `ReactMarkdown`, so what a browser
 * copies out of it is what it *shows*: headings lose their `#`, a table
 * becomes a run of words, a formula comes out as KaTeX's own glyph soup, and
 * a fenced block loses its fence. Pasted into notes or another agent, none of
 * it is the text the agent wrote.
 *
 * The obvious fix — copy the message's source string — only answers for a
 * whole message, which is what the Copy button under each bubble already
 * does. A drag across half an answer has no source string, so this walks the
 * live DOM inside the selection instead and writes markdown back out of it.
 *
 * Three shapes are not what they look like and are handled by name:
 *
 * - **KaTeX renders the formula twice** — once as MathML for screen readers
 *   and once as positioned spans for eyes — so a naive walk produces the
 *   formula twice over, each unreadable. The TeX itself is in the MathML's
 *   `<annotation>`, which is the one copy worth having: a `.katex` subtree is
 *   read for that and never descended into.
 * - **A mermaid diagram is an SVG**, and its labels in document order are not
 *   a diagram. The figure carries its own fence source in `data-md`
 *   (`app/src/components/markdown/Mermaid.tsx`), which is what comes out.
 * - **The furniture is marked, not guessed.** Anything with `data-copy-skip`
 *   — a message's action row, a "Show more" toggle — is chrome that happens
 *   to be selectable, and it is left out.
 *
 * Everything else is ordinary: block tags produce blocks, inline tags wrap
 * their children, and a text node contributes only the part of itself that is
 * actually inside the selection.
 */

/** Tags that start a new block. Everything else is inline, including the
 *  spans and chips a reply is full of. */
const BLOCK = new Set([
  "ADDRESS", "ARTICLE", "ASIDE", "BLOCKQUOTE", "DIV", "DL", "DD", "DT",
  "FIELDSET", "FIGCAPTION", "FIGURE", "FOOTER", "FORM", "H1", "H2", "H3",
  "H4", "H5", "H6", "HEADER", "HR", "LI", "MAIN", "NAV", "OL", "P", "PRE",
  "SECTION", "TABLE", "UL",
]);

/** Never walked into: chrome, and the two element kinds whose text is not
 *  text (a diagram's labels, a stylesheet). */
function skipped(el: Element): boolean {
  return (
    el.hasAttribute("data-copy-skip") ||
    el.tagName === "SCRIPT" ||
    el.tagName === "STYLE" ||
    // `tagName` on an SVG element keeps its source case, hence the test on
    // the local name.
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

/** The part of a text node the selection actually covers. */
function clip(node: Text, range: Range): string {
  const start = node === range.startContainer ? range.startOffset : 0;
  const end = node === range.endContainer ? range.endOffset : node.data.length;
  return node.data.slice(start, end);
}

/**
 * …collapsed the way the browser draws it, unless the box it sits in says
 * otherwise. A question bubble is `whitespace-pre-wrap`, and its line breaks
 * are the message's own.
 */
function clipText(node: Text, range: Range): string {
  const raw = clip(node, range);
  const ws = node.parentElement ? getComputedStyle(node.parentElement).whiteSpace : "normal";
  return ws.startsWith("pre") ? raw : raw.replace(/[\t\n ]+/g, " ");
}

/** The TeX behind a rendered formula, or null for anything else. */
function tex(el: Element): string | null {
  const annotation = el.querySelector('annotation[encoding="application/x-tex"]');
  const source = annotation?.textContent?.trim();
  return source ? source : null;
}

function isKatex(el: Element): boolean {
  return el.classList.contains("katex") || el.classList.contains("katex-display");
}

/** `**bold**`, with any spaces the run ends in left outside the markers —
 *  `** bold **` is not bold. */
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
  // A mention, in the composer's box and in a bubble alike: the message says
  // the path, the chip only draws it (`FileChip.tsx`).
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

/** Trim a run of inline text into the paragraph it will be. */
function tidy(text: string): string {
  return text.replace(/[ \t]+\n/g, "\n").replace(/\n[ \t]+/g, "\n").trim();
}

/**
 * A container's contents as blocks. Inline runs between block children
 * accumulate into paragraphs of their own, which is what makes a selection
 * that starts mid-sentence come out as prose rather than as a fragment glued
 * to the heading after it.
 */
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
          // A nested list is part of its item, not a paragraph after it: a
          // blank line before it makes the whole list loose, and every item
          // of it grows a paragraph's spacing wherever it is rendered next.
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
  // The first row selected stands as the header: a table whose head was
  // scrolled past still has to be a table on the other end of the paste.
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
      // One quote, however many paragraphs: separate `>` blocks would paste
      // back as separate quotes.
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

/**
 * The current selection as markdown, or an empty string when there is nothing
 * worth replacing the browser's own copy with.
 */
export function selectionMarkdown(selection: Selection | null): string {
  if (!selection || selection.rangeCount === 0 || selection.isCollapsed) return "";
  const range = selection.getRangeAt(0);
  const root = range.commonAncestorContainer;
  if (root.nodeType === Node.TEXT_NODE) return tidy(clipText(root as Text, range));
  if (root.nodeType !== Node.ELEMENT_NODE) return "";
  return container(root as Element, range).join("\n\n").trim();
}
