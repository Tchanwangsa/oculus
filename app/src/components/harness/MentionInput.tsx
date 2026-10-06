import { useImperativeHandle, useLayoutEffect, useRef, useState, type KeyboardEvent, type Ref } from "react";
import { FileChip } from "@/components/markdown/FileChip";
import { splitLibraryPaths } from "@/lib/openFile";
import { imageFiles } from "@/lib/attachments";

/** Runs of text with atomic mentions (a mention is only its path). The DOM, not
 *  this model, is the truth while typing: React renders it once per structural
 *  change, because a contenteditable re-rendered per keystroke drops the caret. */
type Chunk = { kind: "text"; text: string } | { kind: "chip"; path: string };

/** A mention as the message spells it: the backticked path `oculus read` takes. */
function chipText(path: string): string {
  return `\`${path}\``;
}

/** Caret landing strip: WebKit cannot place a caret before a
 *  `contenteditable="false"` that starts a block. Stripped from every read. */
const ZWSP = "​";
const ZWSP_RE = /​/g;

/** Block tags a paste or undo may split the box into; each means a line break. */
const BLOCKS = new Set(["DIV", "P", "LI"]);

type Point = { node: Node; offset: number };

type Reading = { text: string; caret: number | null; chunks: Chunk[] };

function isChip(node: Node): boolean {
  return node.nodeType === Node.ELEMENT_NODE && (node as HTMLElement).hasAttribute("data-path");
}

/** Walks the nodes into the sent text, its chunks and the caret's offset in one
 *  pass, because the `@` token is found by offset and spliced by chunk. Drops
 *  WebKit's no-break space (its stand-in for a trailing typed space) and ZWSP. */
