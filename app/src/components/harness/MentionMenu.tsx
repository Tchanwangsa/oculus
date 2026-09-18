import type { CSSProperties } from "react";
import { createPortal } from "react-dom";
import { categoryIconFor } from "@/lib/fileTypes";
import { fileTitle } from "@/lib/openFile";
import { displayCode } from "@/lib/format";
import { PARSE_SWEEP_NOTE } from "@/lib/parseState";
import { useParseStore } from "@/stores/parseStore";
import {
  MENU_GAP,
  type MentionAnchor,
  type MentionMenuProps,
} from "@/components/harness/useMentionMenu";
import { cn } from "@/lib/utils";

/** How wide the list is, now that nothing else sizes it. A completion popup
 *  is read at the caret and one row at a time, so it wants to be narrow: wide
 *  enough for "Week 3 Workshop Solutions" and a subject code, not as wide as
 *  the box the mention is being typed into. Long titles `truncate`. */
const MENU_WIDTH = 300;

/** Ten rows, the `max-h-64` this used to carry as a class. It is a number now
 *  because the placement below has to fit it against the room there is. */
const MENU_MAX_H = 256;

/** …and the height the popup refuses to shrink below. It only matters where
 *  the flip found no good side at all; a few pixels of list with a scrollbar
 *  in them would be worse than overlapping the line that was typed. */
const MENU_MIN_H = 96;

/**
 * The popup's place in the window, from the `@`'s own rect.
 *
 * It is `fixed` and portalled, so the arithmetic is plain CSS pixels on both
 * sides: the app's zoom is the *webview's* page zoom (`setZoom` in
 * `app/src/layouts/AppLayout.tsx`), which scales client rects,
 * `window.innerWidth` and this popup together. It is a CSS `zoom` on a
 * container that would report pointer coordinates and element rects in
 * different units and quietly break exactly this kind of maths, which is why
 * the app has none (CLAUDE.md).
 *
 * Clamped on both axes rather than allowed off-screen. Horizontally, so an `@`
 * typed near the right edge — a task body on a narrow window, the composer
 * with the panel open — does not push the list out of view. Vertically twice
 * over: the side the flip chose gets the room it actually has as a
 * `max-height`, so an eight-file list scrolls inside the viewport instead of
 * running past it, and the offset itself keeps `MENU_MIN_H` of the popup in
 * the window even when neither side had room for it.
 */
function place(anchor: MentionAnchor, drop: "up" | "down"): CSSProperties {
  // The width gives way before the window does, for a window narrower than
  // the popup wants to be — there is no useful list at 300px in a 200px gap.
  const width = Math.min(MENU_WIDTH, window.innerWidth - MENU_GAP * 2);
  const left = Math.max(MENU_GAP, Math.min(anchor.left, window.innerWidth - width - MENU_GAP));
  const room =
    drop === "down"
      ? window.innerHeight - anchor.bottom - MENU_GAP * 2
      : anchor.top - MENU_GAP * 2;
  // One expression for both sides: an inset this far from the window's own
  // edge leaves `MENU_MIN_H` of the popup inside it either way.
  const inset = Math.min(
    drop === "down" ? anchor.bottom + MENU_GAP : window.innerHeight - anchor.top + MENU_GAP,
    window.innerHeight - MENU_GAP - MENU_MIN_H,
  );
  return {
    left,
    width,
    maxHeight: Math.max(MENU_MIN_H, Math.min(MENU_MAX_H, room)),
    ...(drop === "down" ? { top: inset } : { bottom: inset }),
  };
}

/** The popover both branches below wear, so the list and the sentence that
 *  replaces it are recognisably the same surface in the same place. */
const POPUP = "fixed z-50 rounded-xl border border-border bg-popover shadow-md";

/**
 * The `@` list, drawn at the caret that is typing the mention.
 *
 * `fixed` and portalled to the body — it hangs off the `@`'s own client rect
 * (`MentionAnchor`, measured in `./useMentionMenu.ts`) rather than off the box,
 * which is what turns it from a panel as wide as the composer, pinned under the
 * whole thing, into a completion popup where the token was typed. A portal
 * because a `fixed` child of a box that may be transformed, scrolled or
 * `overflow`-clipped is a popup that inherits all three; positioning is this
 * app's own arithmetic, as every other popup here is.
 *
 * It also draws the "no markdown" line, because it is the same popover in the
 * same place with the same flip, and the two cases never both apply: a query
 * that matched files gets rows, and one that matched only unparsed files gets
 * the sentence saying so.
 */
export function MentionMenu({
  ref,
  open,
  emptyReason,
  files,
  index,
  onIndex,
  onPick,
  anchor,
  drop,
  subjectId,
  unparsed,
}: MentionMenuProps) {
  /** An app-wide parse failure, so the line below can name the real cause
   *  rather than blaming the files. */
  const latch = useParseStore((s) => s.latch);

  // Nothing measured is nothing to hang off. It takes a selection that has
  // left the document entirely to get here, and a popup in the window's corner
  // would be worse than none.
  if (!anchor) return null;

  if (emptyReason) {
    return createPortal(
      <div
        style={place(anchor, drop)}
        className={cn(
          POPUP,
          "overflow-y-auto px-3 py-2 text-[11px] leading-snug text-muted-foreground",
        )}
      >
        {unparsed === 1
          ? "1 matching file has no markdown, so it cannot be mentioned."
          : `${unparsed} matching files have no markdown, so they cannot be mentioned.`}
        {` ${latch ? latch.message : PARSE_SWEEP_NOTE}`}
      </div>,
      document.body,
    );
  }

  if (!open) return null;

  return createPortal(
    <div
      ref={ref}
      style={place(anchor, drop)}
      className={cn(POPUP, "overflow-y-auto overflow-x-hidden py-1")}
    >
      {files.map((f, i) => {
        // The same glyph the chip this row produces will wear, so the row
        // and its result are recognisably one thing.
        const Icon = categoryIconFor(f);
        return (
          <button
            key={f.id}
            type="button"
            // `mousedown` and not `click`, prevented: the box must not lose
            // focus to this button, or a pick would blur the editor the chip
            // is about to be spliced into.
            onMouseDown={(e) => {
              e.preventDefault();
              onPick(f);
            }}
            onMouseEnter={() => onIndex(i)}
            className={cn(
              "flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs",
              i === index ? "bg-accent text-foreground" : "text-muted-foreground",
            )}
          >
            <Icon size={12} className="shrink-0" />
            <span className="truncate">{fileTitle(f)}</span>
            {subjectId == null && (
              <span className="ml-auto shrink-0 text-[10px] text-muted-foreground/70">
                {displayCode(f.subject_code)}
              </span>
            )}
          </button>
        );
      })}
    </div>,
    document.body,
  );
}
