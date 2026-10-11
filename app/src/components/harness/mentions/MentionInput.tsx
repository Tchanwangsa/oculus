import { useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, type KeyboardEvent, type Ref } from "react";
import { FileChip } from "@/components/markdown/FileChip";
import { imageFiles } from "@/lib/harness/attachments";
import {
  caretChunk,
  chipText,
  chunkText,
  chunksText,
  domOffset,
  isChip,
  normalize,
  readEditor,
  repaintCaret,
  revealCaret,
  toChunks,
  ZWSP_RE,
  type Chunk,
  type Point,
} from "./dom";
import type { Landing, MentionInputHandle } from "./types";

export type { MentionInputHandle } from "./types";

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
  /** The pending `repaintCaret`; one is enough for a burst of line breaks. */
  const repaint = useRef(0);
  useEffect(() => () => cancelAnimationFrame(repaint.current), []);

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
      cancelAnimationFrame(repaint.current);
      repaint.current = repaintCaret(el);
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
