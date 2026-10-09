import type { CSSProperties } from "react";
import { createPortal } from "react-dom";
import { categoryIconFor } from "@/lib/files/fileTypes";
import { fileTitle } from "@/lib/files/openFile";
import { displayCode } from "@/lib/format/format";
import { PARSE_SWEEP_NOTE } from "@/lib/pipeline/parseState";
import { useParseStore } from "@/stores/sync/parseStore";
import {
  MENU_GAP,
  type MentionAnchor,
  type MentionMenuProps,
} from "@/components/harness/mentions/useMentionMenu";
import { cn } from "@/lib/utils";

const MENU_WIDTH = 300;
const MENU_MAX_H = 256;
/** Floor when neither side has room: overlap the line rather than shrink to nothing. */
const MENU_MIN_H = 96;

/**
 * `fixed` placement from the `@`'s rect, clamped into the window on both axes;
 * `maxHeight` is the room on the chosen side. Plain CSS pixels are safe because
 * zoom is the webview's page zoom (see docs/ui.md#gotchas).
 */
function place(anchor: MentionAnchor, drop: "up" | "down"): CSSProperties {
  const width = Math.min(MENU_WIDTH, window.innerWidth - MENU_GAP * 2);
  const left = Math.max(MENU_GAP, Math.min(anchor.left, window.innerWidth - width - MENU_GAP));
  const room =
    drop === "down"
      ? window.innerHeight - anchor.bottom - MENU_GAP * 2
      : anchor.top - MENU_GAP * 2;
  // Either side, keeps at least `MENU_MIN_H` of the popup in the window.
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

const POPUP = "fixed z-50 rounded-xl border border-border bg-popover shadow-md";

/**
 * The `@` list, portalled (a `fixed` child of a transformed or clipped box would
 * inherit both) and hung off the `@`'s rect. Also draws the "no markdown" line
 * in the same place when every match is unparsed.
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
  /** An app-wide parse failure, named as the real cause. */
  const latch = useParseStore((s) => s.latch);

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
        const Icon = categoryIconFor(f);
        return (
          <button
            key={f.id}
            type="button"
            // Prevented mousedown, so the editor keeps focus for the splice.
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
