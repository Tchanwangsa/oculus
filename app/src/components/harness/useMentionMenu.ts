import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type RefObject } from "react";
import type { MentionInputHandle } from "@/components/harness/MentionInput";
import { countUnparsedMentionMatches, searchMentionFiles, type MentionFile } from "@/lib/db";

/** How much of an `@` token to look at. Long enough for a real filename,
 *  short enough that a stray `@` in prose stops matching once the sentence
 *  runs on. */
const MAX_MENTION = 60;

/** …and how many words of it. Filenames here run to "Week 3 Workshop
 *  Solutions", so four is comfortably past a real title, and it is the second
 *  half of the guard the character cap used to provide on its own: now that a
 *  query may hold spaces, a `@` in prose would otherwise keep matching for
 *  sixty characters of sentence. */
const MAX_MENTION_WORDS = 4;

/** The gap the `@` menu keeps from the line it is anchored to and from the
 *  viewport edge — the old `mb-2`/`mt-2`, in pixels, because the flip below
 *  and the placement in `./MentionMenu.tsx` both do arithmetic with it. */
export const MENU_GAP = 8;

/** Where the open `@` is on screen: the left edge of the `@` character itself
 *  and the top and bottom of the line it sits on, in viewport CSS pixels —
 *  what a `fixed` popup needs to sit under a caret rather than under a box. */
export interface MentionAnchor {
  left: number;
  top: number;
  bottom: number;
}

/**
 * That rect, read off the live selection. `back` is how far the `@` is behind
 * the caret — the query plus the `@` itself.
 *
 * **The `@` and not the caret**, because the query grows as it is typed: a
 * menu pinned to the live caret would slide right one character per keystroke,
 * which is both distracting and wrong — the list is about the token, and the
 * token starts at the `@`. It also makes a second reading *idempotent*, so
 * re-measuring after a scroll can only correct the position and never jitter
 * it.
 *
 * **The collapsed-range trap, avoided rather than handled.** In WebKit
 * `getBoundingClientRect()` on a collapsed Range can come back an all-zero
 * rect, so the reading is taken from a range **extended back over the token**,
 * from the `@` to the caret: never empty, and its first client rect is the
 * first line fragment it covers — exactly the `@`'s left edge and that line's
 * box. The token is known to live inside a single text node (`insertMention`
 * in `./MentionInput.tsx` leans on the same fact: a query holds no backtick
 * and no newline, so no chip and no line break can be inside one), so
 * `focusOffset - back` addresses the `@` directly. The character there is
 * *checked* to be an `@` rather than trusted, because the DOM carries
 * zero-width caret guards the message does not.
 *
 * If that check fails the fallbacks step down, and **none of them is 0,0** —
 * a menu in the window's top-left corner is worse than one that is too wide:
 * the collapsed range's first client rect, then its bounding rect if it has
 * any height at all, then the rect of the element the selection sits in, which
 * is the box itself and roughly where the menu used to be pinned anyway. Only
 * a selection that has left the document gives up, and then the menu draws
 * nothing.
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
 * The `@…` token the caret is sitting in, or null.
 *
 * Anchored to the start of a word so an email address or a handle typed
 * mid-word never opens the menu.
 *
 * The token **may hold spaces**, because filenames do: stopping at the first
 * one put "Week 3 Workshop" permanently out of the menu's reach. Three things
 * keep that from leaving a menu armed over the Enter key for the rest of a
 * sentence — the character after the `@` must not be whitespace, so "@ "
 * closes the menu at once; the token is capped at `MAX_MENTION` characters
 * and `MAX_MENTION_WORDS` words; and a backtick ends it, which is what stops
 * a token reaching back across a mention already picked, since a chip reads
 * out as `` `path` ``. The real protection is still that the menu only opens
 * when the query matched a file.
 */
