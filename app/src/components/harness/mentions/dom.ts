import { splitLibraryPaths } from "@/lib/files/openFile";

/** Runs of text with atomic mentions (a mention is only its path). The DOM, not
 *  this model, is the truth while typing: React renders it once per structural
 *  change, because a contenteditable re-rendered per keystroke drops the caret. */
export type Chunk = { kind: "text"; text: string } | { kind: "chip"; path: string };

/** A mention as the message spells it: the backticked path `oculus read` takes. */
export function chipText(path: string): string {
  return `\`${path}\``;
}

/** Caret landing strip: WebKit cannot place a caret before a
 *  `contenteditable="false"` that starts a block. Stripped from every read. */
const ZWSP = "​";
export const ZWSP_RE = /​/g;

/** Block tags a paste or undo may split the box into; each means a line break. */
const BLOCKS = new Set(["DIV", "P", "LI"]);

export type Point = { node: Node; offset: number };

export type Reading = { text: string; caret: number | null; chunks: Chunk[] };

export function isChip(node: Node): boolean {
  return node.nodeType === Node.ELEMENT_NODE && (node as HTMLElement).hasAttribute("data-path");
}

/** Walks the nodes into the sent text, its chunks and the caret's offset in one
 *  pass, because the `@` token is found by offset and spliced by chunk. Drops
 *  WebKit's no-break space (its stand-in for a trailing typed space) and ZWSP. */
export function readEditor(root: HTMLElement, point: Point | null): Reading {
  const chunks: Chunk[] = [];
  let text = "";
  let caret: number | null = null;

  const push = (s: string) => {
    if (!s) return;
    const last = chunks[chunks.length - 1];
    if (last?.kind === "text") last.text += s;
    else chunks.push({ kind: "text", text: s });
    text += s;
  };
  const clean = (s: string) => s.replace(/ /g, " ").replace(ZWSP_RE, "");

  const visit = (node: Node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      const raw = (node as Text).data;
      if (point?.node === node) {
        const at = Math.min(point.offset, raw.length);
        push(clean(raw.slice(0, at)));
        caret = text.length;
        push(clean(raw.slice(at)));
      } else {
        push(clean(raw));
      }
      return;
    }
    if (node.nodeType !== Node.ELEMENT_NODE) return;
    const el = node as HTMLElement;
    if (isChip(el)) {
      const path = el.getAttribute("data-path") ?? "";
      chunks.push({ kind: "chip", path });
      text += chipText(path);
      return;
    }
    if (el.tagName === "BR") {
      push("\n");
      return;
    }
    if (BLOCKS.has(el.tagName) && text && !text.endsWith("\n")) push("\n");
    walk(el);
  };

  const walk = (el: Node) => {
    const kids = el.childNodes;
    for (let i = 0; i < kids.length; i++) {
      if (point?.node === el && point.offset === i) caret = text.length;
      visit(kids[i]);
    }
    if (point?.node === el && point.offset === kids.length) caret = text.length;
  };

  walk(root);
  return { text, caret, chunks };
}

export function chunkText(c: Chunk): string {
  return c.kind === "chip" ? chipText(c.path) : c.text.replace(ZWSP_RE, "");
}

export function chunksText(chunks: Chunk[]): string {
  return chunks.map(chunkText).join("");
}

/** No empty or adjacent text runs (the caret maths addresses one DOM node per
 *  chunk by index), plus a ZWSP guard before a leading chip. */
export function normalize(chunks: Chunk[]): Chunk[] {
  const out: Chunk[] = [];
  for (const c of chunks) {
    if (c.kind === "chip") {
      out.push(c);
      continue;
    }
    const text = c.text.replace(ZWSP_RE, "");
    if (!text) continue;
    const last = out[out.length - 1];
    if (last?.kind === "text") out[out.length - 1] = { kind: "text", text: last.text + text };
    else out.push({ kind: "text", text });
  }
  if (out[0]?.kind === "chip") out.unshift({ kind: "text", text: ZWSP });
  return out;
}

/** DOM offset of the nth message character in a chunk, skipping ZWSP guards. */
export function domOffset(text: string, n: number): number {
  let seen = 0;
  for (let i = 0; i < text.length; i++) {
    if (seen === n && text[i] !== ZWSP) return i;
    if (text[i] !== ZWSP) seen++;
  }
  return text.length;
}

/** A message offset as (chunk, offset) — chunks, since the old nodes are gone by
 *  the time the caret lands and `childNodes[chunk]` is what that chunk became.
 *  An offset inside a chip resolves to just after it. */
