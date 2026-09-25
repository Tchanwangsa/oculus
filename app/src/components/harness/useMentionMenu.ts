import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type RefObject } from "react";
import type { MentionInputHandle } from "@/components/harness/MentionInput";
import { countUnparsedMentionMatches, searchMentionFiles, type MentionFile } from "@/lib/db";

/** Caps on an `@` token, so a stray `@` in prose stops matching as the sentence
 *  runs on (queries may hold spaces, since filenames do). */
const MAX_MENTION = 60;
const MAX_MENTION_WORDS = 4;

/** Pixel gap from the anchor line and viewport edge; `./MentionMenu.tsx` uses it too. */
export const MENU_GAP = 8;

/** The `@` character's left edge and its line's top/bottom, in viewport pixels. */
export interface MentionAnchor {
  left: number;
  top: number;
  bottom: number;
}

/**
 * The `@`'s rect off the live selection; `back` is query length + 1. Anchored
 * to the `@`, not the caret, so the menu doesn't slide as the query grows and a
 * re-measure is idempotent. Measured over the range `@`→caret because WebKit
 * can return an all-zero rect for a collapsed Range; the `@` is checked, not
 * trusted (the DOM holds ZWSP guards). Fallbacks never yield 0,0.
 */
function measureAnchor(back: number): MentionAnchor | null {
  const sel = window.getSelection();
  const node = sel?.focusNode;
  if (!sel || !node) return null;

  const range = document.createRange();
  const at = sel.focusOffset - back;
  if (node.nodeType === Node.TEXT_NODE && at >= 0 && (node as Text).data[at] === "@") {
    range.setStart(node, at);
    range.setEnd(node, sel.focusOffset);
  } else {
    range.setStart(node, sel.focusOffset);
    range.collapse(true);
  }

  const rects = range.getClientRects();
  let rect: DOMRect | null = rects.length > 0 ? rects[0] : null;
  if (!rect) {
    const bounds = range.getBoundingClientRect();
    if (bounds.height > 0) rect = bounds;
  }
  if (!rect) {
    const el = node.nodeType === Node.TEXT_NODE ? node.parentElement : (node as Element);
    const bounds = el?.getBoundingClientRect();
    if (bounds && (bounds.height > 0 || bounds.width > 0)) rect = bounds;
  }
  return rect ? { left: rect.left, top: rect.top, bottom: rect.bottom } : null;
}

/**
 * The `@…` token at the caret, or null. Starts a word (so emails don't match)
 * and may hold spaces; "@ " closes it, the caps bound it, and a backtick ends
 * it so it can't reach back across a chip (which reads out as `` `path` ``).
 */
function mentionQuery(
  text: string,
  caret: number,
): { query: string; start: number } | null {
  const m = new RegExp(`(?:^|\\s)@([^\\s@\`\\n][^@\`\\n]{0,${MAX_MENTION - 1}})?$`).exec(
    text.slice(0, caret),
  );
  if (!m) return null;
  const query = m[1] ?? "";
  if (query.split(/\s+/).filter(Boolean).length > MAX_MENTION_WORDS) return null;
  return { query, start: caret - query.length - 1 };
}

export interface MentionMenuProps {
  ref: RefObject<HTMLDivElement | null>;
  open: boolean;
  /** Exclusive with `open`: nothing to show because the matches are unparsed. */
  emptyReason: boolean;
  files: MentionFile[];
  index: number;
  onIndex: (i: number) => void;
  onPick: (file: MentionFile) => void;
  /** Null when nothing could be measured; the menu then draws nothing. */
  anchor: MentionAnchor | null;
  drop: "up" | "down";
  /** Null is the whole library — the only case a row names its subject. */
  subjectId: number | null;
  unparsed: number;
}

/**
 * The `@` machinery for every box with mentions (composer, task body); the
 * list itself is `./MentionMenu.tsx`. What the box does with its text is the
 * caller's. `keyDown` runs first at each call site and `preventDefault`s what
 * it claims, so the caller's Enter checks `defaultPrevented`.
 */