function readEditor(root: HTMLElement, point: Point | null): Reading {
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

function chunkText(c: Chunk): string {
  return c.kind === "chip" ? chipText(c.path) : c.text.replace(ZWSP_RE, "");
}

function chunksText(chunks: Chunk[]): string {
  return chunks.map(chunkText).join("");
}

/** No empty or adjacent text runs (the caret maths addresses one DOM node per
 *  chunk by index), plus a ZWSP guard before a leading chip. */
function normalize(chunks: Chunk[]): Chunk[] {
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
function domOffset(text: string, n: number): number {
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
function caretChunk(chunks: Chunk[], offset: number): { chunk: number; offset: number } {
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
function toChunks(text: string): Chunk[] {
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
function revealCaret(el: HTMLElement) {
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

type Landing = { chunk: number; offset: number; focus: boolean };

/** The structural edits. Typing, caret, selection and undo stay the browser's;
 *  content comes back through `onEdit`. */
export interface MentionInputHandle {
  /** Swap the `@…` token starting at message offset `start` for a chip plus a space. */
  insertMention(start: number, path: string): void;
  clear(): void;
  /** Put handed-back text in front of what is typed, mentions restored to chips. */
  prepend(text: string): void;
  /** Put text after what is typed, a blank line between, as plain text. */
  append(text: string): void;
}

/**
 * A contenteditable whose `@` mentions are inline `contenteditable="false"`
 * chips. Drawn and sent text differ: `readEditor` turns each chip back into its
 * backticked library path, so the agent never sees a display name. Only
 * Shift+Enter and chip-eating Backspace/Delete are intercepted.
 */
export function MentionInput({
  ref,
  placeholder,
  initialText,
  autoFocus,
  onEdit,
  onFiles,
  onPasteText,
  onKeyDown,
  onBlur,
}: {
  ref?: Ref<MentionInputHandle>;
  placeholder: string;
  /** Read once at mount; after that the DOM is the truth. */
  initialText?: string;
  autoFocus?: boolean;
  onEdit: (text: string, caret: number) => void;
  /** Pasted pictures; they never enter the editor's nodes. */
  onFiles?: (files: File[]) => void;
  /** Offered a plain-text paste first; true means it was taken (as a card). */
  onPasteText?: (text: string) => boolean;
  /** Runs first; anything not `preventDefault`ed falls through to the editing keys. */
  onKeyDown?: (e: KeyboardEvent<HTMLDivElement>) => void;
  onBlur?: () => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [chunks, setChunks] = useState<Chunk[]>(() => normalize(toChunks(initialText ?? "")));
  /** The editor's `key`: a structural change remounts instead of diffing a tree
   *  the typist edited underneath React. */
  const [version, setVersion] = useState(0);
  /** Placeholder flag — `:empty` can't do it, WebKit's trailing `<br>` isn't empty. */
  const [blank, setBlank] = useState(!(initialText ?? "").trim());
  const landing = useRef<Landing | null>(null);

  function point(el: HTMLElement): Point | null {
    const sel = window.getSelection();
    if (!sel || sel.rangeCount === 0 || !sel.focusNode) return null;
    if (!el.contains(sel.focusNode)) return null;
    return { node: sel.focusNode, offset: sel.focusOffset };
  }

  /** Also on arrow keys and clicks: a caret landing beside an `@` opens the menu. */
  function sync() {
    const el = box.current;
    if (!el) return;
    const r = readEditor(el, point(el));
    setBlank(!r.text.trim());
    onEdit(r.text, r.caret ?? r.text.length);
  }

  function commit(next: Chunk[], caret: number, focus?: boolean) {
    const el = box.current;
    const norm = normalize(next);
    landing.current = {
      ...caretChunk(norm, caret),
      focus: focus === true || (el != null && el.contains(document.activeElement)),
    };
    setChunks(norm);
    setVersion((v) => v + 1);
    const text = chunksText(norm);
    setBlank(!text.trim());
    onEdit(text, caret);
  }

  // Place the caret once the remounted nodes exist, before paint.
  useLayoutEffect(() => {
    const el = box.current;
    const target = landing.current;
    if (!el || !target) return;
    landing.current = null;
    if (target.focus) el.focus();
    const sel = window.getSelection();
    if (!sel) return;
    const range = document.createRange();
    const node = el.childNodes[target.chunk];
    if (node && node.nodeType === Node.TEXT_NODE) {
      range.setStart(node, Math.min(target.offset, (node as Text).data.length));
    } else if (node) {
      range.setStartBefore(node);
    } else {
      range.selectNodeContents(el);
      range.collapse(false);
    }
    range.collapse(true);
    sel.removeAllRanges();
    sel.addRange(range);
    revealCaret(el);
  }, [version]);

  useImperativeHandle(ref, () => ({
    insertMention(start, path) {
      const el = box.current;
      if (!el) return;
      const r = readEditor(el, point(el));
      const caret = r.caret ?? r.text.length;
      // A query never holds a backtick or newline, so the token is in one text run.
      let at = 0;
      for (let i = 0; i < r.chunks.length; i++) {
        const c = r.chunks[i];
        const len = chunkText(c).length;
        if (c.kind === "text" && start >= at && caret <= at + len) {
          const head = c.text.slice(0, domOffset(c.text, start - at));
          const tail = c.text.slice(domOffset(c.text, caret - at));
          commit(
            [
              ...r.chunks.slice(0, i),
              { kind: "text", text: head },
              { kind: "chip", path },
              { kind: "text", text: ` ${tail}` },
              ...r.chunks.slice(i + 1),
            ],
            start + chipText(path).length + 1,
            true,
          );
          return;
        }
        at += len;
      }
    },
    clear() {
      commit([], 0);
    },
    prepend(text) {
      const el = box.current;
      if (!el) return;
      const kept = readEditor(el, null).chunks;
      const back = normalize(toChunks(text));
      commit(
        [...back, ...(kept.length ? [{ kind: "text" as const, text: "\n\n" }, ...kept] : [])],
        chunksText(back).length,
        true,
      );
    },
    append(text) {
      const el = box.current;
      if (!el) return;
      const kept = normalize(readEditor(el, null).chunks);
      const last = kept[kept.length - 1];
      if (last?.kind === "text") kept[kept.length - 1] = { kind: "text", text: last.text.trimEnd() };
      const head = chunksText(kept).trim() ? [...kept, { kind: "text" as const, text: "\n\n" }] : [];
      const next = normalize([...head, { kind: "text", text }]);
      commit(next, chunksText(next).length, true);
    },
  }));

  // Mount-time focus only (later focus is `commit`'s). WebKit focuses a
  // contenteditable with the caret at its start, so move it to the end.
  useLayoutEffect(() => {
    const el = box.current;
    if (!autoFocus || !el) return;
    el.focus();
    const sel = window.getSelection();
    if (!sel || el.childNodes.length === 0) return;
    const range = document.createRange();
    range.selectNodeContents(el);
    range.collapse(false);
    sel.removeAllRanges();
    sel.addRange(range);
  }, [autoFocus]);

  /** The chip a delete key would land on — only at the very edge of a text run. */
  function chipBeside(el: HTMLElement, dir: "back" | "forward"): HTMLElement | null {
    const sel = window.getSelection();
    if (!sel || !sel.isCollapsed) return null;
    const p = point(el);
    if (!p) return null;
    let side: Node | null = null;
    if (p.node.nodeType === Node.TEXT_NODE) {
      const raw = (p.node as Text).data;
      // The zero-width guard is not a character in the way.
      const rest = dir === "back" ? raw.slice(0, p.offset) : raw.slice(p.offset);
      if (rest.replace(ZWSP_RE, "")) return null;
      side = dir === "back" ? p.node.previousSibling : p.node.nextSibling;
    } else {
      side = p.node.childNodes[dir === "back" ? p.offset - 1 : p.offset] ?? null;
    }
    return side && isChip(side) ? (side as HTMLElement) : null;
  }

  /** Delete a whole chip, through the browser so it joins the undo stack. */
  function removeChip(chip: HTMLElement) {
    const sel = window.getSelection();
    const range = document.createRange();
    range.selectNode(chip);
    sel?.removeAllRanges();
    sel?.addRange(range);
    if (!document.execCommand("delete")) chip.remove();
  }

  function keyDown(e: KeyboardEvent<HTMLDivElement>) {
    onKeyDown?.(e);
    if (e.defaultPrevented) return;
    const el = box.current;
    if (!el) return;
    if (e.key === "Enter" && e.shiftKey) {
      // Through the browser, so the break joins undo and WebKit places its own
      // trailing placeholder (a hand-made `<br>` leaves the caret a line up).
      e.preventDefault();
      if (!document.execCommand("insertLineBreak")) {
        document.execCommand("insertText", false, "\n");
      }
      revealCaret(el);
      sync();
      return;
    }
    if (e.key === "Backspace" || e.key === "Delete") {
      const chip = chipBeside(el, e.key === "Backspace" ? "back" : "forward");
      if (!chip) return;
      e.preventDefault();
      removeChip(chip);
      revealCaret(el);
      sync();
    }
  }

  return (
    <div className="relative">
      <div
        key={version}
        ref={box}
        contentEditable
        suppressContentEditableWarning
        role="textbox"
        aria-multiline="true"
        aria-label="Message"
        aria-placeholder={placeholder}
        // `select-text`: `index.css` turns selection off app-wide, and without it
        // WebKit can barely edit the box. `overflow-x-hidden`:
        // y=auto computes x to auto, and always-on scrollbars paint a bar across a one-line box.
        className="max-h-[160px] min-h-[16px] w-full overflow-x-hidden overflow-y-auto break-words whitespace-pre-wrap select-text text-[13px] leading-[16px] outline-none"
        onInput={sync}
        onKeyUp={sync}
        onClick={sync}
        onKeyDown={keyDown}
        onBlur={onBlur}
        onPaste={(e) => {
          // Plain text only (rich fragments bring their own elements); a
          // clipboard image wins over any text flavour it carries.
          e.preventDefault();
          const pictures = onFiles ? imageFiles(e.clipboardData.files) : [];
          if (pictures.length) {
            onFiles?.(pictures);
            return;
          }
          const plain = e.clipboardData.getData("text/plain");
          if (plain && onPasteText?.(plain)) return;
          if (plain) document.execCommand("insertText", false, plain);
          if (box.current) revealCaret(box.current);
          sync();
        }}
      >
        {chunks.map((c, i) => (c.kind === "text" ? c.text : <FileChip key={i} path={c.path} />))}
      </div>
      {blank && (
        <div className="pointer-events-none absolute inset-0 select-none text-[13px] leading-[16px] text-muted-foreground">
          {placeholder}
        </div>
      )}
    </div>
  );
}
