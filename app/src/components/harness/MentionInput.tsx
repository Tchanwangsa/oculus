import { useImperativeHandle, useLayoutEffect, useRef, useState, type KeyboardEvent, type Ref } from "react";
import { FileChip } from "@/components/markdown/FileChip";
import { splitLibraryPaths } from "@/lib/openFile";
import { imageFiles } from "@/lib/attachments";
import { cn } from "@/lib/utils";

/**
 * The box's content, as this component models it: runs of plain text with
 * atomic mentions between them.
 *
 * A mention is only ever its path. Everything the chip draws — the glyph, the
 * display name — the chip derives from that path itself, so the box never
 * holds a second copy of anything the message does not carry.
 *
 * The model is deliberately *not* the source of truth while someone types —
 * the DOM is. React renders it once per structural change (a pick, a restore,
 * a send) and then keeps its hands off, and every read walks the live nodes.
 * A contenteditable that re-renders on each keystroke is a caret that jumps.
 */
type Chunk = { kind: "text"; text: string } | { kind: "chip"; path: string };

/**
 * A mention as the *message* spells it: the library path in backticks, which
 * is what `oculus read` takes. The chip is only how the box draws it, and the
 * two must never diverge — the agent has no idea what a display name is.
 */
function chipText(path: string): string {
  return `\`${path}\``;
}

/**
 * Zero-width space: a landing strip for the caret in front of a chip that
 * would otherwise be the first thing in the box.
 *
 * WebKit has nowhere to put a collapsed selection *before* a
 * `contenteditable="false"` element that starts a block, so without this you
 * cannot type in front of a mention you just picked. It is stripped out of
 * every read, so it never reaches the message.
 */
const ZWSP = "​";
const ZWSP_RE = /​/g;

/** Elements a browser may leave behind that mean "line break here". A
 *  `<br>` is what `insertLineBreak` gives us; the block tags are insurance
 *  against a paste or an undo that splits the box into divs. */
const BLOCKS = new Set(["DIV", "P", "LI"]);

/** A caret, DOM-side: the node it sits in and how far into it. */
type Point = { node: Node; offset: number };

/** The box read out: what it would send, where the caret is in that string,
 *  and the chunks it is currently made of. */
type Reading = { text: string; caret: number | null; chunks: Chunk[] };

/** True for one of our mention spans. */
function isChip(node: Node): boolean {
  return node.nodeType === Node.ELEMENT_NODE && (node as HTMLElement).hasAttribute("data-path");
}

/**
 * Walks the editor's nodes into the string that gets sent, the chunks it is
 * made of, and — if a caret is handed in — that caret's offset into the
 * string.
 *
 * One walk does all three because they have to agree: the `@` token is found
 * by offset in the serialized text and then spliced by chunk, so a
 * disagreement of one character between the two would edit the wrong place.
 *
 * Two characters are dropped on the way out. A no-break space is WebKit's,
 * substituted for a trailing typed space so it does not collapse, and the
 * agent should see the space that was meant. The zero-width space is ours
 * (see `ZWSP`).
 */
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

/** What a chunk contributes to the sent message. */
function chunkText(c: Chunk): string {
  return c.kind === "chip" ? chipText(c.path) : c.text.replace(ZWSP_RE, "");
}

/** The message a set of chunks would send. */
function chunksText(chunks: Chunk[]): string {
  return chunks.map(chunkText).join("");
}

/**
 * Tidies a spliced set of chunks into the shape the renderer relies on: no
 * empty runs, no two text runs in a row (React renders one text node per
 * chunk, and the caret maths below addresses them by index), and a
 * zero-width guard in front of a leading chip.
 */
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

/** Where in a chunk's DOM text the nth character *of the message* lives. The
 *  two differ only by the zero-width guards, which the message never sees. */
function domOffset(text: string, n: number): number {
  let seen = 0;
  for (let i = 0; i < text.length; i++) {
    if (seen === n && text[i] !== ZWSP) return i;
    if (text[i] !== ZWSP) seen++;
  }
  return text.length;
}

/**
 * An offset in the sent message, as a chunk and an offset inside it.
 *
 * Chunks are addressed rather than nodes because the caret is placed *after*
 * a re-render, when the old nodes are gone: React renders exactly one node
 * per chunk, in order, so `childNodes[chunk]` is the one that chunk became. A
 * mention is atomic, so an offset that lands inside one resolves to the
 * position just after it.
 */
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