export function caretChunk(chunks: Chunk[], offset: number): { chunk: number; offset: number } {
  let at = 0;
  for (let i = 0; i < chunks.length; i++) {
    const c = chunks[i];
    const len = chunkText(c).length;
    if (c.kind === "chip") {
      if (offset <= at + len) return { chunk: i + 1, offset: 0 };
    } else if (offset <= at + len) {
      return { chunk: i, offset: domOffset(c.text, offset - at) };
    }
    at += len;
  }
  return { chunk: chunks.length, offset: 0 };
}

/** Text back into chunks via `splitLibraryPaths`, so a restored draft chips the
 *  same runs a sent message does. */
export function toChunks(text: string): Chunk[] {
  return splitLibraryPaths(text).map((p) => {
    if (p.kind === "path") return { kind: "chip" as const, path: p.path };
    // A picture is not a mention the box can redraw; it stays text.
    if (p.kind === "image") return { kind: "text" as const, text: `\`${p.raw}\`` };
    return { kind: "text" as const, text: p.text };
  });
}

/** True when nothing but WebKit's placeholder `<br>` follows the caret — the one
 *  place it can't be measured (see `revealCaret`). */
function atEnd(el: HTMLElement, caret: Range): boolean {
  const tail = document.createRange();
  tail.selectNodeContents(el);
  tail.setStart(caret.startContainer, caret.startOffset);
  if (tail.toString().replace(ZWSP_RE, "").replace(/\n/g, "").trim()) return false;
  const rest = tail.cloneContents();
  if (rest.querySelector("[data-path]")) return false;
  // One break is the placeholder; more are blank lines below the caret.
  return rest.querySelectorAll("br").length + (tail.toString().match(/\n/g)?.length ?? 0) <= 1;
}

/** The rect of a node's first or last line (wrapped text has one per line). */
function edgeRect(node: Node, side: "start" | "end"): DOMRect | null {
  let rects: DOMRectList;
  if (node.nodeType === Node.ELEMENT_NODE) {
    rects = (node as HTMLElement).getClientRects();
  } else {
    const range = document.createRange();
    range.selectNodeContents(node);
    rects = range.getClientRects();
  }
  const rect = side === "end" ? rects[rects.length - 1] : rects[0];
  return rect && rect.height ? rect : null;
}

/** The caret's line. WebKit gives no rects for a caret between nodes (where a
 *  line break leaves it), so the neighbours are measured — never the box itself,
 *  which is the viewport the caret is compared against. */
function caretRect(caret: Range): DOMRect | null {
  const own = caret.getClientRects()[0];
  if (own?.height) return own;
  const node = caret.startContainer;
  if (node.nodeType !== Node.ELEMENT_NODE) return null;
  const before = node.childNodes[caret.startOffset - 1];
  const after = node.childNodes[caret.startOffset];
  return (before && edgeRect(before, "end")) || (after && edgeRect(after, "start")) || null;
}

/** Scroll the box so the caret is in view: WebKit doesn't follow an `execCommand`
 *  break past `max-h`, and a hand-placed range moves no scroll. At the end, the
 *  placeholder `<br>` measures inconsistently, so jump to the bottom instead.
 *  Not `scrollIntoView` — that would scroll the thread behind the composer too. */
export function revealCaret(el: HTMLElement) {
  // A box written to while unfocused (a `clear()`) keeps its scroll.
  if (!el.contains(document.activeElement)) return;
  const sel = window.getSelection();
  if (!sel || sel.rangeCount === 0 || !sel.focusNode || !el.contains(sel.focusNode)) return;
  const caret = sel.getRangeAt(0).cloneRange();
  caret.collapse(false);
  if (atEnd(el, caret)) {
    el.scrollTop = el.scrollHeight;
    return;
  }
  const rect = caretRect(caret);
  if (!rect) return;
  const view = el.getBoundingClientRect();
  if (rect.bottom > view.bottom) el.scrollTop += rect.bottom - view.bottom;
  else if (rect.top < view.top) el.scrollTop -= view.top - rect.top;
}

/** Sets the selection again once the frame has settled. Two line breaks in
 *  quick succession can leave the first caret painted beside the second;
 *  WebKit repaints the caret when the selection is set. */
export function repaintCaret(el: HTMLElement): number {
  return requestAnimationFrame(() => {
    const sel = window.getSelection();
    if (!sel || sel.rangeCount === 0 || !sel.focusNode || !el.contains(sel.focusNode)) return;
    const range = sel.getRangeAt(0).cloneRange();
    sel.removeAllRanges();
    sel.addRange(range);
  });
}
