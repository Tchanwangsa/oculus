import { splitLibraryPaths } from "@/lib/files/libraryPaths";
import { parseLink } from "@/lib/harness/links";
import { skillText, splitSkillTokens } from "@/lib/harness/skills";
import { chipText, linkChunks, ZWSP_RE, type Chunk } from "./chunks";

/** Block tags a paste or undo may split the box into; each means a line break. */
const BLOCKS = new Set(["DIV", "P", "LI"]);

export type Point = { node: Node; offset: number };

/** `stale`: a drawn link no longer reads as one, so the nodes need redrawing. */
export type Reading = { text: string; caret: number | null; chunks: Chunk[]; stale: boolean };

/** A mention, an attachment or a picked skill, the atomic references: the
 *  caret steps over each whole and a delete key takes it whole. */
export function isChip(node: Node): boolean {
  if (node.nodeType !== Node.ELEMENT_NODE) return false;
  const el = node as HTMLElement;
  return el.hasAttribute("data-path") || el.hasAttribute("data-attachment") || el.hasAttribute("data-skill");
}

/** A link (`ComposerLink`): editable text drawn as a link. */
function isLink(node: Node): boolean {
  return node.nodeType === Node.ELEMENT_NODE && (node as HTMLElement).hasAttribute("data-link");
}

/** A markdown link's words back into `[words](href)`; a bare link is its text. */
function linkRaw(text: string, href: string | null): string {
  if (href == null) return text;
  return text ? `[${text}](${href})` : "";
}

/** Walks the nodes into the sent text, its chunks and the caret's offset in one
 *  pass, because the `@` token is found by offset and spliced by chunk. Drops
 *  WebKit's no-break space (its stand-in for a trailing typed space) and ZWSP. */
export function readEditor(root: HTMLElement, point: Point | null): Reading {
  const chunks: Chunk[] = [];
  let text = "";
  let caret: number | null = null;
  let stale = false;

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
    if (isLink(el)) {
      visitLink(el);
      return;
    }
    if (el.hasAttribute("data-skill")) {
      const name = el.getAttribute("data-skill") ?? "";
      chunks.push({ kind: "skill", name });
      text += skillText(name);
      return;
    }
    if (el.hasAttribute("data-attachment")) {
      const path = el.getAttribute("data-attachment") ?? "";
      chunks.push({ kind: "attachment", path });
      text += chipText(path);
      return;
    }
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

  // The text as it stands, so an edit inside the link is read; one that leaves
  // no link reads as prose and marks the reading `stale`.
  const visitLink = (el: HTMLElement) => {
    let words = "";
    let inside: number | null = null;
    const nodes = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    for (let t = nodes.nextNode() as Text | null; t; t = nodes.nextNode() as Text | null) {
      if (point?.node === t) inside = words.length + clean(t.data.slice(0, point.offset)).length;
      words += clean(t.data);
    }
    if (point?.node === el) inside = point.offset === 0 ? 0 : words.length;
    const href = el.getAttribute("data-href");
    const raw = linkRaw(words, href);
    const at = text.length;
    if (parseLink(raw)) {
      chunks.push({ kind: "link", raw });
      text += raw;
    } else {
      push(raw);
      stale = true;
    }
    if (inside != null) caret = at + (href != null && raw ? inside + 1 : inside);
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
  return { text, caret, chunks, stale };
}


/** Text back into chunks via `splitLibraryPaths`, so a restored draft chips the
 *  same runs a sent message does, and its links draw as links. An attachment
 *  keeps the message's spelling of its path. A `/name` word becomes a skill
 *  chip only when `skills` (the installed catalogue) holds it. */
export function toChunks(text: string, skills?: ReadonlySet<string>): Chunk[] {
  const paths = splitLibraryPaths(text).flatMap((p): Chunk[] => {
    if (p.kind === "path") return [{ kind: "chip", path: p.path }];
    if (p.kind === "image" || p.kind === "attachment") return [{ kind: "attachment", path: p.raw }];
    if (!skills?.size) return [{ kind: "text", text: p.text }];
    return splitSkillTokens(p.text, skills);
  });
  return linkChunks(paths);
}

/** True when nothing but WebKit's placeholder `<br>` follows the caret — the one
 *  place it can't be measured (see `revealCaret`). */
function atEnd(el: HTMLElement, caret: Range): boolean {
  const tail = document.createRange();
  tail.selectNodeContents(el);
  tail.setStart(caret.startContainer, caret.startOffset);
  if (tail.toString().replace(ZWSP_RE, "").replace(/\n/g, "").trim()) return false;
  const rest = tail.cloneContents();
  if (rest.querySelector("[data-path], [data-attachment], [data-skill], [data-link]")) return false;
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