/**
 * Text back into chunks, with every backticked library path becoming the chip
 * it was typed as — through the same `splitLibraryPaths` the bubble and the
 * agent's own prose read a message with, so a restored draft and a sent
 * message chip exactly the same runs.
 *
 * A backticked anything-else is prose that happens to be fenced and stays the
 * text it was.
 */
function toChunks(text: string): Chunk[] {
  return splitLibraryPaths(text).map((p) => {
    if (p.kind === "path") return { kind: "chip" as const, path: p.path };
    // An attached picture comes back as the text it was sent as — the box
    // holds messages, and a picture is not a mention it can redraw.
    if (p.kind === "image") return { kind: "text" as const, text: `\`${p.raw}\`` };
    return { kind: "text" as const, text: p.text };
  });
}

/**
 * True when the caret is at the end of the box: nothing after it but the
 * placeholder `<br>` WebKit keeps there so the last line can be stood on.
 *
 * It is worth asking separately because the end is the one place the caret
 * cannot be *measured* — see `revealCaret` — and the one place the answer is
 * free: the bottom of the scroll.
 */
function atEnd(el: HTMLElement, caret: Range): boolean {
  const tail = document.createRange();
  tail.selectNodeContents(el);
  tail.setStart(caret.startContainer, caret.startOffset);
  if (tail.toString().replace(ZWSP_RE, "").replace(/\n/g, "").trim()) return false;
  const rest = tail.cloneContents();
  if (rest.querySelector("[data-path]")) return false;
  // One break is the placeholder; more than one is blank lines the caret is
  // sitting above, and the bottom of the box is no longer where it is.
  return rest.querySelectorAll("br").length + (tail.toString().match(/\n/g)?.length ?? 0) <= 1;
}

/**
 * A node's edge line, as a rectangle: the line its start sits on, or the line
 * its end sits on. A run of text that wraps has one rect per line, and only
 * the last of them is the line a caret sitting behind it is on.
 */
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

/**
 * The caret's own line, in viewport coordinates.
 *
 * A collapsed range usually measures itself, but not always: WebKit hands back
 * *no* rects at all for a caret sitting between nodes rather than inside text
 * — which is where a line break leaves it. The nodes on either side of that
 * gap are on the caret's line, so they are measured instead.
 *
 * What must never be measured is the box itself: its rect is the viewport the
 * caret is compared *against*, so a caret "found" there always looks
 * comfortably in view and nothing ever scrolls.
 */
function caretRect(caret: Range): DOMRect | null {
  const own = caret.getClientRects()[0];
  if (own?.height) return own;
  const node = caret.startContainer;
  if (node.nodeType !== Node.ELEMENT_NODE) return null;
  const before = node.childNodes[caret.startOffset - 1];
  const after = node.childNodes[caret.startOffset];
  return (before && edgeRect(before, "end")) || (after && edgeRect(after, "start")) || null;
}

/**
 * Scroll the box so the caret is inside it.
 *
 * A `<textarea>` did this for free; a contenteditable does it only for the
 * edits the browser thinks it made. A break inserted by `execCommand` on a box
 * that has reached its `max-h` leaves the new line under the bottom edge —
 * typing carries on out of sight — and a caret placed by hand after a
 * structural re-render (`commit`) moves no scroll at all, because setting a
 * range is not an edit. Both end here.
 *
 * The end of the box is answered by `atEnd` rather than by measuring, because
 * the thing that would be measured there is WebKit's trailing placeholder
 * `<br>`, whose rect sits on the caret's line sometimes and on the line below
 * it the rest of the time — a reveal built on it lands a line short on every
 * other press. The bottom of the scroll is the same answer and is always
 * right.
 *
 * `scrollIntoView` is not what does any of it: that walks *every* scrollable
 * ancestor, so a break in the composer would drag the thread behind it too.
 */