export function mentionQuery(
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

/** Everything `MentionMenu` needs to draw itself, handed over as one object so
 *  a call site spreads it rather than re-deriving it. */
export interface MentionMenuProps {
  ref: RefObject<HTMLDivElement | null>;
  /** Whether there is a list to show at all. */
  open: boolean;
  /** …and whether the emptiness is worth explaining. The two are exclusive:
   *  a menu with rows never needs the line. */
  emptyReason: boolean;
  files: MentionFile[];
  index: number;
  onIndex: (i: number) => void;
  onPick: (file: MentionFile) => void;
  /** Where to open: the `@`'s own rect, so the list appears where the token
   *  was typed. Null while nothing could be measured, which is the one case
   *  the menu declines to draw. */
  anchor: MentionAnchor | null;
  /** Which way to open. Measured by the hook, since only it knows how tall the
   *  list came out. */
  drop: "up" | "down";
  /** Null is the whole library, and the only case where a row says which
   *  subject its file came from. */
  subjectId: number | null;
  /** How many files the query matched that have no markdown, when the menu
   *  itself has nothing to show. */
  unparsed: number;
}

/**
 * The `@` machinery, once, for every box that has mentions in it.
 *
 * The token parser, the file lookup, the "no markdown" count, the selected
 * row, the keyboard handling, the `@`'s own place on screen and the measured
 * up/down flip all live here; the list itself is `./MentionMenu.tsx`. It was
 * all inside `Composer.tsx` until a task body wanted the same `@`, and a
 * second copy of it is exactly the duplication that stays correct until one of
 * them grows a rule the other lacks — `FileChip`'s "one definition, N places" argument, applied to the
 * menu that produces the chip.
 *
 * What is deliberately *not* here: whatever the box does with the text. The
 * composer sends it, the task body saves it, and neither is the menu's
 * business — the hook only reports the token and splices the pick in through
 * the editor's own handle.
 *
 * `keyDown` runs first at every call site and `preventDefault`s the keys it
 * claims, so the caller's own Enter (send, or save) is guarded by a
 * `defaultPrevented` check and the two never fight over a keystroke.
 */
export function useMentionMenu({
  subjectId,
  input,
}: {
  /** What `@` may reach: a subject, or null for the whole library. */
  subjectId: number | null;
  /** The editor the pick is spliced into. */
  input: RefObject<MentionInputHandle | null>;
}) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [mention, setMention] = useState<{ query: string; start: number } | null>(null);
  /** The open `@`'s place in the window. Kept beside `mention` rather than
   *  inside it so a re-measurement does not look like a new token and send the
   *  lookup below round again. */
  const [anchor, setAnchor] = useState<MentionAnchor | null>(null);
  const [files, setFiles] = useState<MentionFile[]>([]);
  /** Files this query matched that have no markdown, so could not be offered. */
  const [unparsed, setUnparsed] = useState(0);
  const [index, setIndex] = useState(0);
  /** Which way the `@` menu opens. See the layout effect that sets it. */
  const [drop, setDrop] = useState<"up" | "down">("down");
  // The query the menu is currently showing, so a slow lookup that lands
  // after the token changed cannot overwrite a newer list.
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
        // Only when the menu has nothing to show: an empty type-ahead reads
        // as a typo, and for a PDF that is simply unparsed it is not one.
        // Counting on every keystroke would be a second query per key for a
        // line almost nobody ever sees.
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

  // A scroll moves the `@` on screen without touching the selection, and a
  // `fixed` popup does not ride along with it. Re-measured rather than closed,
  // the way an editor's completion popup usually is, because the box being
  // typed into is itself a scroller: a message long enough to fill it scrolls
  // on every keystroke, and closing on that would shut the menu mid-word.
  // Capture phase, since a scroll event does not bubble — and scrolling the
  // menu's own list lands here too, harmlessly, because the `@` has not moved.
  useEffect(() => {
    if (!mention) return;
    const remeasure = () => setAnchor(measureAnchor(mention.query.length + 1));
    window.addEventListener("scroll", remeasure, true);
    return () => window.removeEventListener("scroll", remeasure, true);
  }, [mention]);

  const open = mention !== null && files.length > 0;
  /**
   * Why the menu is empty, when the answer is "those files have no markdown".
   *
   * Deliberately *not* rows in the menu: a row that cannot be picked is a
   * control that does nothing, and the agent genuinely cannot read a file that
   * was never parsed. It is a line instead — and it is outside `open`, so the
   * list's keyboard handling, its flip measurement and the caller's own Enter
   * all behave exactly as they do with no `@` open at all.
   */
  const emptyReason = mention !== null && files.length === 0 && unparsed > 0;

  /**
   * The `@` menu opens downwards, and only flips up when it would not fit.
   *
   * The composer alone is the same component in three very different places —
   * the middle of the home page, the middle of the chat hero, and pinned to
   * the bottom of an open thread — so which way is "out of the way" is a fact
   * about the viewport, not about the call site. Opening up unconditionally
   * was right for the thread and wrong everywhere else: on the home page the
   * list covered the subject pill and the cards above it while the whole
   * lower half of the page sat empty.
   *
   * Measured after the menu is in the DOM rather than against its maximum
   * height, so a three-file list is judged on the ~90px it actually occupies
   * instead of the 256px it is allowed. `useLayoutEffect`, so the flip lands
   * before paint and the menu is never seen in the wrong place.
   *
   * What it measures against is now the **caret's line** and not the box: the
   * menu hangs off the `@`, so the room it has is the room under that line.
   */
  useLayoutEffect(() => {
    const menu = menuRef.current;
    if (!open || !menu || !anchor) return;
    const needed = menu.offsetHeight + MENU_GAP;
    setDrop(
      window.innerHeight - anchor.bottom >= needed || anchor.top < needed ? "down" : "up",
    );
  }, [open, files.length, anchor]);

  /**
   * What the box holds and where the caret is in it.
   *
   * The token is recomputed from wherever the caret actually is — typing, but
   * also an arrow key or a click that lands beside an existing `@` — and the
   * editor fires this on all three.
   *
   * The `@`'s rect is read **here**, inside the event that produced the
   * caret: `onEdit` comes off `input`, `keyup` and `click`, so the selection is
   * live and the position lands in the same commit the menu appears in rather
   * than a frame later.
   *
   * The calls that are not keystrokes are `commit`'s, which reports the new
   * text *before* it has re-rendered the box — and none of them leaves a
   * token open to place. A pick puts a chip and a space under the caret, a
   * pasted picture its markdown, a restore the handed-back message; the regex
   * finds no `@` in front of any of them, so the menu is shut and the box may
   * remount as freely as it likes. If one ever did match there, the reading
   * would be one keystroke stale and the next keystroke would correct it —
   * which is only true because the anchor is the `@` and not the caret.
   */
  function track(text: string, caret: number) {
    const next = mentionQuery(text, caret);
    setMention(next);
    setAnchor(next ? measureAnchor(next.query.length + 1) : null);
  }

  function close() {
    setMention(null);
    setAnchor(null);
  }

  /** Swap the `@token` for the file, which the box draws as a chip and
   *  serializes back to its fenced library path. The editor owns the caret
   *  that lands after it, since only it knows which node the chip became. */
  function pick(file: MentionFile) {
    if (!mention) return;
    input.current?.insertMention(mention.start, file.relative_path);
    close();
  }

  /** The menu's keys, claimed before the caller's. Arrows move, Enter and Tab
   *  pick, Escape closes — and nothing at all is claimed while the list is
   *  shut, so a box whose Enter means something else keeps it. */
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
    /** One object, spread into one `<MentionMenu {...menu} />`: a call site
     *  that had to know when the list is open, which way it flips or when the
     *  "no markdown" line applies would be holding half the menu again. */
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