export function useMentionMenu({
  subjectId,
  input,
}: {
  /** A subject, or null for the whole library. */
  subjectId: number | null;
  input: RefObject<MentionInputHandle | null>;
}) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [mention, setMention] = useState<{ query: string; start: number } | null>(null);
  /** Beside `mention`, not in it, so a re-measure doesn't re-run the lookup. */
  const [anchor, setAnchor] = useState<MentionAnchor | null>(null);
  const [files, setFiles] = useState<MentionFile[]>([]);
  const [unparsed, setUnparsed] = useState(0);
  const [index, setIndex] = useState(0);
  const [drop, setDrop] = useState<"up" | "down">("down");
  // Guards against a slow lookup overwriting a newer list.
  const latest = useRef("");

  useEffect(() => {
    if (!mention) {
      setFiles([]);
      setUnparsed(0);
      return;
    }
    const token = `${subjectId ?? ""} ${mention.query}`;
    latest.current = token;
    searchMentionFiles(subjectId, mention.query)
      .then(async (found) => {
        if (latest.current !== token) return;
        setFiles(found);
        setIndex(0);
        // Counted only when empty, to explain it — not a second query per key.
        const missing = found.length === 0
          ? await countUnparsedMentionMatches(subjectId, mention.query).catch(() => 0)
          : 0;
        if (latest.current === token) setUnparsed(missing);
      })
      .catch(() => {});
  }, [mention, subjectId]);

  // A subject change re-scopes what `@` may reach, so the open list is stale.
  useEffect(() => {
    setMention(null);
    setAnchor(null);
  }, [subjectId]);

  // Re-measure (not close) on scroll: the box itself scrolls while typing, and
  // a `fixed` popup doesn't follow. Capture phase, since scroll doesn't bubble.
  useEffect(() => {
    if (!mention) return;
    const remeasure = () => setAnchor(measureAnchor(mention.query.length + 1));
    window.addEventListener("scroll", remeasure, true);
    return () => window.removeEventListener("scroll", remeasure, true);
  }, [mention]);

  const open = mention !== null && files.length > 0;
  // Unparsed matches get an explanatory line, not unpickable rows; being outside
  // `open`, it leaves keyboard handling and the caller's Enter untouched.
  const emptyReason = mention !== null && files.length === 0 && unparsed > 0;

  // Open down unless the list's measured height won't fit under the `@`'s line
  // (the composer sits mid-page and at the bottom). Before paint, so it never flashes.
  useLayoutEffect(() => {
    const menu = menuRef.current;
    if (!open || !menu || !anchor) return;
    const needed = menu.offsetHeight + MENU_GAP;
    setDrop(
      window.innerHeight - anchor.bottom >= needed || anchor.top < needed ? "down" : "up",
    );
  }, [open, files.length, anchor]);

  /** The editor's `onEdit`. The anchor is measured here, inside the event, so the
   *  selection is live and the menu lands placed in the same commit. `commit`'s
   *  calls report before re-render but never leave a token open to place. */
  function track(text: string, caret: number) {
    const next = mentionQuery(text, caret);
    setMention(next);
    setAnchor(next ? measureAnchor(next.query.length + 1) : null);
  }

  function close() {
    setMention(null);
    setAnchor(null);
  }

  function pick(file: MentionFile) {
    if (!mention) return;
    input.current?.insertMention(mention.start, file.relative_path);
    close();
  }

  /** Claims nothing while the list is shut, so the caller keeps its Enter. */
  function keyDown(e: KeyboardEvent<HTMLElement>) {
    if (!open) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setIndex((i) => (i + 1) % files.length);
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      setIndex((i) => (i - 1 + files.length) % files.length);
      return;
    }
    if (e.key === "Enter" || e.key === "Tab") {
      e.preventDefault();
      pick(files[index]);
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    }
  }

  return {
    track,
    close,
    keyDown,
    /** Spread into `<MentionMenu {...menu} />`. */
    menu: {
      ref: menuRef,
      open,
      emptyReason,
      files,
      index,
      onIndex: setIndex,
      onPick: pick,
      anchor,
      drop,
      subjectId,
      unparsed,
    } satisfies MentionMenuProps,
  };
}