function revealCaret(el: HTMLElement) {
  // Only a box being typed in has a caret to follow. A box that is merely
  // being written *to* — a `clear()` on a composer nobody is in — keeps the
  // scroll it had rather than jumping somewhere nobody is looking.
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

/** The caret that is placed once the DOM catches up with a structural change. */
type Landing = { chunk: number; offset: number; focus: boolean };

/**
 * What the composer can ask of the box: the three things that change its
 * content structurally. Everything else — typing, the caret, the selection,
 * undo — is the browser's and deliberately untouched, and what the box holds
 * comes back through `onEdit` rather than being asked for.
 */
export interface MentionInputHandle {
  /** Swap the `@…` token starting at `start` (an offset into the message) for
   *  a chip for this library path, followed by the space the old textarea
   *  left. */
  insertMention(start: number, path: string): void;
  clear(): void;
  /** Put handed-back text in front of whatever is typed, with its mentions
   *  restored to chips. */
  prepend(text: string): void;
  /** Drop a run of text in at the caret.
   *
   *  For a picture written on paste, where the markdown has to land *in* the
   *  text being written and the write is a round trip to Rust, so there is no
   *  keystroke to ride in on (`TaskBody` in `app/src/pages/TaskPage.tsx`).
   *  Structural like the other three — it goes through the same splice and the
   *  same caret landing — rather than a second way of editing the box. */
  insertText(text: string): void;
}

/**
 * The composer's text box: a contenteditable whose `@` mentions are chips.
 *
 * A `<textarea>` cannot draw an icon inside its text, and the mention wants
 * one — a bare library path spent a line and a half saying `courses/…/13.pdf`
 * when what the student picked was "Week 3 Lecture". So the box became
 * editable markup, and each mention an inline `contenteditable="false"` span
 * carrying the file's own glyph and the label the side panel and the `@` menu
 * already use.
 *
 * **What is drawn and what is sent are different strings**, and that is the
 * whole design: `readEditor` walks the nodes back into the message, where a
 * chip is its backticked library path again, because the path is the one
 * thing the agent was ever missing (`oculus read` takes it as-is). Nothing
 * here sends a display name.
 *
 * Growing the box is the browser's job now — `min-h`/`max-h` and a scrollbar
 * replace the height arithmetic a textarea needed. Line breaks, undo and
 * selection are also left alone; the only keys this intercepts are
 * Shift+Enter (a break rather than a form submit) and a Backspace or Delete
 * that would otherwise nibble the inside of a chip.
 *
 * It is the composer's box and also a task body's editor
 * (`app/src/pages/TaskPage.tsx`), which is what `initialText` and `className`
 * are for: a body *opens* holding what was written last time, where a message
 * always starts empty, and it is given a page's worth of height instead of the
 * composer's ten lines.
 */
export function MentionInput({
  ref,
  placeholder,
  initialText,
  label = "Message",
  className,
  autoFocus,
  onEdit,
  onFiles,
  onKeyDown,
  onBlur,
}: {
  ref?: Ref<MentionInputHandle>;
  placeholder: string;
  /** What the box opens holding. Read once, at mount: after that the DOM is
   *  the truth and a prop that kept re-seeding it would fight the typist —
   *  the same rule `DraftField` states for the row it follows. */
  initialText?: string;
  /** What a screen reader calls this box. */
  label?: string;
  /** Appended to the box's own classes, so a call site can give it more
   *  height. `cn` is tailwind-merge, so a `max-h-…` here replaces the one
   *  below rather than racing it in the cascade. */
  className?: string;
  autoFocus?: boolean;
  /** Every change and every caret move: the message and the caret in it. */
  onEdit: (text: string, caret: number) => void;
  /** Pictures pasted into the box. They are not text and never enter the
   *  editor's nodes — the composer holds them beside it and puts their paths
   *  into the message on send. */
  onFiles?: (files: File[]) => void;
  /** Runs *before* this component's own keys, so the composer's Enter-sends
   *  and `@`-menu navigation win; anything it does not `preventDefault` falls
   *  through to the editing keys below. */
  onKeyDown?: (e: KeyboardEvent<HTMLDivElement>) => void;
  onBlur?: () => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [chunks, setChunks] = useState<Chunk[]>(() => normalize(toChunks(initialText ?? "")));
  /** Bumped on every structural change. It is the editor's `key`, so React
   *  builds the box's nodes fresh from `chunks` instead of diffing them
   *  against a tree the typist has been editing underneath it. */
  const [version, setVersion] = useState(0);
  /** Drawn only while there is nothing to read. `:empty::before` cannot do
   *  it: a box holding WebKit's trailing placeholder `<br>` is not empty. */
  const [blank, setBlank] = useState(!(initialText ?? "").trim());
  const landing = useRef<Landing | null>(null);

  /** The live caret, or null when the selection is somewhere else entirely. */
  function point(el: HTMLElement): Point | null {
    const sel = window.getSelection();
    if (!sel || sel.rangeCount === 0 || !sel.focusNode) return null;
    if (!el.contains(sel.focusNode)) return null;
    return { node: sel.focusNode, offset: sel.focusOffset };
  }

  /** Tell the composer what the box holds. Called on input, but also on
   *  arrow keys and clicks: a caret that lands beside an existing `@` opens
   *  the menu just as typing one does. */
  function sync() {
    const el = box.current;
    if (!el) return;
    const r = readEditor(el, point(el));
    setBlank(!r.text.trim());
    onEdit(r.text, r.caret ?? r.text.length);
  }

  /** Re-render the box from a new set of chunks, with the caret going to an
   *  offset in the message rather than to a node that is about to be
   *  replaced. */
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

  // The caret, after the nodes it addresses exist. A structural change
  // remounts the box, so there is no earlier moment at which this could be
  // done — and no later one either, if the box is not to be seen with the
  // caret in the wrong place.
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
      // The token lives inside a single text run: a query can hold spaces but
      // never a backtick or a newline, so neither a chip nor a line break can
      // sit inside the stretch being replaced.
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
    insertText(text) {
      const el = box.current;
      if (!el) return;
      const r = readEditor(el, point(el));
      const caret = r.caret ?? r.text.length;
      let at = 0;
      for (let i = 0; i < r.chunks.length; i++) {
        const c = r.chunks[i];
        const len = chunkText(c).length;
        if (c.kind === "text" && caret <= at + len) {
          const cut = domOffset(c.text, caret - at);
          commit(
            [
              ...r.chunks.slice(0, i),
              { kind: "text", text: c.text.slice(0, cut) + text + c.text.slice(cut) },
              ...r.chunks.slice(i + 1),
            ],
            caret + text.length,
            true,
          );
          return;
        }
        at += len;
      }
      // No text run holds the caret — an empty box, or a caret sitting hard
      // against a chip that ends the content. Appending is the only place
      // left, and it is where the caret already is.
      commit([...r.chunks, { kind: "text", text }], r.text.length + text.length, true);
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
  }));

  // Mount-time focus only. Every later focus is `commit`'s, which knows
  // whether the box had it before the nodes were replaced.
  //
  // A box that opens with text gets the caret at the *end* of it: WebKit
  // focuses a contenteditable with the selection at its start, which for a
  // task body clicked into to keep writing means typing in front of what is
  // already there. An empty box has no nodes, so the composer takes the bare
  // `focus()` it always did.
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

  /** The chip immediately before or after a collapsed caret, if a delete key
   *  would land on one. Only at the very edge of a text run, so a Backspace
   *  with a character to eat still eats the character. */
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
      // Through the browser, so the break joins the undo stack and WebKit
      // places its own trailing placeholder — a `<br>` typed in by hand at
      // the end of the box leaves the caret on the line above it. The
      // fallback is what `whitespace-pre-wrap` is for.
      e.preventDefault();
      if (!document.execCommand("insertLineBreak")) {
        document.execCommand("insertText", false, "\n");
      }
      // The line the break just made is below the fold on a box that has
      // reached its `max-h`, and WebKit does not follow it down.
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
        aria-label={label}
        aria-placeholder={placeholder}
        // Ten lines and then a scrollbar, as the textarea did — but measured
        // by the browser rather than by two writes to `style.height`.
        //
        // `select-text` is not decoration: the app turns selection off on
        // `body` and hands it back per tag (`index.css`), and a `div` is not
        // one of the tags — a textarea was. Without it the box cannot be
        // selected in, which in WebKit means it can barely be edited in.
        // `overflow-x-hidden` is load-bearing, not tidying. Setting the y axis
        // to `auto` computes the x axis to `auto` too, and with macOS set to
        // always show scrollbars rather than overlay them, WebKit reserves and
        // paints a horizontal bar in a box one line tall — across the text.
        // The content wraps, so there is never anything to scroll sideways.
        className={cn(
          "max-h-[160px] min-h-[16px] w-full overflow-x-hidden overflow-y-auto break-words whitespace-pre-wrap select-text text-[13px] leading-[16px] outline-none",
          className,
        )}
        onInput={sync}
        onKeyUp={sync}
        onClick={sync}
        onKeyDown={keyDown}
        onBlur={onBlur}
        onPaste={(e) => {
          // Plain text only. A pasted rich fragment would bring its own
          // styling and its own elements into a box whose nodes mean
          // something, and `insertText` keeps the browser's undo.
          e.preventDefault();
          // A screenshot is a file on the clipboard, and it comes with a text
          // flavour of its own on some sources — the picture wins, and the
          // box is left as it was.
          const pictures = onFiles ? imageFiles(e.clipboardData.files) : [];
          if (pictures.length) {
            onFiles?.(pictures);
            return;
          }
          const plain = e.clipboardData.getData("text/plain");
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
